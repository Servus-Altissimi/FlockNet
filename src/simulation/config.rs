
use crate::agent::TrafficPattern;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimConfig {
    pub name: String,
    pub strategy_name: String,
    pub num_agents: u32,
    pub num_servers: u32,
    pub duration: Duration,
    pub buffer_size: usize,
    pub bandwidth_bps: u64,
    pub traffic_pattern: TrafficPattern,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            name: "default_sim".to_string(),
            strategy_name: "drop-tail".to_string(),
            num_agents: 64,
            num_servers: 4,
            duration: Duration::from_secs(60),
            buffer_size: 1024,
            bandwidth_bps: 100_000_000,
            traffic_pattern: TrafficPattern::Constant { rate_pps: 100.0 },
        }
    }
}

impl SimConfig {
    pub fn with_strategy(mut self, strategy: impl Into<String>) -> Self {
        self.strategy_name = strategy.into();
        self
    }
    
    pub fn with_peak_traffic(mut self, base: f64, peak: f64, duration_s: f64) -> Self {
        self.traffic_pattern = TrafficPattern::PeakTraffic {
            base_rate: base,
            peak_rate: peak,
            peak_duration_s: duration_s,
        };
        self
    }
}