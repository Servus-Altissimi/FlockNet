//  ,---.,--.             ,--.                     ,--.   
// /  .-'|  | ,---.  ,---.|  |,-. ,--,--,  ,---. ,-'  '-. 
// |  `-,|  || .-. || .--'|     / |      \| .-. :'-.  .-' 
// |  .-'|  |' '-' '\ `--.|  \  \ |  ||  |\   --.  |  |   
// `--'  `--' `---'  `---'`--'`--'`--''--' `----'  `--'   

// This is my largest completed project to-date. Which means it will likely be riddled with flaws and large oversights.
// While I do believe that this research tool is complete for benchmarking AQM's and developing new solutions.
// I don't believe that there's no room for improvement.

// Copyright 2025 Servus Altissimi (Pseudonym)

// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
// The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.                                                                          

use flocknet::prelude::*;
use flocknet::simulation::config::SimConfig;
use flocknet::agent::TrafficPattern;
use flocknet::strategies::StrategyRegistry;
use flocknet::metrics::analyzer;
use flocknet::simulation::Simulation;

use clap::{Parser, Subcommand};
use anyhow::Result;
use std::time::{Duration, Instant};
use tracing::{info, Level};

use tracing_subscriber;

// unused rn
const MAX_REASONABLE_SOJOURN_MS: f64 = 30_000.0; 
const BURST_PACKET_DELAY_MICROS: u64 = 100;
const DEBUG_PACKET_COUNT: u64 = 10;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(short, long, default_value = "drop-tail")]
        strategy: String,
        #[arg(short = 'n', long, default_value_t = 256)]
        agents: u32,
        #[arg(short = 'S', long, default_value_t = 4)]
        servers: u32,
        #[arg(short, long, default_value_t = 256)]
        duration: u64,
        #[arg(short, long, default_value = "peak")]
        traffic: String,
        #[arg(long, default_value_t = 50.0)]
        base_rate: f64,
        #[arg(long, default_value_t = 500.0)]
        peak_rate: f64,
        #[arg(long, default_value_t = 10.0)]
        peak_duration: f64,
    },
    
    Compare {
        #[arg(short, long, default_value = "drop-tail,red,adaptive-red,blue,codel,pie,fq-codel")] // TODO: Make this read a global table
        strategies: String,
        #[arg(short = 'n', long, default_value_t = 256)]
        agents: u32,
        #[arg(short = 'S', long, default_value_t = 4)]
        servers: u32,
        #[arg(short, long, default_value_t = 256)]
        duration: u64,
        #[arg(short, long, default_value_t = 3)]
        repetitions: u32,
        #[arg(long)]
        latex: bool,
    },
    
    Export {
        input: String,
        #[arg(short, long, default_value = "results/comparison.tex")]
        output: String,
        #[arg(short, long, default_value = "all")]
        format: String,
    },
    
    Analyze {
        #[arg(default_value = "results")]
        path: String,
    },
    
    List,
}

#[tokio::main]
async fn main() -> Result<()> {
    let program_start = Instant::now(); // Global timer for end time.
    
    let cli = Cli::parse();
    
    let level = if cli.verbose { Level::DEBUG } else { Level::INFO };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .init();
    
    match cli.command {
        Commands::Run {
            strategy,
            agents,
            servers,
            duration,
            traffic,
            base_rate,
            peak_rate,
            peak_duration,
        } => {
            run_single_simulation(
                strategy,
                agents,
                servers,
                duration,
                traffic,
                base_rate,
                peak_rate,
                peak_duration,
            ).await?;
        }
        
        Commands::Compare {
            strategies,
            agents,
            servers,
            duration,
            repetitions,
            latex,
        } => {
            compare_strategies(
                strategies,
                agents,
                servers,
                duration,
                repetitions,
                latex,
                program_start,
            ).await?;
        }
        
        Commands::Export { input, output, format } => {
            export_latex(&input, &output, &format)?;
        }
        
        Commands::Analyze { path } => {
            analyze_results(&path)?;
        }
        
        Commands::List => {
            println!("\nAvailable Buffer Strategies");
            
            for strategy in StrategyRegistry::global().list() {
                println!("  - {}", strategy);
            }
            
            println!("\nUsage: cargo run -- run --strategy <name>");
            println!("Example: cargo run -- run --strategy fq-codel\n");
        }
    }
    
    let total_time = program_start.elapsed();
    info!("Total runtime: {:.2}s", total_time.as_secs_f64());
    
    Ok(())
}

