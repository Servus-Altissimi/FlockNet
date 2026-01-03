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
}

impl Blue {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            p_mark: 0.0,
            d1: 0.02,
            d2: 0.0002,
            freeze_time: std::time::Duration::from_millis(100),
            last_update: Instant::now(),
            buffer_size,
        }
    }

    fn can_update(&self) -> bool {
        self.last_update.elapsed() >= self.freeze_time
    }
}

impl Strategy for Blue {
    fn on_enqueue(&mut self, _packet: &Packet, queue_len: usize) -> Action {
        if queue_len >= self.buffer_size && self.can_update() {
            self.p_mark = (self.p_mark + self.d1).min(1.0);
            self.last_update = Instant::now();
        }

        if self.p_mark > 0.0 && rand::thread_rng().r#gen::<f64>() < self.p_mark {
            Action::Drop
        } else {
            Action::Accept
        }
    }

    fn on_dequeue(&mut self, queue_len: usize) {
        if queue_len == 0 && self.can_update() {
            self.p_mark = (self.p_mark - self.d2).max(0.0);
            self.last_update = Instant::now();
        }
    }

    fn update(&mut self, _queue_len: usize, _avg_sojourn_ms: f64) {}

    fn name(&self) -> &str {"BLUE"}

    fn reset(&mut self) { 
        self.p_mark = 0.0;
        self.last_update = Instant::now();
    }

    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(Self::new(self.buffer_size))
    }
}