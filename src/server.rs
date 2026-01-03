// A lot of debug prints due to issues I had developing

use crate::network::Packet;
use crate::strategies::Strategy;
use crate::metrics::MetricsCollector;
use tokio::net::TcpListener;
use std::collections::VecDeque;
use std::sync::Arc;
use parking_lot::Mutex;
use tracing::{info, warn, debug};

pub struct Server {
    id: u32,
    addr: String,
    buffer: Arc<Mutex<VecDeque<Packet>>>,
    strategy: Arc<Mutex<Box<dyn Strategy>>>,
    metrics: MetricsCollector,
    bandwidth_bps: u64,
}

impl Server {
    pub fn new(
        id: u32,
        addr: String,
        strategy: Box<dyn Strategy>,
        metrics: MetricsCollector,
        bandwidth_bps: u64,
    ) -> Self {
        Self {
            id,
            addr,
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            strategy: Arc::new(Mutex::new(strategy)),
            metrics,
            bandwidth_bps,
        }
    }

    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;
        info!("Server {} listening on {}", self.id, self.addr);

        let processor = self.clone();
        tokio::spawn(async move {
            processor.process_queue().await;
        });

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    debug!("Server {} accepted connection from {}", self.id, addr);
                    let server = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = server.handle_connection(socket).await {
                            warn!("Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    warn!("Accept error: {}", e);
                }
            }
        }
    }

    pub async fn run_with_counter(
        self: Arc<Self>,
        ready_counter: Arc<std::sync::atomic::AtomicU32>,
    ) -> anyhow::Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;
        info!("Server {} listening on {}", self.id, self.addr);

        ready_counter.fetch_add(1, std::sync::atomic::Ordering::Release);

        let processor = self.clone();
        tokio::spawn(async move {
            processor.process_queue().await;
        });

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    debug!("Server {} accepted connection from {}", self.id, addr);
                    let server = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = server.handle_connection(socket).await {
                            warn!("Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    warn!("Accept error: {}", e);
                }
            }
        }
    }

    async fn handle_connection(&self, mut socket: tokio::net::TcpStream) -> anyhow::Result<()> {
        use tokio::io::AsyncReadExt;

        let mut buf = vec![0u8; 4096];

        loop {
            let n = socket.read(&mut buf).await?;
            if n == 0 {
                break;
            }

            if let Ok(packet) = bincode::deserialize::<Packet>(&buf[..n]) {
                self.enqueue_packet(packet);
            }
        }

        Ok(())
    }

    fn enqueue_packet(&self, packet: Packet) {
        let mut buffer = self.buffer.lock();
        let mut strategy = self.strategy.lock();

        let action = strategy.on_enqueue(&packet, buffer.len());

        match action {
            crate::strategies::Action::Accept => {
                buffer.push_back(packet.clone());
            }
            crate::strategies::Action::Drop => {
                self.metrics.packet_dropped();
            }
            crate::strategies::Action::Mark => {
                buffer.push_back(packet.clone());
            }
        }

        self.metrics.record_queue_length(buffer.len());
    }

    async fn process_queue(&self) {
        let packet_time = std::time::Duration::from_secs_f64(
            (1500.0 * 8.0) / self.bandwidth_bps as f64
        );

        info!("Server {} packet transmission time: {:?}", self.id, packet_time);

        let mut recent_sojourn_times: Vec<f64> = Vec::new(); 
        let mut update_counter = 0;
        let mut packets_processed = 0u64;

        loop {
            tokio::time::sleep(packet_time).await;

            let packet_opt = {
                let mut buffer = self.buffer.lock();
                buffer.pop_front()
            };

            if let Some(packet) = packet_opt {
                let sojourn = packet.sojourn_time();
                let sojourn_ms = sojourn.as_secs_f64() * 1000.0;
                
                packets_processed += 1;
                
                // Print first 10 packets just to verify timing
                if packets_processed <= 10 {
                    debug!("Server {} packet #{}: sojourn_time = {:.6}ms", 
                          self.id, packets_processed, sojourn_ms);
                }
                
                self.metrics.packet_received(sojourn);
                
                // Warn about impossible values (>30 seconds, magic number)
                if sojourn_ms > 30_000.0 {
                    warn!("Server {} DEQUEUE: Impossibly high sojourn time {:.2}ms for packet {:?}", 
                          self.id, sojourn_ms, packet.id);
                }
                
                recent_sojourn_times.push(sojourn_ms);
                
                if recent_sojourn_times.len() > 100 {
                    recent_sojourn_times.remove(0);
                }

                let queue_len = self.buffer.lock().len();

                let mut strategy = self.strategy.lock();
                strategy.on_dequeue(queue_len);
                drop(strategy);
            }

            update_counter += 1;

            if update_counter >= 3 {
                update_counter = 0;
                
                let queue_len = self.buffer.lock().len();
                
                let avg_sojourn = if !recent_sojourn_times.is_empty() {
                    recent_sojourn_times.iter().sum::<f64>() / recent_sojourn_times.len() as f64
                } else {
                    0.0
                };
                
                if packets_processed % 100 == 0 && packets_processed > 0 {
                    debug!("Server {} processed {} packets! Average sojourn: {:.2}ms, queue: {}", 
                          self.id, packets_processed, avg_sojourn, queue_len);
                }
                
                let mut strategy = self.strategy.lock();
                strategy.update(queue_len, avg_sojourn);
            }
        }
    }
}