//! RuVector Memory Optimizer
//!
//! An intelligent cross-platform memory optimizer that leverages RuVector neural
//! capabilities for smart optimization decisions.
//!
//! ## Features
//!
//! - **Neural Decision Engine**: GNN-based learning for optimal timing
//! - **Pattern Recognition**: HNSW-indexed patterns for fast lookup
//! - **Adaptive Strategy**: MinCut control for mode switching
//! - **Anti-Forgetting**: EWC prevents losing good strategies
//! - **Real-time Monitoring**: Live metrics dashboard
//! - **Cross-Platform**: Windows and macOS support
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
pub mod platform;

// Platform-specific modules
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

// Re-exports - core functionality
pub use core::config::OptimizerConfig;
pub use core::optimizer::IntelligentOptimizer;
pub use neural::engine::NeuralDecisionEngine;
pub use monitor::realtime::RealtimeMonitor;
pub use security::privileges::PrivilegeManager;
pub use algorithms::{MinCutClusterer, ProcessPageRank, CountMinSketch, SpectralAnalyzer};
pub use bench::{AdvancedBenchmarkRunner, BenchmarkSuite};
pub use dashboard::{DashboardServer, DashboardData};

// Platform-agnostic re-exports for safety types
#[cfg(target_os = "windows")]
pub use windows::safety::{SafetyConfig, SafetyGuard, SafetyStats};

#[cfg(target_os = "macos")]
pub use macos::safety::{SafetyConfig, SafetyGuard, SafetyStats};

// Platform-agnostic type aliases
pub use platform::{MemoryOptimizer, MemoryStatus, OptimizationResult};

pub mod accel;
pub mod tray;

// AI Mode - optional GPU/VRAM management and AI workload optimization
#[cfg(feature = "ai")]
pub mod ai;
