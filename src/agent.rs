// A lot of magic numbers here

use crate::network::{Packet, PacketId, Priority};
use crate::metrics::MetricsCollector;
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use tokio::time::{interval, Duration};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::Mutex;
use tracing::{info, warn, debug};
use rand::{Rng, thread_rng};
use rand_distr::{Distribution, Exp};
use serde::{Deserialize, Serialize};

pub struct Agent {
    id: u32,
    server_addrs: Vec<String>,
    packet_counter: AtomicU64,
    metrics: MetricsCollector,
    traffic_pattern: TrafficPattern,
    connections: Arc<Mutex<Vec<Option<TcpStream>>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrafficPattern {
    Constant { rate_pps: f64 },
    Bursty { avg_rate_pps: f64, burst_size: u32 },
    Poisson { lambda: f64 },
    PeakTraffic { base_rate: f64, peak_rate: f64, peak_duration_s: f64 },
}

impl Agent {
    pub fn new(
        id: u32,
        server_addrs: Vec<String>,
        metrics: MetricsCollector,
        traffic_pattern: TrafficPattern,
    ) -> Self {
        let num_servers = server_addrs.len();
        Self {
            id,
            server_addrs,
            packet_counter: AtomicU64::new(0),
            metrics,
            traffic_pattern,
            connections: Arc::new(Mutex::new((0..num_servers).map(|_| None).collect())),
        }
    }
    
    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
        info!("Agent {} starting with pattern {:?}", self.id, self.traffic_pattern);
        
        match &self.traffic_pattern {
            TrafficPattern::Constant { rate_pps } => {
                self.run_constant(*rate_pps).await
            }
            TrafficPattern::Bursty { avg_rate_pps, burst_size } => {
                self.run_bursty(*avg_rate_pps, *burst_size).await
            }
            TrafficPattern::Poisson { lambda } => {
                self.run_poisson(*lambda).await
            }
            TrafficPattern::PeakTraffic { base_rate, peak_rate, peak_duration_s } => {
                self.run_peak_traffic(*base_rate, *peak_rate, *peak_duration_s).await
            }
        }
    }
    
    async fn run_constant(&self, rate_pps: f64) -> anyhow::Result<()> {
        let interval_ms = (1000.0 / rate_pps).max(1.0) as u64;
        let mut tick = interval(Duration::from_millis(interval_ms));
        
        loop {
            tick.tick().await;
            self.send_packet().await;
        }
    }
    
    async fn run_bursty(&self, avg_rate_pps: f64, burst_size: u32) -> anyhow::Result<()> {
        let burst_interval_ms = (burst_size as f64 / avg_rate_pps * 1000.0) as u64;
        let mut tick = interval(Duration::from_millis(burst_interval_ms));
        
        loop {
            tick.tick().await;
            
            for _ in 0..burst_size {
                self.send_packet().await;
                tokio::time::sleep(Duration::from_micros(100)).await; // magic number
            }
        }
    }
    
    async fn run_poisson(&self, lambda: f64) -> anyhow::Result<()> {
        let exp_dist = Exp::new(lambda).unwrap();
        
        loop {
            let wait_time = {
                let mut rng = thread_rng();
                exp_dist.sample(&mut rng)
            };
            tokio::time::sleep(Duration::from_secs_f64(wait_time)).await;
            self.send_packet().await;
        }
    }
    
    async fn run_peak_traffic(
        &self,
        base_rate: f64,
        peak_rate: f64,
        peak_duration_s: f64,
    ) -> anyhow::Result<()> {
        let start = tokio::time::Instant::now();
        let peak_duration = Duration::from_secs_f64(peak_duration_s);
        
        loop {
            let elapsed = start.elapsed();
            let rate = if elapsed < peak_duration {
                peak_rate
            } else {
                base_rate
            };
            
            let interval_ms = (1000.0 / rate).max(1.0) as u64;
            tokio::time::sleep(Duration::from_millis(interval_ms)).await;
            self.send_packet().await;
        }
    }
    
    async fn send_packet(&self) {
        let packet_id = self.packet_counter.fetch_add(1, Ordering::Relaxed);
        let server_idx = thread_rng().gen_range(0..self.server_addrs.len());
        
        let packet = Packet::new(
            PacketId::new(packet_id),
            self.id,
            server_idx as u32,
            1500,
            Priority::Normal,
        );
        
        let result = self.send_w_connection(server_idx, &packet).await; // Try to get, or atleast create persistent connection
        
        match result {
            Ok(_) => {
                self.metrics.packet_sent();
                debug!("Agent {} sent packet {} to server {}", self.id, packet_id, server_idx);
            }
            Err(e) => {
                warn!("Agent {} failed to send packet: {}", self.id, e);
                self.metrics.packet_dropped();
                
                let mut conns = self.connections.lock(); // Clean slate
                conns[server_idx] = None;
            }
        }
    }
    
    async fn send_w_connection(&self, server_idx: usize, packet: &Packet) -> anyhow::Result<()> {
        let server_addr = &self.server_addrs[server_idx];
        
        //  prio connection first, then create new one
        let mut stream = {
            let mut conns = self.connections.lock();
            conns[server_idx].take()
        };
        
        if stream.is_none() {
            match TcpStream::connect(server_addr).await {
                Ok(new_stream) => {
                    stream = Some(new_stream);
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
        
        if let Some(mut s) = stream {
            let data = wincode::serialize(packet)?;
            
            match s.write_all(&data).await {
                Ok(_) => {
                    let mut conns = self.connections.lock();
                    conns[server_idx] = Some(s);
                    Ok(())
                }
                Err(e) => {
                    Err(e.into())
                }
            }
        } else {
            Err(anyhow::anyhow!("Failed to establish any connection"))
        }
    }
}