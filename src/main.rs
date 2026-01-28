//! RuVector Memory Optimizer - Intelligent memory management for Windows
//!
//! This is the Windows-specific CLI. For macOS, see macos_main.rs.

#![cfg(target_os = "windows")]

use clap::{Parser, Subcommand};
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod core;
mod windows;
mod neural;
mod bench;
mod monitor;
mod accel;
mod tray;
mod algorithms;
mod dashboard;
mod apps;

use core::config::OptimizerConfig;
use apps::{BrowserOptimizer, ElectronManager, DockerManager, LeakDetector, SmartSuggestions};
use core::optimizer::IntelligentOptimizer;
use windows::memory::WindowsMemoryOptimizer;
use windows::safety::{SafetyConfig, SafetyGuard};
use bench::runner::BenchmarkRunner;
use bench::advanced::AdvancedBenchmarkRunner;
use monitor::dashboard::render_dashboard;
use dashboard::DashboardServer;

#[derive(Parser)]
#[command(name = "ruvector-memopt")]
#[command(about = "Intelligent memory optimizer with neural learning", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show current memory status
    Status,

    /// Run one-time optimization
    Optimize {
        #[arg(short, long)]
        aggressive: bool,

        #[arg(long)]
        dry_run: bool,
    },

    /// Start continuous optimization daemon
    Daemon {
        #[arg(short, long, default_value = "60")]
        interval: u64,
    },

    /// Run startup optimization mode
    Startup,

    /// Run benchmarks
    Bench {
        #[arg(short, long, default_value = "100")]
        iterations: usize,

        /// Run advanced algorithm benchmarks (MinCut, PageRank, Sketch, Spectral)
        #[arg(long)]
        advanced: bool,
    },

    /// Show real-time dashboard
    Dashboard,

    /// Start dashboard server (JSON API)
    DashboardServer {
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },

    /// Show configuration
    Config,

    /// Launch system tray icon
    Tray,

    /// Show CPU/SIMD capabilities
    Cpu,

    /// Analyze processes with PageRank priority scoring
    Pagerank {
        /// Number of top processes to show
        #[arg(short, long, default_value = "10")]
        top: usize,
    },

    /// Show process clusters (MinCut analysis)
    Clusters {
        /// Maximum clusters to show
        #[arg(short, long, default_value = "5")]
        max: usize,
    },

    /// Analyze memory patterns (Spectral analysis)
    Patterns {
        /// Sample duration in seconds
        #[arg(short, long, default_value = "30")]
        duration: u64,
    },

    /// Show browser memory usage (Chrome, Firefox, Edge, Arc, Brave)
    Browsers,

    /// Show Electron app memory usage (VS Code, Discord, Slack, etc.)
    Electron,

    /// Show Docker container resource usage
    Docker,

    /// Detect potential memory leaks
    Leaks {
        /// Number of samples to take
        #[arg(short, long, default_value = "10")]
        samples: usize,

        /// Interval between samples in seconds
        #[arg(short, long, default_value = "5")]
        interval: u64,
    },

    /// Show smart optimization suggestions
    Suggest,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Status => {
            let status = WindowsMemoryOptimizer::get_memory_status()?;
            println!("Memory Status:");
            println!("  Total:     {:.0} MB", status.total_physical_mb);
            println!("  Available: {:.0} MB", status.available_physical_mb);
            println!("  Used:      {:.0} MB", status.used_physical_mb());
            println!("  Load:      {}%", status.memory_load_percent);
            println!("  Pressure:  {}", 
                if status.is_critical() { "CRITICAL" }
                else if status.is_high_pressure() { "HIGH" }
                else { "Normal" }
            );
        }
        
        Commands::Optimize { aggressive, dry_run } => {
            let config = OptimizerConfig {
                aggressive_mode: aggressive,
                ..Default::default()
            };
            
            let mut safety = SafetyGuard::new(SafetyConfig {
                dry_run,
                ..Default::default()
            });
            
            let status = WindowsMemoryOptimizer::get_memory_status()?;
            
            if let Err(e) = safety.check_safe(status.available_physical_mb) {
                println!("Safety check failed: {}", e);
                return Ok(());
            }
            
            if dry_run {
                println!("DRY RUN - No changes will be made");
            }
            
            let optimizer = WindowsMemoryOptimizer::new();
            let result = optimizer.optimize(aggressive)?;
            
            safety.record_attempt(result.freed_mb >= 0.0);
            
            println!("Optimization complete:");
            println!("  Freed:     {:.1} MB", result.freed_mb);
            println!("  Trimmed:   {} processes", result.processes_trimmed);
            println!("  Duration:  {} ms", result.duration_ms);
        }
        
        Commands::Daemon { interval } => {
            info!("Starting optimization daemon (interval: {}s)", interval);
            
            let config = OptimizerConfig::default();
            let mut optimizer = IntelligentOptimizer::new(config);
            
            optimizer.run_loop(Duration::from_secs(interval)).await;
        }
        
        Commands::Startup => {
            info!("Running startup optimization");
            
            let config = OptimizerConfig {
                startup_mode: true,
                ..Default::default()
            };
            let mut optimizer = IntelligentOptimizer::new(config);
            
            match optimizer.startup_optimize().await {
                Ok(result) => {
                    println!("Startup optimization complete:");
                    println!("  Freed: {:.1} MB", result.freed_mb);
                }
                Err(e) => {
                    println!("Startup optimization failed: {}", e);
                }
            }
        }
        
        Commands::Bench { iterations, advanced } => {
            if advanced {
                println!("Running advanced RuVector algorithm benchmarks ({} iterations)...", iterations);

                let runner = AdvancedBenchmarkRunner::new(iterations);
                let suite = runner.run_all();
                suite.print_summary();
            } else {
                println!("Running benchmarks ({} iterations)...", iterations);

                let runner = BenchmarkRunner::new(iterations);
                let results = runner.run_all();

                for result in results {
                    println!("\nBenchmark: {}", result.name);
                    println!("  Iterations:  {}", result.iterations);
                    println!("  Total:       {} ms", result.total_ms);
                    println!("  Avg:         {:.3} ms", result.avg_ms);
                    println!("  Min:         {} ms", result.min_ms);
                    println!("  Max:         {} ms", result.max_ms);
                    println!("  Ops/sec:     {:.0}", result.ops_per_sec);
                }

                println!("\nTip: Run with --advanced for MinCut, PageRank, Sketch, and Spectral benchmarks");
            }
        }
        
        Commands::Dashboard => {
            println!("Starting real-time dashboard (Ctrl+C to exit)...\n");
            
            let metrics = bench::metrics::BenchmarkMetrics::new();
            
            loop {
                if let Ok(status) = WindowsMemoryOptimizer::get_memory_status() {
                    // Clear screen
                    print!("\x1B[2J\x1B[1;1H");
                    println!("{}", render_dashboard(&status, &metrics.summary()));
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
        
        Commands::Config => {
            let config = OptimizerConfig::default();
            println!("Current Configuration:");
            println!("{}", toml::to_string_pretty(&config)?);
        }

        Commands::Tray => {
            println!("Starting system tray icon...");
            let tray_app = tray::TrayApp::new();
            if let Err(e) = tray_app.run() {
                eprintln!("Tray error: {}", e);
            }
        }

        Commands::Cpu => {
            let caps = accel::CpuCapabilities::detect();
            caps.print_report();

            println!("\nRunning quick SIMD benchmark...");
            let simd = accel::SimdOptimizer::new();
            let (scalar, simd_time, speedup) = simd.benchmark(1024, 10000);
            println!("  Scalar time:  {:.4}s", scalar);
            println!("  SIMD time:    {:.4}s", simd_time);
            println!("  Speedup:      {:.2}x", speedup);
        }

        Commands::DashboardServer { port } => {
            let server = DashboardServer::new();
            server.serve(port).await?;
        }

        Commands::Pagerank { top } => {
            println!("Analyzing processes with PageRank...\n");

            let mut system = sysinfo::System::new_all();
            system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

            let mut pagerank = algorithms::ProcessPageRank::new();
            pagerank.compute(&system);

            let candidates = pagerank.get_trim_candidates(top);
            let critical = pagerank.get_critical_processes(top);

            println!("📉 LOW PRIORITY (trim candidates):");
            println!("┌─────────┬────────────────────────────┬──────────────┐");
            println!("│   PID   │ Process                    │ PageRank     │");
            println!("├─────────┼────────────────────────────┼──────────────┤");
            for (pid, score) in &candidates {
                let name = system
                    .process(sysinfo::Pid::from_u32(*pid))
                    .map(|p| p.name().to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".into());
                println!("│ {:>7} │ {:26} │ {:>12.6} │", pid, truncate(&name, 26), score);
            }
            println!("└─────────┴────────────────────────────┴──────────────┘");

            println!("\n📈 HIGH PRIORITY (preserve):");
            println!("┌─────────┬────────────────────────────┬──────────────┐");
            println!("│   PID   │ Process                    │ PageRank     │");
            println!("├─────────┼────────────────────────────┼──────────────┤");
            for (pid, score) in &critical {
                let name = system
                    .process(sysinfo::Pid::from_u32(*pid))
                    .map(|p| p.name().to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".into());
                println!("│ {:>7} │ {:26} │ {:>12.6} │", pid, truncate(&name, 26), score);
            }
            println!("└─────────┴────────────────────────────┴──────────────┘");

            let stats = pagerank.stats();
            println!("\nStatistics:");
            println!("  Processes: {}", stats.process_count);
            println!("  Mean score: {:.6}", stats.mean_score);
            println!("  Std dev:    {:.6}", stats.std_dev);
        }

        Commands::Clusters { max } => {
            println!("Analyzing process clusters with MinCut...\n");

            let mut system = sysinfo::System::new_all();
            system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

            let mut clusterer = algorithms::MinCutClusterer::new();
            clusterer.build_graph(&system);

            let clusters = clusterer.find_clusters(max);
            let stats = clusterer.stats();

            println!("Found {} clusters from {} processes ({} edges)\n",
                clusters.len(), stats.total_processes, stats.total_edges);

            for cluster in &clusters {
                println!("📦 Cluster {} ({} processes, {:.1} MB, connectivity: {:.2})",
                    cluster.id, cluster.processes.len(), cluster.total_memory_mb, cluster.connectivity);

                let trim_order = clusterer.get_trim_order(cluster);
                for (i, pid) in trim_order.iter().take(5).enumerate() {
                    let name = system
                        .process(sysinfo::Pid::from_u32(*pid))
                        .map(|p| p.name().to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".into());
                    let mem = system
                        .process(sysinfo::Pid::from_u32(*pid))
                        .map(|p| p.memory() as f64 / (1024.0 * 1024.0))
                        .unwrap_or(0.0);
                    println!("   {}. {} (PID {}) - {:.1} MB", i + 1, name, pid, mem);
                }
                if cluster.processes.len() > 5 {
                    println!("   ... and {} more", cluster.processes.len() - 5);
                }
                println!();
            }
        }

        Commands::Patterns { duration } => {
            println!("Analyzing memory patterns for {} seconds...\n", duration);

            let mut analyzer = algorithms::SpectralAnalyzer::new(duration as usize);

            for i in 0..duration {
                if let Ok(status) = WindowsMemoryOptimizer::get_memory_status() {
                    let usage = status.memory_load_percent as f64 / 100.0;
                    analyzer.add_sample(usage);

                    print!("\rSampling: {}/{} ({}%)", i + 1, duration, status.memory_load_percent);
                    std::io::Write::flush(&mut std::io::stdout())?;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }

            println!("\n\n📊 Spectral Analysis Results:\n");

            let stats = analyzer.stats();
            let rec = analyzer.get_recommendation();

            println!("Pattern Classification: {:?}", stats.classification);
            println!("Mean Usage:            {:.1}%", stats.mean * 100.0);
            println!("Variance:              {:.4}", stats.variance);
            println!("Trend:                 {:.4} ({})",
                stats.trend,
                if stats.trend > 0.01 { "increasing" }
                else if stats.trend < -0.01 { "decreasing" }
                else { "stable" });
            println!("Dominant Frequency:    bin {}", stats.dominant_frequency);

            println!("\n🎯 Recommendation:");
            println!("   Action:     {:?}", rec.action);
            println!("   Confidence: {:.0}%", rec.confidence * 100.0);
            println!("   Reason:     {}", rec.reason);
            if rec.predicted_relief_mb > 0 {
                println!("   Est. relief: {} MB", rec.predicted_relief_mb);
            }
        }

        Commands::Browsers => {
            println!("Analyzing browser memory usage...\n");
            let mut optimizer = BrowserOptimizer::new();
            optimizer.refresh();
            optimizer.print_summary();
        }

        Commands::Electron => {
            println!("Analyzing Electron app memory usage...\n");
            let mut manager = ElectronManager::new();
            manager.refresh();
            manager.print_summary();
        }

        Commands::Docker => {
            let mut manager = DockerManager::new();
            if !manager.is_available() {
                println!("Docker is not available or not running.");
                return Ok(());
            }
            manager.refresh();
            manager.print_summary();
        }

        Commands::Leaks { samples, interval } => {
            println!("Monitoring for memory leaks...");
            println!("Taking {} samples at {} second intervals\n", samples, interval);

            let mut detector = LeakDetector::new();
            detector.set_sample_interval(interval);

            for i in 0..samples {
                detector.sample();
                print!("\rSampling... {}/{}", i + 1, samples);
                std::io::Write::flush(&mut std::io::stdout()).ok();

                if i < samples - 1 {
                    tokio::time::sleep(Duration::from_secs(interval)).await;
                }
            }
            println!();

            detector.print_summary();
        }

        Commands::Suggest => {
            println!("Generating smart optimization suggestions...\n");
            let mut engine = SmartSuggestions::new();
            engine.refresh();
            engine.print_summary();
        }
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

// Stub main for non-Windows platforms
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("This binary is Windows-only.");
    eprintln!("On macOS, please build and run the macOS-specific binary:");
    eprintln!("  cargo build --release --bin ruvector-memopt-macos");
    std::process::exit(1);
}
