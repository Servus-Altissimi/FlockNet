pub mod logger;
pub mod analyzer;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub timestamp: f64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub packets_dropped: u64,
    pub throughput_bps: f64,
    pub avg_latency_ms: f64,
    pub queue_length: usize,
    pub packet_loss_rate: f64,
}

#[derive(Debug, Clone)]
pub struct MetricsCollector {
    inner: Arc<RwLock<MetricsInner>>,
    start_time: Instant,
}

#[derive(Debug)]
struct MetricsInner {
    packets_sent: u64,
    packets_received: u64,
    packets_dropped: u64,
    total_latency_ms: f64,
    latency_samples: u64,
    queue_lengths: Vec<usize>,
    snapshots: Vec<MetricsSnapshot>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(MetricsInner {
                packets_sent: 0,
                packets_received: 0,
                packets_dropped: 0,
                total_latency_ms: 0.0,
                latency_samples: 0,
                queue_lengths: Vec::new(),
                snapshots: Vec::new(),
            })),
            start_time: Instant::now(),
        }
    }

    pub fn packet_sent(&self) {
        self.inner.write().packets_sent += 1;
    }

    pub fn packet_received(&self, latency: Duration) {
        let mut inner = self.inner.write();
        inner.packets_received += 1;
        
        let latency_ms = latency.as_secs_f64() * 1000.0;
        
        // Only reject truly impossible values (30s as always)
        if latency_ms > 30_000.0 {
            warn!("Detected impossible latency: {:.2}ms - ignoring sample (likely timing bug)", latency_ms);
            return;
        }
        
        // Only count valid samples for average calculation
        inner.total_latency_ms += latency_ms;
        inner.latency_samples += 1;
    }

    pub fn packet_dropped(&self) {
        self.inner.write().packets_dropped += 1;
    }

    pub fn record_queue_length(&self, len: usize) {
        self.inner.write().queue_lengths.push(len);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let inner = self.inner.read();
        
        let elapsed = self.start_time.elapsed().as_secs_f64();
        
        let throughput_bps = if elapsed > 0.0 {
            (inner.packets_received as f64 * 1500.0 * 8.0) / elapsed
        } else {
            0.0
        };
        
        let avg_latency_ms = if inner.latency_samples > 0 {
            let avg = inner.total_latency_ms / inner.latency_samples as f64;
            
            // Additional sanity check on the average, redundant
            if avg > 10_000.0 {
                warn!("Average latency suspiciously high: {:.2}ms - data may be corrupted", avg);
                0.0
            } else {
                avg
            }
        } else {
            0.0
        };
        
        let packet_loss_rate = if inner.packets_sent > 0 {
            inner.packets_dropped as f64 / inner.packets_sent as f64
        } else {
            0.0
        };
        
        let queue_length = inner.queue_lengths.last().copied().unwrap_or(0);
        
        MetricsSnapshot {
            timestamp: elapsed,
            packets_sent: inner.packets_sent,
            packets_received: inner.packets_received,
            packets_dropped: inner.packets_dropped,
            throughput_bps,
            avg_latency_ms,
            queue_length,
            packet_loss_rate,
        }
    }

    pub fn save_snapshot(&self) {
        let snapshot = self.snapshot();
        self.inner.write().snapshots.push(snapshot);
    }

    pub fn get_snapshots(&self) -> Vec<MetricsSnapshot> {
        self.inner.read().snapshots.clone()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}