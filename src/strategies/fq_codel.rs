use super::{Action, Strategy};
use crate::network::Packet;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct FlowState {
    first_above_time: Option<Instant>,
    drop_next: Instant,
    count: u32,
    dropping: bool,
}

impl FlowState {
    fn new() -> Self {
        Self {
            first_above_time: None,
            drop_next: Instant::now(),
            count: 0,
            dropping: false,
        }
    }
}

#[derive(Debug, Clone)]
struct QueuedPacket {
    packet: Packet,
    flow_id: u32,
    enqueue_time: Instant,
}

#[derive(Debug)]
pub struct FqCoDel {
    num_flows: usize,
    flow_states: HashMap<u32, FlowState>,
    flow_queues: HashMap<u32, VecDeque<QueuedPacket>>,
    buffer_size: usize,
    target: Duration,
    interval: Duration,
    last_dequeue_flow: u32,
}

impl FqCoDel {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            num_flows: 1024,
            flow_states: HashMap::new(),
            flow_queues: HashMap::new(),
            buffer_size,
            target: Duration::from_millis(5),
            interval: Duration::from_millis(100),
            last_dequeue_flow: 0,
        }
    }

    fn hash_flow(packet: &Packet) -> u32 {
        packet.source_agent % 1024
    }

    fn control_law(&self, count: u32) -> Duration {
        Duration::from_secs_f64(
            self.interval.as_secs_f64() / (count as f64).sqrt().max(1.0)
        )
    }

    fn total_queue_length(&self) -> usize {
        self.flow_queues.values().map(|q| q.len()).sum()
    }

    fn flow_queue_length(&self, flow_id: u32) -> usize {
        self.flow_queues.get(&flow_id).map(|q| q.len()).unwrap_or(0)
    }

    fn get_sojourn_time(&self, flow_id: u32) -> Duration {
        if let Some(queue) = self.flow_queues.get(&flow_id) {
            if let Some(oldest) = queue.front() {
                return oldest.enqueue_time.elapsed();
            }
        }
        Duration::from_millis(0)
    }

    // same as standalone CoDel
    fn should_drop_flow(&mut self, flow_id: u32, sojourn_time: Duration, now: Instant) -> bool {
        let state = self.flow_states.entry(flow_id).or_insert_with(FlowState::new);

        if sojourn_time < self.target {
            state.first_above_time = None;
            state.dropping = false;
            state.count = 0;
            return false;
        }

        if state.first_above_time.is_none() {
            state.first_above_time = Some(now);
            return false;
        }

        if let Some(first_above) = state.first_above_time {
            let time_above = now.duration_since(first_above);

            if time_above < self.interval {
                return false;
            }

            if !state.dropping {
                state.dropping = true;
                state.count = 1;
                state.drop_next = now;
                return true;
            }

            if now >= state.drop_next {
                state.count += 1;
                let count = state.count;
                let interval = self.interval;
                drop(state);
                let control_duration = self.control_law(count);
                let state = self.flow_states.get_mut(&flow_id).unwrap();
                state.drop_next = now + control_duration;
                return true;
            }
        }

        false
    }
}

impl Strategy for FqCoDel {
    fn on_enqueue(&mut self, packet: &Packet, _queue_len: usize) -> Action {
        let flow_id = Self::hash_flow(packet);
        let now = Instant::now();
        
        if self.total_queue_length() >= self.buffer_size {
            return Action::Drop;
        }

        let sojourn_time = self.get_sojourn_time(flow_id);

        // Apply CoDel algorithm per flow
        if self.should_drop_flow(flow_id, sojourn_time, now) {
            Action::Drop
        } else {
            let queued_packet = QueuedPacket {
                packet: packet.clone(),
                flow_id,
                enqueue_time: now,
            };
            self.flow_queues
                .entry(flow_id)
                .or_insert_with(VecDeque::new)
                .push_back(queued_packet);
            Action::Accept
        }
    }

    fn on_dequeue(&mut self, _queue_len: usize) {
        let mut flow_ids: Vec<u32> = self.flow_queues.keys().copied().collect();
        if flow_ids.is_empty() {
            return;
        }
        
        flow_ids.sort();
        
        // find start position (after last dequeued flow)
        let start_pos = flow_ids.iter().position(|&id| id > self.last_dequeue_flow).unwrap_or(0);
        
        flow_ids.rotate_left(start_pos);
        
        for flow_id in flow_ids {
            if let Some(queue) = self.flow_queues.get_mut(&flow_id) {
                if !queue.is_empty() {
                    queue.pop_front();
                    self.last_dequeue_flow = flow_id;
                    break;
                }
            }
        }
    }

    fn update(&mut self, _queue_len: usize, _avg_sojourn_ms: f64) {
        // Cleanup empty flows to prevent memory leak (redundant)
        self.flow_states.retain(|_, state| state.dropping || state.first_above_time.is_some());
        self.flow_queues.retain(|_, queue| !queue.is_empty());
    }

    fn name(&self) -> &str { "FQ-CoDel" }

    fn reset(&mut self) {
        self.flow_states.clear();
        self.flow_queues.clear();
        self.last_dequeue_flow = 0;
    }

    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(Self::new(self.buffer_size))
    }
}