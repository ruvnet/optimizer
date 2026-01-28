//! RuVector Memory Optimizer - macOS CLI
//!
//! Cross-platform memory optimizer for macOS with Apple Silicon support.

// Stub main for non-macOS platforms - must be first and unconditional
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This binary is macOS-only.");
    eprintln!("On Windows, please use the main ruvector-memopt binary:");
    eprintln!("  cargo run --release --bin ruvector-memopt");
    std::process::exit(1);
}

// All macOS-specific code below
#[cfg(target_os = "macos")]
mod macos_impl {
    use clap::{Parser, Subcommand};
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    pub mod core {
        pub use crate::core::*;
    }
    pub mod macos {
        pub use crate::macos::*;
    }
    pub mod neural {
        pub use crate::neural::*;
    }
    pub mod bench {
        pub use crate::bench::*;
    }
    pub mod accel {
        pub use crate::accel::*;
    }
    pub mod algorithms {
        pub use crate::algorithms::*;
    }
    pub mod platform {
        pub use crate::platform::*;
    }
    pub mod dashboard {
        pub use crate::dashboard::*;
    }
    pub mod monitor {
        pub use crate::monitor::*;
    }
    pub mod security {
        pub use crate::security::*;
    }
    pub mod apps {
        pub use crate::apps::*;
    }

    use crate::macos::memory::MacMemoryOptimizer;
    use crate::apps::{BrowserOptimizer, ElectronManager, DockerManager, LeakDetector, SmartSuggestions};
    use crate::macos::safety::{SafetyConfig, SafetyGuard};
    use crate::bench::advanced::AdvancedBenchmarkRunner;

    #[derive(Parser)]
    #[command(name = "ruvector-memopt")]
    #[command(about = "Intelligent memory optimizer for macOS", long_about = None)]
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

        /// Run benchmarks
        Bench {
            #[arg(short, long, default_value = "100")]
            iterations: usize,

            /// Run advanced algorithm benchmarks (MinCut, PageRank, Sketch, Spectral)
            #[arg(long)]
            advanced: bool,
        },

        /// Show configuration
        Config,

        /// Launch menu bar app
        Tray,

        /// Show CPU/SIMD capabilities
        Cpu,

        /// Analyze processes with PageRank priority scoring
        Pagerank {
            #[arg(short, long, default_value = "10")]
            top: usize,
        },

        /// Show process clusters (MinCut analysis)
        Clusters {
            #[arg(short, long, default_value = "5")]
            max: usize,
        },

