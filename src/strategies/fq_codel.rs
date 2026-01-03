// This was a headache

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
        }
    }

    fn hash_flow(packet: &Packet) -> u32 {
        packet.source_agent % 1024 // Hash based on sourced agent to separate all the flows

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

    fn estimate_sojourn_time(&self, flow_id: u32) -> Duration {
        // Use avg_sojourn from update() if available, otherwise estimate
        let queue_len = self.flow_queue_length(flow_id);
        // conservative estimate for 100Mbps, 1500 byte packets
        Duration::from_micros((queue_len as u64) * 120)
    }
}

impl Strategy for FqCoDel {
    fn on_enqueue(&mut self, packet: &Packet, _queue_len: usize) -> Action {
        let flow_id = Self::hash_flow(packet);
        let now = Instant::now();
        
        // Check if total buffer is full
        if self.total_queue_length() >= self.buffer_size {
            return Action::Drop;
        }

        // Estimate sojourn time based on flow queue length
        let sojourn_time = self.estimate_sojourn_time(flow_id);
        
        // Calculate control law interval before borrowing state mutably
        let interval = self.interval;
        let target = self.target;
        let control_law_fn = |count: u32| -> Duration {
            Duration::from_secs_f64(interval.as_secs_f64() / (count as f64).sqrt().max(1.0))
        };
        
        // Get or create flow state
        let state = self.flow_states.entry(flow_id).or_insert_with(FlowState::new);

        // Apply CoDel algorithm for every flow
        let should_drop = if sojourn_time < target {
            state.first_above_time = None; // Below target: Reset
            state.dropping = false;
            state.count = 0;
            false
        } else {
            if state.first_above_time.is_none() {
                state.first_above_time = Some(now);
            }
            
            if let Some(first_above) = state.first_above_time {
                if now.duration_since(first_above) > interval {
                    if !state.dropping {
                        state.dropping = true;
                        state.count = 1;
                        state.drop_next = now;
                        true
                    } else if now >= state.drop_next {
                        // Continue dropping according to control law
                        let count = state.count;
                        state.count += 1;
                        state.drop_next = now + control_law_fn(count);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };

        if should_drop {
            Action::Drop
        } else {
            // If accepted, add to the specific flows queue
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
        // Round-robin dequeue: find the first non-empty flow and dequeue from it
        let mut flow_ids: Vec<u32> = self.flow_queues.keys().copied().collect();
        flow_ids.sort();
        
        for flow_id in flow_ids {
            if let Some(queue) = self.flow_queues.get_mut(&flow_id) {
                if !queue.is_empty() {
                    queue.pop_front();
                    break; // Only dequeue one packet per call
                }
            }
        }
    }

    fn update(&mut self, _queue_len: usize, _avg_sojourn_ms: f64) { 
        self.flow_states.retain(|_, state| state.dropping || state.first_above_time.is_some());
        self.flow_queues.retain(|_, queue| !queue.is_empty());
    }

    fn name(&self) -> &str { "FQ-CoDel" }

    fn reset(&mut self) {
        self.flow_states.clear();
        self.flow_queues.clear();
    }

    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(Self::new(self.buffer_size))
    }
}