async fn run_single_simulation(
    strategy_name: String,
    agents: u32,
    servers: u32,
    duration: u64,
    traffic: String,
    base_rate: f64,
    peak_rate: f64,
    peak_duration: f64,
) -> Result<()> {
    let traffic_pattern = parse_traffic_pattern(
        &traffic,
        base_rate,
        peak_rate,
        peak_duration,
    )?;
    
    let config = SimConfig {
        name: format!("{}_{}", strategy_name, traffic),
        strategy_name,
        num_agents: agents,
        num_servers: servers,
        duration: Duration::from_secs(duration),
        buffer_size: 1024,
        bandwidth_bps: 100_000_000,
        traffic_pattern,
    };
    
    info!("FlockNet: Single Run");
    
    let mut sim = Simulation::new(config);
    sim.run().await?;
    
    Ok(())
}

async fn compare_strategies(
    strategies_str: String,
    agents: u32,
    servers: u32,
    duration: u64,
    repetitions: u32,
    export_latex: bool,
    global_start: Instant,
) -> Result<()> {
    let strategy_names: Vec<&str> = strategies_str.split(',').map(|s| s.trim()).collect();
    
    info!("FlockNet: Comparison");
    info!("");
    info!("Strategies: {}", strategy_names.join(", "));
    info!("Repetitions: {}", repetitions);
    info!("Duration per test: {}s", duration);
    info!("");
    
    let mut all_reports = Vec::new();
    let total_tests = strategy_names.len() * repetitions as usize;
    let mut completed = 0;
    
    for strategy_name in strategy_names {
        info!("Testing: {}", strategy_name);
        
        let mut strategy_reports = Vec::new();
        
        for rep in 1..=repetitions {
            completed += 1;
            let elapsed = global_start.elapsed();
            info!("  [{}] Run {}/{} - Elapsed: {:.1}s", 
                  format_time(elapsed), rep, repetitions, elapsed.as_secs_f64());
            
            let config = SimConfig {
                name: format!("{}_{}", strategy_name, rep),
                strategy_name: strategy_name.to_string(),
                num_agents: agents,
                num_servers: servers,
                duration: Duration::from_secs(duration),
                buffer_size: 1024,
                bandwidth_bps: 100_000_000,
                traffic_pattern: TrafficPattern::PeakTraffic {
                    base_rate: 50.0,
                    peak_rate: 500.0,
                    peak_duration_s: 10.0,
                },
            };
            
            let mut sim = Simulation::new(config);
            sim.run().await?;
            
            let snapshots = sim.metrics.get_snapshots();
            let report = analyzer::analyze(&snapshots, strategy_name);
            strategy_reports.push(report);
        }
        
        let avg_report = average_reports(&strategy_reports);
        all_reports.push(avg_report);
        
        info!("");
    }
    
    comparison_table(&all_reports);
    
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let comparison_path = format!("results/comparison_{}.json", timestamp);
    std::fs::write(
        &comparison_path,
        serde_json::to_string_pretty(&all_reports)?
    )?;
    info!("Comparison saved to: {}", comparison_path);
    
    if export_latex {
        let latex_table_path = format!("results/comparison_{}_table.tex", timestamp);
        let latex_detailed_path = format!("results/comparison_{}_detailed.tex", timestamp);
        let latex_figure_path = format!("results/comparison_{}_figure.tex", timestamp);
        
        analyzer::export_latex_table(&all_reports, &latex_table_path)?;
        info!("LaTeX table exported to: {}", latex_table_path);
        
        analyzer::export_latex_detailed(&all_reports, &latex_detailed_path)?;
        info!("LaTeX detailed analysis exported to: {}", latex_detailed_path);
        
        analyzer::export_latex_figure(
            &all_reports,
            &latex_figure_path,
            "Vergelijking van gemiddelde doorvoer tussen bufferstrategieën",
            "fig:throughput_comparison"
        )?;
        info!("LaTeX figure exported to: {}", latex_figure_path);
        
        info!("");
        info!("LaTeX exports are ready. Included in your document are:");
        info!("   \\input{{{}}}", latex_table_path);
        info!("   \\input{{{}}}", latex_detailed_path);
        info!("   \\input{{{}}}", latex_figure_path);
    }
    
    Ok(())
}

