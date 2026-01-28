//! RuVector Memory Optimizer
//!
//! An intelligent memory optimizer for Windows that leverages RuVector neural
//! capabilities for smart optimization decisions.
//!
//! ## Features
//!
//! - **Neural Decision Engine**: GNN-based learning for optimal timing
//! - **Pattern Recognition**: HNSW-indexed patterns for fast lookup
//! - **Adaptive Strategy**: MinCut control for mode switching
//! - **Anti-Forgetting**: EWC prevents losing good strategies
//! - **Real-time Monitoring**: Live metrics dashboard
//! - **Windows Service**: Background service support
//! - **Security**: Privilege management and input validation
//!
//! ## Safety
//!
//! - Protected process list prevents system instability
//! - Memory floor ensures minimum available RAM
//! - Rate limiting prevents over-optimization
//! - Dry-run mode for testing

pub mod core;
pub mod windows;
pub mod platform;
pub mod neural;
pub mod bench;
pub mod monitor;
pub mod security;
pub mod algorithms;
pub mod dashboard;

// Re-exports
pub use core::config::OptimizerConfig;
pub use core::optimizer::IntelligentOptimizer;
pub use neural::engine::NeuralDecisionEngine;
pub use monitor::realtime::RealtimeMonitor;
pub use windows::safety::{SafetyConfig, SafetyGuard};
pub use security::privileges::PrivilegeManager;
pub use algorithms::{MinCutClusterer, ProcessPageRank, CountMinSketch, SpectralAnalyzer};
pub use bench::{AdvancedBenchmarkRunner, BenchmarkSuite};
pub use dashboard::{DashboardServer, DashboardData};
pub use platform::{
    MemoryInfo, OptimizationReport, PlatformError, PlatformResult,
    PlatformMemoryManager, PlatformProcessManager, PlatformSystemInfo,
    PlatformPerformanceUtils, Platform, ProcessInfo, SystemDetails,
    create_platform,
};
pub mod accel;

#[cfg(feature = "desktop")]
pub mod tray;
pub mod browser;

// AI Mode - optional GPU/VRAM management and AI workload optimization
#[cfg(feature = "ai")]
pub mod ai;
