use super::{Action, Strategy};
use crate::network::Packet;
use rand::Rng;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Blue {
    p_mark: f64,
    d1: f64,
    d2: f64,
    freeze_time: std::time::Duration,
    last_update: Instant,
    buffer_size: usize,
    last_increase: Instant,
    last_decrease: Instant,
    last_loss_event: Option<Instant>,
}

impl Blue {
    pub fn new(buffer_size: usize) -> Self {
        let now = Instant::now();
        Self {
            p_mark: 0.0,
            d1: 0.02,
            d2: 0.002, 
            freeze_time: std::time::Duration::from_millis(100),
            last_update: now,
            buffer_size,
            last_increase: now,
            last_decrease: now,
            last_loss_event: None,
        }
    }

    fn can_increase(&self) -> bool {
        self.last_increase.elapsed() >= self.freeze_time
    }

    fn can_decrease(&self) -> bool {
        self.last_decrease.elapsed() >= self.freeze_time
    }
}

impl Strategy for Blue {
    fn on_enqueue(&mut self, _packet: &Packet, queue_len: usize) -> Action {
        let now = Instant::now();
        
        let threshold = (self.buffer_size as f64 * 0.8) as usize;
        if queue_len >= threshold && self.can_increase() {
            self.p_mark = (self.p_mark + self.d1).min(1.0);
            self.last_increase = now;
        }
        
        if queue_len >= self.buffer_size {
            if let Some(last_loss) = self.last_loss_event {
                // If losses are frequent, increase more aggressively
                if last_loss.elapsed() < self.freeze_time {
                    self.p_mark = (self.p_mark + self.d1 * 2.0).min(1.0);
                }
            }
            self.last_loss_event = Some(now);
            self.last_increase = now;
            return Action::Drop;
        }
        
        // Probabilistic marking
        if self.p_mark > 0.0 && rand::thread_rng().r#gen::<f64>() < self.p_mark {
            Action::Drop
        } else {
            Action::Accept
        }
    }

    fn on_dequeue(&mut self, queue_len: usize) {
        // decrease when queue is low and has no recent losses
        if queue_len < (self.buffer_size / 4) && self.can_decrease() {
            if let Some(last_loss) = self.last_loss_event {
                if last_loss.elapsed() > self.freeze_time * 2 {
                    self.p_mark = (self.p_mark - self.d2).max(0.0);
                    self.last_decrease = Instant::now();
                }
            } else {
                self.p_mark = (self.p_mark - self.d2).max(0.0);
                self.last_decrease = Instant::now();
            }
        }
    }

    fn update(&mut self, queue_len: usize, _avg_sojourn_ms: f64) {
        // periodic adjustment
        if self.last_update.elapsed() > self.freeze_time * 5 {
            let target = self.buffer_size / 2;
            
            if queue_len > target && self.p_mark < 0.5 {
                self.p_mark = (self.p_mark + self.d1 * 0.5).min(1.0);
            } else if queue_len < target / 2 && self.p_mark > 0.01 {
                self.p_mark = (self.p_mark - self.d2 * 0.5).max(0.0);
            }
            
            self.last_update = Instant::now();
        }
    }

    fn name(&self) -> &str { "BLUE" }
    fn reset(&mut self) {
        self.p_mark = 0.0;
        let now = Instant::now();
        self.last_update = now;
        self.last_increase = now;
        self.last_decrease = now;
        self.last_loss_event = None;
    }

    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(Self::new(self.buffer_size))
    }
}