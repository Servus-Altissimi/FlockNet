use super::{Action, Strategy};
use crate::network::Packet;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CoDel {
    target: Duration,
    interval: Duration,
    first_above_time: Option<Instant>,
    drop_next: Instant,
    count: u32,
    dropping: bool,
}

impl CoDel {
    pub fn new() -> Self {
        Self {
            target: Duration::from_millis(5),
            interval: Duration::from_millis(100),
            first_above_time: None,
            drop_next: Instant::now(),
            count: 0,
            dropping: false,
        }
    }

    fn control_law(&self, _t: Duration) -> Duration {
        Duration::from_secs_f64(
            self.interval.as_secs_f64() / (self.count as f64).sqrt().max(1.0)
        )
    }
}

impl Strategy for CoDel {
    fn on_enqueue(&mut self, _packet: &Packet, _queue_len: usize) -> Action {
        let now = Instant::now();
        
        if self.dropping {
            // Check if it's time to drop based on control law
            if now >= self.drop_next {
                self.count += 1;
                self.drop_next = now + self.control_law(Duration::from_millis(0));
                Action::Drop
            } else {
                Action::Accept
            }
        } else {
            Action::Accept
        }
    }

    fn on_dequeue(&mut self, _queue_len: usize) {} // CoDel makes decisions on enqueue, redundant

    fn update(&mut self, _queue_len: usize, avg_sojourn_ms: f64) {
        let now = Instant::now();
        let sojourn_time = Duration::from_secs_f64(avg_sojourn_ms / 1000.0);

        if sojourn_time < self.target {
            // Below target: Reset
            self.first_above_time = None;
            self.dropping = false;
            self.count = 0;
        } else {
            if self.first_above_time.is_none() {
                self.first_above_time = Some(now);
            }
            
            if let Some(first_above) = self.first_above_time {
                if now.duration_since(first_above) > self.interval {
                    if !self.dropping {
                        self.dropping = true;
                        self.count = 1;
                        self.drop_next = now;
                    }
                }
            }
        }
    }

    fn name(&self) -> &str {"CoDel"}

    fn reset(&mut self) {
        self.first_above_time = None;
        self.dropping = false;
        self.count = 0;
    }

    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(self.clone())
    }
}