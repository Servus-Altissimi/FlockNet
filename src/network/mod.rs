pub mod packet;

pub use packet::{Packet, PacketId, Priority};

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub bandwidth_bps: u64,
    pub latency_ms: u64,
    pub buffer_size: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            bandwidth_bps: 100_000_000,
            latency_ms: 5,
            buffer_size: 1024,
        }
    }
}
