pub mod config;
pub use config::SimConfig;

use crate::agent::{Agent, TrafficPattern};
use crate::server::Server;
use crate::strategies::StrategyRegistry;
use crate::metrics::{MetricsCollector, analyzer};
use crate::metrics::logger::MetricsLogger;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tracing::{info, error};
use indicatif::{ProgressBar, ProgressStyle};

pub struct Simulation {
    config: SimConfig,
    pub metrics: MetricsCollector,
}

impl Simulation {
    pub fn new(config: SimConfig) -> Self {
        Self {
            config,
            metrics: MetricsCollector::new(),
        }
    }
    
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting simulation: {}", self.config.name);
        info!("Strategy: {}", self.config.strategy_name);
        info!("Duration: {:?}", self.config.duration);
        info!("Agents: {}, Servers: {}", self.config.num_agents, self.config.num_servers);
        
        let cancel_token = CancellationToken::new(); // Create cancellation token for graceful shutdown later, prevents issues next run
        
        // Create notification system for server readiness
        let ready_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let mut servers = Vec::new();
        let mut server_handles = Vec::new();
        
        // Start servers w readiness notification
        for i in 0..self.config.num_servers {
            let addr = format!("127.0.0.1:{}", 5000 + i);
            let strategy = StrategyRegistry::global()
                .create(&self.config.strategy_name, self.config.buffer_size)
                .ok_or_else(|| anyhow::anyhow!("Unknown strategy: {}", self.config.strategy_name))?;
            
            let server = Arc::new(Server::new(
                i,
                addr.clone(),
                strategy,
                self.metrics.clone(),
                self.config.bandwidth_bps,
            ));
            
            let server_clone = server.clone();
            let ready_counter = ready_count.clone();
            let cancel = cancel_token.clone();
            
            let handle = tokio::spawn(async move {
                tokio::select! {
                    result = server_clone.run_with_counter(ready_counter) => {
                        if let Err(e) = result {
                            error!("Server {} error: {}", i, e);
                        }
                    }
                    _ = cancel.cancelled() => {
                        info!("Server {} shutting down", i);
                    }
                }
            });
            
            server_handles.push(handle);
            servers.push(server);
        }
        
        // Wait for ALL servers to be ready
        info!("Waiting for servers to be ready...");
        while ready_count.load(std::sync::atomic::Ordering::Acquire) < self.config.num_servers {
            sleep(Duration::from_millis(10)).await;
        }
        
        // Extra safety margin to ensure OS has fully bound ports, magic number
        sleep(Duration::from_millis(100)).await;
        info!("All servers ready!");
        
        let server_addrs: Vec<String> = (0..self.config.num_servers)
            .map(|i| format!("127.0.0.1:{}", 5000 + i))
            .collect();
        
        let mut agents = Vec::new();
        let mut agent_handles = Vec::new();
        
        for i in 0..self.config.num_agents {
            let pattern = self.get_traffic_pattern(i);
            let agent = Arc::new(Agent::new(
                i,
                server_addrs.clone(),
                self.metrics.clone(),
                pattern,
            ));
            
            let agent_clone = agent.clone();
            let cancel = cancel_token.clone();
            
            let handle = tokio::spawn(async move {
                tokio::select! {
                    result = agent_clone.run() => {
                        if let Err(e) = result {
                            error!("Agent {} error: {}", i, e);
                        }
                    }
                    _ = cancel.cancelled() => {
                        info!("Agent {} shutting down", i);
                    }
                }
            });
            
            agent_handles.push(handle);
            agents.push(agent);
        }
        
        let pb = ProgressBar::new(self.config.duration.as_secs());
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.orange/yellow} {pos}/{len}s {msg}")? // god I should've begun using colours earlier
                .progress_chars("█▓░")
        );
        
        let mut tick = interval(Duration::from_secs(1));
        for _ in 0..self.config.duration.as_secs() {
            tick.tick().await;
            self.metrics.save_snapshot();
            pb.inc(1);
            
            let snapshot = self.metrics.snapshot();
            pb.set_message(format!(
                "Loss: {:.2}% | Queue: {}",
                snapshot.packet_loss_rate * 100.0,
                snapshot.queue_length
            ));
        }
        
        pb.finish_with_message("Simulation complete");
        
        info!("Shutting down simulation..");
        cancel_token.cancel();
        
        for handle in agent_handles {
            let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
        }
        for handle in server_handles {
            let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
        }
        
        // Give OS time to release ports, magic number
        sleep(Duration::from_millis(500)).await;
        
        self.save_results()?;
        Ok(())
    }
    
    fn get_traffic_pattern(&self, agent_id: u32) -> TrafficPattern {
        match &self.config.traffic_pattern {
            TrafficPattern::PeakTraffic { base_rate, peak_rate, peak_duration_s } => {
                let variance = 0.1;
                let factor = 1.0 + (agent_id as f64 * 0.01) % variance - variance / 2.0;
                TrafficPattern::PeakTraffic {
                    base_rate: base_rate * factor,
                    peak_rate: peak_rate * factor,
                    peak_duration_s: *peak_duration_s,
                }
            }
            pattern => pattern.clone(),
        }
    }
    
    fn save_results(&self) -> Result<()> {
        let snapshots = self.metrics.get_snapshots();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        
        std::fs::create_dir_all("results")?;
        
        let csv_path = format!("results/{}_{}.csv", self.config.name, timestamp);
        let mut logger = MetricsLogger::new(&csv_path)?;
        logger.log_batch(&snapshots)?;
        info!("Results saved to: {}", csv_path);
        
        let report = analyzer::analyze(
            &snapshots,
            &self.config.strategy_name
        );
        
        let json_path = format!("results/{}_{}_analysis.json", self.config.name, timestamp);
        std::fs::write(&json_path, serde_json::to_string_pretty(&report)?)?;
        info!("Analysis saved to: {}", json_path);
        
        let plot_data_path = format!("results/{}_{}_plot.dat", self.config.name, timestamp);
        analyzer::export_latex_plot_data(&snapshots, &plot_data_path)?;
        info!("Plot data saved to: {}", plot_data_path);
        
        info!("Avg Throughput: {:.2} Mbps", report.avg_throughput_mbps);
        info!("Avg Latency: {:.2} ms", report.avg_latency_ms);
        info!("Packet Loss: {:.2}%", report.packet_loss_rate * 100.0);
        
        Ok(())
    }
}