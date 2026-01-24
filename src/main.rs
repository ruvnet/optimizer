//! RuVector Memory Optimizer - Intelligent memory management for Windows

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

use core::config::OptimizerConfig;
use core::optimizer::IntelligentOptimizer;
use windows::memory::WindowsMemoryOptimizer;
use windows::safety::{SafetyConfig, SafetyGuard};
use bench::runner::BenchmarkRunner;
use monitor::dashboard::render_dashboard;

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
    },
    
    /// Show real-time dashboard
    Dashboard,
    
    /// Show configuration
    Config,

    /// Launch system tray icon
    Tray,

    /// Show CPU/SIMD capabilities
    Cpu,
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
        
        Commands::Bench { iterations } => {
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
    }

    Ok(())
}
