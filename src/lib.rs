//! RuVector Memory Optimizer
//!
//! An intelligent cross-platform memory optimizer that leverages RuVector neural
//! capabilities for smart optimization decisions.
//!
//! ## Platforms
//!
//! - **Windows**: Full optimization via Win32 APIs
//! - **macOS**: Memory pressure hints, purge command, Apple Silicon support
//!
//! ## Features
//!
//! - **Neural Decision Engine**: GNN-based learning for optimal timing
//! - **Pattern Recognition**: HNSW-indexed patterns for fast lookup
//! - **Adaptive Strategy**: MinCut control for mode switching
//! - **Anti-Forgetting**: EWC prevents losing good strategies
//! - **Real-time Monitoring**: Live metrics dashboard
//! - **System Tray/Menu Bar**: Background optimization
//! - **Security**: Privilege management and input validation
//!
//! ## Safety
//!
//! - Protected process list prevents system instability
//! - Memory floor ensures minimum available RAM
//! - Rate limiting prevents over-optimization
//! - Dry-run mode for testing

pub mod core;
pub mod neural;
pub mod bench;
pub mod monitor;
pub mod security;
pub mod algorithms;
pub mod dashboard;
pub mod accel;
pub mod platform;

// Platform-specific modules
#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub mod tray;
#[cfg(target_os = "windows")]
pub mod browser;

#[cfg(target_os = "macos")]
pub mod macos;

// Application-specific optimization (cross-platform)
pub mod apps;

// Re-exports - Core
pub use core::config::OptimizerConfig;
#[cfg(target_os = "windows")]
pub use core::optimizer::IntelligentOptimizer;
#[cfg(target_os = "windows")]
pub use neural::engine::NeuralDecisionEngine;
#[cfg(target_os = "windows")]
pub use monitor::realtime::RealtimeMonitor;
pub use algorithms::{MinCutClusterer, ProcessPageRank, CountMinSketch, SpectralAnalyzer};
pub use bench::{AdvancedBenchmarkRunner, BenchmarkSuite};
pub use dashboard::DashboardData;
#[cfg(target_os = "windows")]
pub use dashboard::DashboardServer;

// Platform-specific re-exports
#[cfg(target_os = "windows")]
pub use windows::safety::{SafetyConfig, SafetyGuard};
#[cfg(target_os = "windows")]
pub use security::privileges::PrivilegeManager;

#[cfg(target_os = "macos")]
pub use macos::safety::{SafetyConfig, SafetyGuard};

// App-specific optimization re-exports
pub use apps::{
    BrowserOptimizer, ElectronManager, DockerManager, LeakDetector, SmartSuggestions,
    AppCategory, AppInfo, OptimizationAction, OptimizationResult,
};

// AI Mode - optional GPU/VRAM management and AI workload optimization
#[cfg(feature = "ai")]
pub mod ai;
