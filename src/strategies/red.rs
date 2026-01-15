use super::{Action, Strategy};
use crate::network::Packet;
use rand::Rng;

#[derive(Debug, Clone)]
pub struct Red {
    min_th: f64,
    max_th: f64,
    pub max_p: f64,
    w_q: f64,
    pub avg_queue: f64,
    count: usize,
}

impl Red {
    pub fn new(buffer_size: usize) -> Self {
        let min_th = (buffer_size as f64 * 0.3).max(5.0);
        let max_th = buffer_size as f64 * 0.9;
        Self {
            min_th,
            max_th,
            max_p: 0.1,
            w_q: 0.02,
            avg_queue: 0.0,
            count: 0,
        }
    }

    fn calc_probability(&self, avg: f64) -> f64 {
        if avg < self.min_th {
            0.0
        } else if avg >= self.max_th {
            1.0
        } else {
            // p_b = base probability
            let p_b = ((avg - self.min_th) / (self.max_th - self.min_th)) * self.max_p;
            p_b / (1.0 - (self.count as f64) * p_b)
        }
    }
}

impl Strategy for Red {
    fn on_enqueue(&mut self, _packet: &Packet, queue_len: usize) -> Action {
        // Update EWMA (Exponentially Weighted Moving Average) of queue length
        self.avg_queue = (1.0 - self.w_q) * self.avg_queue + self.w_q * (queue_len as f64);
        let drop_prob = self.calc_probability(self.avg_queue);
    
        if drop_prob >= 1.0 {
            // Force drop when above max_th
            self.count = 0;
            Action::Drop
        } else if drop_prob > 0.0 && rand::thread_rng().r#gen::<f64>() < drop_prob {
            // Probabilistic drop between min_th and max_th
            self.count = 0;
            Action::Drop
        } else {
            self.count += 1;
            Action::Accept
        }
    }

    fn on_dequeue(&mut self, _queue_len: usize) { }

    fn update(&mut self, queue_len: usize, _avg_sojourn_ms: f64) {
        // Update EWMA periodically
        self.avg_queue = (1.0 - self.w_q) * self.avg_queue + self.w_q * (queue_len as f64);
    }

    fn name(&self) -> &str { "RED" }

    fn reset(&mut self) {
        self.avg_queue = 0.0;
        self.count = 0;
    }

    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Clone)]
pub struct AdaptiveRed {
    pub red: Red,
    target: f64,
    alpha: f64,
    beta: f64,
    last_update: std::time::Instant,
}

impl AdaptiveRed {
    pub fn new(buffer_size: usize) -> Self {
        let red = Red::new(buffer_size);
        let target = 0.5 * (red.min_th + red.max_th);
        Self {
            red,
            target,
            alpha: 0.01,
            beta: 0.9,
            last_update: std::time::Instant::now(),
        }
    }
}

impl Strategy for AdaptiveRed {
    fn on_enqueue(&mut self, packet: &Packet, queue_len: usize) -> Action {
        self.red.on_enqueue(packet, queue_len)
    }

    fn on_dequeue(&mut self, queue_len: usize) {
        self.red.on_dequeue(queue_len);
    }

    fn update(&mut self, queue_len: usize, avg_sojourn_ms: f64) {
        self.red.update(queue_len, avg_sojourn_ms);
        
        if self.last_update.elapsed().as_millis() >= 500 {
            if self.red.avg_queue < self.target && self.red.max_p < 0.5 {
                self.red.max_p += self.alpha.min(self.red.max_p / 4.0);
            } else if self.red.avg_queue > self.target && self.red.max_p > 0.01 {
                self.red.max_p *= self.beta;
            }
            self.last_update = std::time::Instant::now();
        }
    }

    fn name(&self) -> &str { "Adaptive-RED" }

    fn reset(&mut self) {
        self.red.reset();
        self.last_update = std::time::Instant::now();
    }

    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(self.clone())
    }
}