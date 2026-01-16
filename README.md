<div align="center">
  <img src="./flocknet.svg" alt="FlockNet Logo" width="300"/>

  # FlockNet

  **Easy 2 Use AQM network simulation framework for benchmarking AQM strategies in swarm network environments. Written in Rust (with Tokio for async IO)**

  [![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
  [![Rust](https://img.shields.io/badge/rust-1.89+-orange.svg)](https://www.rust-lang.org/)

</div>

## Overview & Background

FlockNet simulates network topologies where software agents send packets to servers through configurable buffer strategies. It's built for research and comparison of different Active Queue Management (AQM) algorithms, with a focus on swarm network environments.

I'm developing this as part of my high school research article on viable AQM algorithms for swarm networks. The project's evolving alongside the article (target finish: April 2025!).

## Features

### Simulation Engine
- TCP-based packet transmission with persistent connections
- Configurable buffer sizes and bandwidth limits

### Traffic Patterns
- **Constant**: Fixed packet rate
- **Bursty**: Periodic bursts with configurable size
- **Poisson**: Exponentially distributed inter-arrival times
- **Peak Traffic**: Base rate with configurable peak periods

### AQM Strategies
- **Drop-Tail & FIFO**: Basic static queue management
- **RED**: Random Early Detection with EWMA
- **Adaptive RED**: Dynamic parameter adjustment based on queue state
- **BLUE**: Queue management based on packet loss and link idle events
- **CoDel**: Controlled Delay with control law dropping
- **PIE**: Proportional Integral Enhanced with burst allowance
- **FQ-CoDel**: Flow queuing with per-flow CoDel (1024 flow hash buckets)

### Metrics and Analysis
- Live metrics collection with configurable snapshots
- Throughput, latency, packet loss, and queue length tracking
- Jitter calculation with statistical analysis
- CSV export for raw data
- JSON export for structured results

## Installation

### Prerequisites
- Rust
- Cargo

### Build from Source
```bash
git clone https://github.com/Servus-Altissimi/flocknet.git 
cd flocknet
cargo build --release
```

## Usage

### Command Line Interface

List available strategies:
```bash
cargo run --release list
```

Run a single simulation:
```bash
cargo run --release run --strategy fq-codel --agents 256 --servers 4 --duration 120
```

Compare multiple strategies:
```bash
cargo run --release compare --strategies "drop-tail,red,codel,pie,fq-codel" --agents 256 --duration 120 --repetitions 5 
```

Analyze existing results:
```bash
cargo run -- analyze results
```

### Command Reference

#### `run`: Single Simulation
| Flag | Description | Default |
|------|-------------|---------|
| `--strategy, -s` | AQM strategy name | `drop-tail` |
| `--agents, -n` | Number of agents | `256` |
| `--servers, -S` | Number of servers | `4` |
| `--duration, -d` | Simulation duration (seconds) | `256` |
| `--traffic, -t` | Traffic pattern | `peak` |
| `--base-rate` | Base packet rate (pps) | `50` |
| `--peak-rate` | Peak packet rate (pps) | `500` |
| `--peak-duration` | Peak period duration (seconds) | `10` |

#### `compare`: Strategy Comparison
| Flag | Description | Default |
|------|-------------|---------|
| `--strategies, -s` | Comma-separated strategy list | All built-in strategies |
| `--agents, -n` | Number of agents | `256` |
| `--servers, -S` | Number of servers | `4` |
| `--duration, -d` | Simulation duration (seconds) | `256` |
| `--repetitions, -r` | Number of runs per strategy | `3` |
| `--latex` | Generate LaTeX exports (Dutch) | `false` |

#### `export`: LaTeX Generation (Dutch)
| Argument | Description | Default |
|----------|-------------|---------|
| `input` | JSON results file or directory | a valid file |
| `--output, -o` | Output file path | `results/comparison.tex` |
| `--format, -f` | Export format | `all` |

Format options: `table`, `detailed`, `figure`, `all`

#### `analyze`: Results Analysis
| Argument | Description | Default |
|----------|-------------|---------|
| `path` | Directory containing result files | `results` |

Right now it's a bit of a mess.

### Output Files

Results get saved to `results/` with timestamps:

- `{name}_{timestamp}.csv` - Raw metrics
- `{name}_{timestamp}_analysis.json` - Statistical analysis
- `{name}_{timestamp}_plot.dat` - Time series data for plotting
- `comparison_{timestamp}.json` - Multi-strategy comparison
- `comparison_{timestamp}_table.tex` - LaTeX comparison table
- `comparison_{timestamp}_detailed.tex` - LaTeX detailed analysis
- `comparison_{timestamp}_figure.tex` - LaTeX bar chart (WIP)

## Architecture

### Core Components
- **Agent**: Generates packets according to traffic patterns, maintains persistent TCP connections to servers
- **Server**: Receives packets, applies buffer strategy, processes queue with configurable bandwidth
- **Strategy**: Implements AQM algorithm via enqueue/dequeue hooks
- **MetricsCollector**: Collects metrics with snapshot support
- **Simulation**: Orchestrates agents, servers, and lifecycles

### Packet Flow
  1. Agent generates packet based on traffic pattern
  2. Packet gets serialized with `bincode` and sent over TCP
  3. Server applies strategy decision (accept/drop)
  4. Accepted packets get enqueued to buffer
  5. Server processes queue at configured bandwidth rate
  6. Metrics get collected throughout

### Timing and Synchronization
- Agents use per-pattern interval timers for packet generation
- Servers process queues at bandwidth-derived packet intervals
- Metrics snapshots captured at 1-second intervals
- Strategy `update()` called approximately every 100ms
- Sojourn time calculated using serializable `SystemTime` timestamps

## Known Limitations 
- TCP overhead not accounted for in metrics
- Single-node simulation only (no distributed mode)
- Sojourn time estimates in some strategies assume fixed packet size (working on fixing this)
- No support for variable packet sizes within a simulation
- Port binding requires brief delays for OS cleanup between runs, which can feel inconsistent

## Contributing
This is research software under active development. Bug reports, suggestions, and contributions are welcome. The codebase has comments marking areas for improvement and known issues. I'll keep improving it as my research continues.

## Acknowledgments
Created as part of research into swarm network queue management. Implements algorithms from:
- Floyd & Jacobson (1993) - Random Early Detection
- Floyd et al. (2001) - Adaptive RED
- Feng et al. (2001) - BLUE
- Nichols & Jacobson (2012) - CoDel
- Pan et al. (2013) - PIE
- Hoiland-Jorgensen et al. (2018) - FQ-CoDel

## Citation
If you use FlockNet in your research like I do:

```bibtex
@software{flocknet2025,
  title = {FlockNet},
  author = {Servus Altissimi},
  year = {2025},
  url = {https://github.com/Servus-Altissimi/flocknet}
}
```






## Implementing Custom Strategies (Draft)
Create a new file in `src/strategies/` implementing the `Strategy` trait:

```rust
use super::{Action, Strategy};
use crate::network::Packet;

#[derive(Debug, Clone)]
pub struct MyStrategy {
    buffer_size: usize,
    threshold: f64,
    // Add state variables
}

impl MyStrategy {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer_size,
            threshold: 0.8,
        }
    }
}

impl Strategy for MyStrategy {
    fn on_enqueue(&mut self, packet: &Packet, queue_len: usize) -> Action {
        // Implement enqueue logic
        let utilization = queue_len as f64 / self.buffer_size as f64;
        
        if utilization > self.threshold {
            Action::Drop
        } else {
            Action::Accept
        }
    }
    
    fn on_dequeue(&mut self, queue_len: usize) {
        // Optional: Update state after dequeue
    }
    
    fn update(&mut self, queue_len: usize, avg_sojourn_ms: f64) {
        // Optional: Periodic state updates (~100ms intervals)
    }
    
    fn name(&self) -> &str {
        "MyStrategy"
    }
    
    fn reset(&mut self) {
        // Reset state between simulations
    }
    
    fn clone_box(&self) -> Box<dyn Strategy> {
        Box::new(self.clone())
    }
}
```
