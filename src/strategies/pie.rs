use super::{Action, Strategy};
use crate::network::Packet;
use rand::Rng;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Pie {
    target_delay: Duration,
    drop_prob: f64,
    alpha: f64,
    beta: f64,
    last_update: Instant,
    update_interval: Duration,
    qdelay_old: f64,
    burst_allowance: Duration,
    burst_start: Option<Instant>,
    bandwidth_bps: f64,
}

impl Pie {
    pub fn new() -> Self {
        Self {
            target_delay: Duration::from_millis(15),
            drop_prob: 0.0,
            alpha: 0.125,
            beta: 1.25,
            last_update: Instant::now(),
            update_interval: Duration::from_millis(30),
            qdelay_old: 0.0,
            burst_allowance: Duration::from_millis(150),
            burst_start: None,
            bandwidth_bps: 100_000_000.0,  // Default to 100 mbps
        }
    }

    fn estimate_queue_delay(&self, queue_len: usize) -> f64 {
        // (packet_size_bytes * 8 bits) / bandwidth_bps * 1000 = delay_ms 
        let packet_delay_ms = (1500.0 * 8.0) / self.bandwidth_bps * 1000.0;
        queue_len as f64 * packet_delay_ms
    }
}

impl Strategy for Pie {
    fn on_enqueue(&mut self, _packet: &Packet, queue_len: usize) -> Action {
        let now = Instant::now();

        // Allow burst if within allowance window
        if let Some(burst_start) = self.burst_start {
            if now.duration_since(burst_start) < self.burst_allowance {
                return Action::Accept;
            }
        }

        // Reset burst timer for them smaller queues
        if queue_len < 10 {
            self.burst_start = Some(now);
        }

        // Probabilistic drop
        if self.drop_prob > 0.0 && rand::thread_rng().r#gen::<f64>() < self.drop_prob {
            Action::Drop
        } else {
            Action::Accept
        }
    }

    fn update(&mut self, queue_len: usize, avg_sojourn_ms: f64) {
        let now = Instant::now();
        if now.duration_since(self.last_update) < self.update_interval {
            return;
        }

        let qdelay = if avg_sojourn_ms > 0.0 { // Use actual sojourn time if available, otherwise: a rough estimate
            avg_sojourn_ms
        } else {
            self.estimate_queue_delay(queue_len)
        };

        let target_ms = self.target_delay.as_secs_f64() * 1000.0;

        // PI controller update
        let p = self.alpha * (qdelay - target_ms);
        let i = self.beta * (qdelay - self.qdelay_old);

        self.drop_prob = (self.drop_prob + p + i).clamp(0.0, 1.0);
        self.qdelay_old = qdelay;
        self.last_update = now;
    }

    fn name(&self) -> &str {"PIE"}

    fn reset(&mut self) {
        self.drop_prob = 0.0;
        self.qdelay_old = 0.0;
        self.burst_start = None;
    }

    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(self.clone())
    }

    fn on_dequeue(&mut self, _queue_len: usize) {}
}