fn format_time(duration: Duration) -> String {
    let secs = duration.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

fn export_latex(input: &str, output: &str, format: &str) -> Result<()> {
    use std::fs;
    use std::path::Path;
    
    info!("Exporting LaTeX from: {}", input);
    
    let input_path = Path::new(input);
    let reports = if input_path.is_file() {
        let content = fs::read_to_string(input)?;
        if input.contains("comparison") {
            serde_json::from_str::<Vec<analyzer::AnalysisReport>>(&content)?
        } else {
            vec![serde_json::from_str::<analyzer::AnalysisReport>(&content)?]
        }
    } else if input_path.is_dir() {
        let mut reports = Vec::new();
        for entry in fs::read_dir(input)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") 
                && path.to_string_lossy().contains("analysis") {
                let content = fs::read_to_string(&path)?;
                if let Ok(report) = serde_json::from_str::<analyzer::AnalysisReport>(&content) {
                    reports.push(report);
                }
            }
        }
        reports
    } else {
        anyhow::bail!("Input must exist and be valid");
    };
    
    if reports.is_empty() {
        anyhow::bail!("No analysis reports found");
    }
    
    match format.to_lowercase().as_str() {
        "table" => {
            analyzer::export_latex_table(&reports, output)?;
            info!("LaTeX table exported to: {}", output);
        }
        "detailed" => {
            analyzer::export_latex_detailed(&reports, output)?;
            info!("LaTeX detailed analysis exported to: {}", output);
        }
        "figure" => {
            analyzer::export_latex_figure(
                &reports,
                output,
                "Vergelijking van bufferstrategieën",
                "fig:strategy_comparison"
            )?;
            info!("LaTeX figure exported to: {}", output);
        }
        "all" => {
            let base = output.trim_end_matches(".tex");
            
            let table_path = format!("{}_table.tex", base);
            analyzer::export_latex_table(&reports, &table_path)?;
            info!("LaTeX table exported to: {}", table_path);
            
            let detailed_path = format!("{}_detailed.tex", base);
            analyzer::export_latex_detailed(&reports, &detailed_path)?;
            info!("LaTeX detailed analysis exported to: {}", detailed_path);
            
            let figure_path = format!("{}_figure.tex", base);
            analyzer::export_latex_figure(
                &reports,
                &figure_path,
                "Vergelijking van gemiddelde doorvoer tussen bufferstrategieën",
                "fig:throughput_comparison"
            )?;
            info!("LaTeX figure exported to: {}", figure_path);
            
            info!("");
            info!("All LaTeX exports are ready! Included in your document is:");
            info!("   \\input{{{}}}", table_path);
            info!("   \\input{{{}}}", detailed_path);
            info!("   \\input{{{}}}", figure_path);
        }
        _ => anyhow::bail!("Unknown format: {}. Use: table, detailed, figure, or all", format),
    }
    
    Ok(())
}

fn analyze_results(path: &str) -> Result<()> {
    use std::fs;
    
    info!("Analyzing results in: {}", path);
    
    let entries = fs::read_dir(path)?;
    let mut reports = Vec::new();
    
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) == Some("json") 
            && path.to_string_lossy().contains("analysis") {
            let content = fs::read_to_string(&path)?;
            let report: analyzer::AnalysisReport = serde_json::from_str(&content)?;
            reports.push(report);
        }
    }
    
    if reports.is_empty() {
        info!("No analysis files found.");
        return Ok(());
    }
    
    comparison_table(&reports);
    
    Ok(())
}