        /// Show browser memory usage (Chrome, Firefox, Safari, Edge, Arc, Brave)
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

    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        // Initialize logging
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::INFO)
            .with_target(false)
            .finish();
        tracing::subscriber::set_global_default(subscriber)?;

        let cli = Cli::parse();

        match cli.command {
            Commands::Status => {
                let status = MacMemoryOptimizer::get_memory_status()?;
                println!("Memory Status:");
                println!("  Total:     {:.0} MB", status.total_physical_mb);
                println!("  Available: {:.0} MB", status.available_physical_mb);
                println!("  Used:      {:.0} MB", status.used_physical_mb());
                println!("  Load:      {}%", status.memory_load_percent);
                println!("  Swap:      {:.0}/{:.0} MB",
                    status.total_swap_mb - status.available_swap_mb,
                    status.total_swap_mb);
                println!("  Pressure:  {} ({})",
                    match status.pressure_level {
                        0 => "Normal",
                        1 => "WARN",
                        2 => "CRITICAL",
                        3 => "URGENT",
                        _ => "EXTREME",
                    },
                    if status.is_apple_silicon { "Apple Silicon" } else { "Intel" }
                );
            }

            Commands::Optimize { aggressive, dry_run } => {
                let mut safety = SafetyGuard::new(SafetyConfig {
                    dry_run,
                    ..Default::default()
                });

                let status = MacMemoryOptimizer::get_memory_status()?;

                if let Err(e) = safety.check_safe(status.available_physical_mb) {
                    println!("Safety check failed: {}", e);
                    return Ok(());
                }

                if dry_run {
                    println!("DRY RUN - No changes will be made");
                    println!("Would run: {} optimization", if aggressive { "aggressive (purge)" } else { "standard (madvise hints)" });
                    return Ok(());
                }

                let optimizer = MacMemoryOptimizer::new();
                let result = optimizer.optimize(aggressive)?;

                safety.record_attempt(result.freed_mb >= 0.0);

                println!("Optimization complete:");
                println!("  Method:    {:?}", result.method);
                println!("  Freed:     {:.1} MB", result.freed_mb);
                println!("  Before:    {:.1} MB available", result.before_available_mb);
                println!("  After:     {:.1} MB available", result.after_available_mb);
                println!("  Duration:  {} ms", result.duration_ms);
            }

            Commands::Bench { iterations, advanced } => {
                if advanced {
                    println!("Running advanced RuVector algorithm benchmarks ({} iterations)...", iterations);

                    let runner = AdvancedBenchmarkRunner::new(iterations);
                    let suite = runner.run_all();
                    suite.print_summary();
                } else {
                    println!("Running basic memory benchmarks ({} iterations)...", iterations);

                    // Basic memory operation benchmark
                    let mut times = Vec::with_capacity(iterations);
                    for _ in 0..iterations {
                        let start = std::time::Instant::now();
                        let _ = MacMemoryOptimizer::get_memory_status();
                        times.push(start.elapsed().as_micros());
                    }

                    let avg = times.iter().sum::<u128>() as f64 / times.len() as f64;
                    let min = times.iter().min().unwrap_or(&0);
                    let max = times.iter().max().unwrap_or(&0);

                    println!("\nMemory Status Query:");
                    println!("  Avg: {:.2}µs", avg);
                    println!("  Min: {}µs", min);
                    println!("  Max: {}µs", max);

                    println!("\nTip: Run with --advanced for MinCut, PageRank, Sketch, and Spectral benchmarks");
                }
            }

            Commands::Config => {
                println!("macOS Configuration:");
                println!("  Apple Silicon: {}", if cfg!(target_arch = "aarch64") { "Yes" } else { "No" });
                println!("  Sudo Access:   {}", if unsafe { libc::geteuid() == 0 } { "Yes" } else { "No" });
                println!("\nOptimization Methods:");
                println!("  - purge (requires sudo): Clear file system caches");
                println!("  - madvise hints: Suggest memory cleanup to kernel");
                println!("\nFor full optimization, run with sudo:");
                println!("  sudo ruvector-memopt optimize --aggressive");
            }

            Commands::Tray => {
                println!("Starting menu bar app...");
                let tray_app = crate::macos::tray::MacTrayApp::new();
                if let Err(e) = tray_app.run() {
                    eprintln!("Tray error: {}", e);
                }
            }

            Commands::Cpu => {
                let caps = crate::accel::CpuCapabilities::detect();
                caps.print_report();

                println!("\nRunning quick SIMD benchmark...");
                let simd = crate::accel::SimdOptimizer::new();
                let (scalar, simd_time, speedup) = simd.benchmark(1024, 10000);
                println!("  Scalar time:  {:.4}s", scalar);
                println!("  SIMD time:    {:.4}s", simd_time);
                println!("  Speedup:      {:.2}x", speedup);
            }

            Commands::Pagerank { top } => {
                println!("Analyzing processes with PageRank...\n");

                let mut system = sysinfo::System::new_all();
                system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

                let mut pagerank = crate::algorithms::ProcessPageRank::new();
                pagerank.compute(&system);

                let candidates = pagerank.get_trim_candidates(top);
                let critical = pagerank.get_critical_processes(top);

                println!("LOW PRIORITY (trim candidates):");
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

                println!("\nHIGH PRIORITY (preserve):");
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

                let mut clusterer = crate::algorithms::MinCutClusterer::new();
                clusterer.build_graph(&system);

                let clusters = clusterer.find_clusters(max);
                let stats = clusterer.stats();

                println!("Found {} clusters from {} processes ({} edges)\n",
                    clusters.len(), stats.total_processes, stats.total_edges);

                for cluster in &clusters {
                    println!("Cluster {} ({} processes, {:.1} MB, connectivity: {:.2})",
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
                        std::thread::sleep(std::time::Duration::from_secs(interval));
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
}

// Module declarations needed for both platforms
#[cfg(target_os = "macos")]
mod core;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod neural;
#[cfg(target_os = "macos")]
mod bench;
#[cfg(target_os = "macos")]
mod accel;
#[cfg(target_os = "macos")]
mod algorithms;
#[cfg(target_os = "macos")]
mod platform;
#[cfg(target_os = "macos")]
mod dashboard;
#[cfg(target_os = "macos")]
mod monitor;
#[cfg(target_os = "macos")]
mod security;
#[cfg(target_os = "macos")]
mod apps;

#[cfg(target_os = "macos")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    macos_impl::run().await
}
