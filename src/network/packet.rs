use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PacketId(u64);

impl PacketId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Packet {
    pub id: PacketId,
    pub source_agent: u32,
    pub destination_server: u32,
    pub payload_size: u32,
    pub priority: Priority,
    
    // Store creation time as microseconds since UNIX_EPOCH
    // This CAN be serialized and works across network boundaries (:
    created_at_micros: u128,
    
    pub data: Vec<u8>,
}

impl Packet {
    pub fn new(
        id: PacketId,
        source: u32,
        dest: u32,
        size: u32,
        priority: Priority,
    ) -> Self {
        let created_at_micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros();
        
        if created_at_micros < 1_000_000_000_000_000 {
            eprintln!("WARNING: Packet created with invalid timestamp: {}", created_at_micros);
        }
        
        Self {
            id,
            source_agent: source,
            destination_server: dest,
            payload_size: size,
            priority,
            created_at_micros,
            data: vec![0; size as usize],
        }
    }
    
    pub fn sojourn_time(&self) -> Duration {  // Sojourn time = the total time a packet spends inside the system, cool term I learned
        // checks if created_at_micros is 0 or unreasonably small, 
        // it likely means that the packet wasn't initialized properly. return 0 to avoid ruining metrics
        if self.created_at_micros < 1_000_000_000_000_000 {  // 2001 
            return Duration::ZERO;
        }
        
        let now_micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros();
        
        let elapsed_micros = now_micros.saturating_sub(self.created_at_micros);
        
        if elapsed_micros > 30_000_000 { // if elapsed time is > 30 seconds, something is wrong, magic number in a way
            return Duration::ZERO;
        }
        
        // Convert to Duration (with overflow protection)
        Duration::from_micros(elapsed_micros.min(u64::MAX as u128) as u64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}