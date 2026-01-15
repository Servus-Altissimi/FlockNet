use super::{Action, Strategy};
use crate::network::Packet;
use std::time::{Duration, Instant};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
struct QueuedPacket {
    packet: Packet,
    enqueue_time: Instant,
}

#[derive(Debug, Clone)]
pub struct CoDel {
    target: Duration,
    interval: Duration,
    first_above_time: Option<Instant>,
    drop_next: Instant,
    count: u32,
    dropping: bool,
    queue: VecDeque<QueuedPacket>,
    buffer_size: usize,
}

impl CoDel {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            target: Duration::from_millis(5),
            interval: Duration::from_millis(100),
            first_above_time: None,
            drop_next: Instant::now(),
            count: 0,
            dropping: false,
            queue: VecDeque::new(),
            buffer_size,
        }
    }

    fn control_law(&self) -> Duration {
        Duration::from_secs_f64(
            self.interval.as_secs_f64() / (self.count as f64).sqrt().max(1.0)
        )
    }
}

impl Strategy for CoDel {
    fn on_enqueue(&mut self, packet: &Packet, _queue_len: usize) -> Action {
        if self.queue.len() >= self.buffer_size {
            return Action::Drop;
        }

        // Always accept and tag with timestamp
        self.queue.push_back(QueuedPacket {
            packet: packet.clone(),
            enqueue_time: Instant::now(),
        });
        Action::Accept
    }

    fn on_dequeue(&mut self, _queue_len: usize) {
        loop {
            let Some(head) = self.queue.front() else {
                // Queue empty, exit dropping state
                self.dropping = false;
                self.first_above_time = None;
                return;
            };

            let now = Instant::now();
            let sojourn_time = now.duration_since(head.enqueue_time);

            // Check if sojourn time is below target
            if sojourn_time < self.target {
                self.first_above_time = None;
                self.dropping = false;
                self.queue.pop_front();
                return;
            }

            if self.first_above_time.is_none() {
                self.first_above_time = Some(now);
                self.queue.pop_front();
                return;
            }

            let time_above = now.duration_since(self.first_above_time.unwrap());
            
            if time_above < self.interval {
                self.queue.pop_front();
                return;
            }

            if !self.dropping {
                self.dropping = true;
                self.count = 1;
                self.drop_next = now;
                self.queue.pop_front(); // DROP the packet

                continue;
            }

            // Already in dropping state
            if now >= self.drop_next {
                self.count += 1;
                self.drop_next = now + self.control_law();
                self.queue.pop_front(); // DROP the packet
                continue;
            } else {
                // dequeue normally
                self.queue.pop_front();
                return;
            }
        }
    }

    fn update(&mut self, _queue_len: usize, _avg_sojourn_ms: f64) { }
    fn name(&self) -> &str { "CoDel" }

    fn reset(&mut self) {
        self.first_above_time = None;
        self.dropping = false;
        self.count = 0;
        self.queue.clear();
    }

    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(Self::new(self.buffer_size))
    }
}