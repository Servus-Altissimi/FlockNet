pub mod agent;
pub mod server;
pub mod network;
pub mod strategies;
pub mod metrics;
pub mod simulation;

pub use agent::Agent;
pub use server::Server;
pub use strategies::Strategy;
pub use simulation::{Simulation, SimConfig};
pub use metrics::MetricsCollector;

pub mod prelude {
    pub use crate::agent::Agent;
    pub use crate::server::Server;
    pub use crate::strategies::{Strategy, StrategyRegistry};
    pub use crate::simulation::{Simulation, SimConfig};
    pub use crate::network::Packet;
    pub use crate::metrics::MetricsSnapshot;
}