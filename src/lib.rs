//! RuVector Memory Optimizer
//!
//! An intelligent cross-platform memory optimizer that leverages RuVector neural
//! capabilities for smart optimization decisions. Supports both Windows and Linux.
//!
//! ## Features
//!
//! - **Neural Decision Engine**: GNN-based learning for optimal timing
//! - **Pattern Recognition**: HNSW-indexed patterns for fast lookup
//! - **Adaptive Strategy**: MinCut control for mode switching
//! - **Anti-Forgetting**: EWC prevents losing good strategies
//! - **Real-time Monitoring**: Live metrics dashboard
//! - **Cross-Platform**: Windows service and Linux daemon support
//! - **Security**: Privilege management and input validation
//!
//! ## Platform Support
//!
//! - **Windows**: Win32 APIs for memory management, Windows Service support
//! - **Linux**: /proc filesystem, madvise() syscalls, systemd integration
//!
//! ## Safety
//!
//! - Protected process list prevents system instability
//! - Memory floor ensures minimum available RAM
//! - Rate limiting prevents over-optimization
//! - Dry-run mode for testing

pub mod core;
pub mod windows;
pub mod neural;
pub mod bench;
pub mod monitor;
pub mod security;
pub mod algorithms;
pub mod dashboard;

// Platform-specific modules
#[cfg(target_os = "linux")]
pub mod platform;

// Re-exports
#[cfg(target_os = "linux")]
pub use platform::linux::{
    LinuxMemoryOptimizer, MemoryError as LinuxMemoryError, ProcessMemoryInfo as LinuxProcessMemoryInfo,
    SystemMemoryInfo as LinuxSystemMemoryInfo,
};
pub use core::config::OptimizerConfig;
pub use core::optimizer::IntelligentOptimizer;
pub use neural::engine::NeuralDecisionEngine;
pub use monitor::realtime::RealtimeMonitor;
pub use windows::safety::{SafetyConfig, SafetyGuard};
pub use security::privileges::PrivilegeManager;
pub use algorithms::{MinCutClusterer, ProcessPageRank, CountMinSketch, SpectralAnalyzer};
pub use bench::{AdvancedBenchmarkRunner, BenchmarkSuite};
pub use dashboard::{DashboardServer, DashboardData};
pub mod accel;
pub mod tray;

// AI Mode - optional GPU/VRAM management and AI workload optimization
#[cfg(feature = "ai")]
pub mod ai;
