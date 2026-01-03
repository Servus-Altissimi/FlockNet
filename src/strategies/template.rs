// Template strategy, there are examples included but they were AI written
// I will try to write a proper guide ASAP 

use super::{Action, Strategy};
use crate::network::Packet;

#[derive(Debug, Clone)]
pub struct MyStrategy { // Replace MyStrategy with your strategy name
    // State variables
    buffer_size: usize,
    threshold: f64,
    drop_count: u64,
    
    // Add any other parameters you need:
    // - Counters (packets sent/received/dropped)
    // - Timers (last update time, intervals)
    // - Moving averages (queue length, latency)
    // - Algorithm specific state
}

impl MyStrategy {
    // Constructor to initialize your strategy
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer_size,
            threshold: 0.8,  // Example: 80% buffer threshold
            drop_count: 0,
        }
    }
    
    /// Optional: Builder pattern for custom parameters
    /// Usage: MyStrategy::new(1024).with_threshold(0.9)
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }
    
    /// Optional: Add more builder methods as needed
    pub fn with_custom_param(mut self, _param: f64) -> Self {
        // self.custom_param = param;
        self
    }
}

impl Strategy for MyStrategy {
    /// Called when a packet arrives at the queue
    /// Return Action::Accept to enqueue, Action::Drop to drop
    fn on_enqueue(&mut self, packet: &Packet, queue_len: usize) -> Action {
        // IMPLEMENT YOUR ENQUEUE LOGIC HERE
        
        // Example: Simple threshold based dropping
        let utilization = queue_len as f64 / self.buffer_size as f64;
        
        if utilization > self.threshold {
            self.drop_count += 1;
            Action::Drop
        } else {
            Action::Accept
        }
        
        // Other common patterns:
        //
        // 1. Probabilistic dropping (like with RED):
        // if rand::thread_rng().gen::<f64>() < drop_probability {
        //     Action::Drop
        // } else {
        //     Action::Accept
        // }
        //
        // 2. Priority-based:
        // match packet.priority {
        //     Priority::Critical => Action::Accept,
        //     Priority::Low if queue_len > threshold => Action::Drop,
        //     _ => Action::Accept,
        // }
        //
        // 3. Time-based (check packet.timestamp):
        // if packet.sojourn_time() > max_delay {
        //     Action::Drop
        // } else {
        //     Action::Accept
        // }
    }
    
    /// Called when a packet is removed from the queue
    /// Use this to update state after a dequeue
    fn on_dequeue(&mut self, queue_len: usize) {
        // optional: Implement dequeue logic here
        
        // Detect idle link
        // if queue_len == 0 {
        //     self.link_idle_count += 1;
        // }
        
        // Most strategies don't need this.
    }
    
    /// Called once in a while (~100ms) to update strategy state
    /// Use this for adaptive algorithms
    fn update(&mut self, queue_len: usize, avg_sojourn_ms: f64) {
        // OPTIONAL: Implement periodic update logic
        
        // Example: Adaptive threshold adjustment
        // if avg_sojourn_ms > target_latency {
        //     self.threshold *= 0.95;  // Lower threshold = more dropping
        // } else if avg_sojourn_ms < target_latency * 0.5 {
        //     self.threshold = (self.threshold + 0.05).min(1.0); // Raises threshold
        // }
        
        // Example: Moving average update
        // self.avg_queue = 0.9 * self.avg_queue + 0.1 * (queue_len as f64);
    }
    
    fn name(&self) -> &str {
        "MyStrategy"  // Your strategy name used in logs and reports
    }
    
    /// Reset strategy state (called between simulation runs)
    fn reset(&mut self) {
        // Reset all state variables to initial values
        self.drop_count = 0;
        // Reset other state variables as needed
    }
    
    /// Clone for parallel simulations, usually no changes needed
    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(self.clone())
    }
}
