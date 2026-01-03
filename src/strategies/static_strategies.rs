// A catch all for FIFO an Droptail, they're both pretty simplistic anyway

use super::{Action, Strategy};
use crate::network::Packet;

#[derive(Debug, Clone)]
pub struct DropTail {
    buffer_size: usize,
}

impl DropTail {
    pub fn new(buffer_size: usize) -> Self {
        Self { buffer_size }
    }
}

impl Strategy for DropTail {
    fn on_enqueue(&mut self, _packet: &Packet, queue_len: usize) -> Action {
        if queue_len >= self.buffer_size {
            Action::Drop
        } else {
            Action::Accept
        }
    }

    fn on_dequeue(&mut self, _queue_len: usize) {}

    fn update(&mut self, _queue_len: usize, _avg_sojourn_ms: f64) {}

    fn name(&self) -> &str {"DropTail"}

    fn reset(&mut self) {}

    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Clone)]
pub struct Fifo {
    buffer_size: usize,
}

impl Fifo {
    pub fn new(buffer_size: usize) -> Self {
        Self { buffer_size }
    }
}

impl Strategy for Fifo {
    fn on_enqueue(&mut self, _packet: &Packet, queue_len: usize) -> Action {
        if queue_len >= self.buffer_size {
            Action::Drop
        } else {
            Action::Accept
        }
    }

    fn on_dequeue(&mut self, _queue_len: usize) {}

    fn update(&mut self, _queue_len: usize, _avg_sojourn_ms: f64) {}

    fn name(&self) -> &str {"FIFO"}

    fn reset(&mut self) {}

    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(self.clone())
    }
}