fn parse_traffic_pattern(
    name: &str,
    base_rate: f64,
    peak_rate: f64,
    peak_duration: f64,
) -> Result<TrafficPattern> {
    match name.to_lowercase().as_str() {
        "constant" => Ok(TrafficPattern::Constant { rate_pps: base_rate }),
        "bursty" => Ok(TrafficPattern::Bursty {
            avg_rate_pps: base_rate,
            burst_size: 10,
        }),
        "poisson" => Ok(TrafficPattern::Poisson { lambda: base_rate }),
        "peak" => Ok(TrafficPattern::PeakTraffic {
            base_rate,
            peak_rate,
            peak_duration_s: peak_duration,
        }),
        _ => anyhow::bail!("Unknown traffic pattern: {}", name),
    }
}

fn average_reports(reports: &[analyzer::AnalysisReport]) -> analyzer::AnalysisReport {
    let n = reports.len() as f64;
    
    analyzer::AnalysisReport {
        strategy_name: reports[0].strategy_name.clone(),
        avg_throughput_mbps: reports.iter().map(|r| r.avg_throughput_mbps).sum::<f64>() / n,
        avg_latency_ms: reports.iter().map(|r| r.avg_latency_ms).sum::<f64>() / n,
        packet_loss_rate: reports.iter().map(|r| r.packet_loss_rate).sum::<f64>() / n,
        peak_queue_length: reports.iter().map(|r| r.peak_queue_length).max().unwrap_or(0),
        avg_queue_length: reports.iter().map(|r| r.avg_queue_length).sum::<f64>() / n,
        jitter_ms: reports.iter().map(|r| r.jitter_ms).sum::<f64>() / n,
    }
}

// TODO: Make this less prone to break
fn comparison_table(reports: &[analyzer::AnalysisReport]) {
    println!("\n╔═══════════════════════════════════════════════════════════════════════════════╗"); 
    println!("║                          STRATEGY COMPARISON                                  ║");
    println!("╠═══════════════╦═══════════╦═══════════╦════════════╦════════════╦═════════════╣");
    println!("║ Strategy      ║ Throughput║ Latency   ║ Loss Rate  ║ Avg Queue  ║ Jitter      ║");
    println!("║               ║ (mbps)    ║ (ms)      ║ (%)        ║ (packets)  ║ (ms)        ║");
    println!("╠═══════════════╬═══════════╬═══════════╬════════════╬════════════╬═════════════╣");
    
    for report in reports {
        println!(
            "║ {:<13} ║ {:>9.2} ║ {:>9.2} ║ {:>9.2}% ║ {:>10.1} ║ {:>11.2} ║",
            report.strategy_name,
            report.avg_throughput_mbps,
            report.avg_latency_ms,
            report.packet_loss_rate * 100.0,
            report.avg_queue_length,
            report.jitter_ms,
        );
    }
    
    println!("╚═══════════════╩═══════════╩═══════════╩════════════╩════════════╩═════════════╝\n");
    
    if let Some(best_throughput) = reports.iter().max_by(|a, b| {
        a.avg_throughput_mbps.partial_cmp(&b.avg_throughput_mbps).unwrap()
    }) {
        println!("Top Throughput: {} ({:.2} Mbps)", 
            best_throughput.strategy_name, best_throughput.avg_throughput_mbps); // TODO: Make precision a flag
    }
    
    if let Some(best_latency) = reports.iter().min_by(|a, b| {
        a.avg_latency_ms.partial_cmp(&b.avg_latency_ms).unwrap()
    }) {
        println!("Lowest Latency: {} ({:.2} ms)", 
            best_latency.strategy_name, best_latency.avg_latency_ms);
    }
    
    if let Some(best_loss) = reports.iter().min_by(|a, b| {
        a.packet_loss_rate.partial_cmp(&b.packet_loss_rate).unwrap()
    }) {
        println!("Lowest Loss: {} ({:.2}%)", 
            best_loss.strategy_name, best_loss.packet_loss_rate * 100.0);
    }
    
    println!();
}