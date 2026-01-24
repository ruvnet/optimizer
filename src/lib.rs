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
pub mod neural;
pub mod bench;
pub mod monitor;
pub mod security;

// Re-exports
pub use core::config::OptimizerConfig;
pub use core::optimizer::IntelligentOptimizer;
pub use neural::engine::NeuralDecisionEngine;
pub use monitor::realtime::RealtimeMonitor;
pub use windows::safety::{SafetyConfig, SafetyGuard};
pub use security::privileges::PrivilegeManager;
pub mod accel;
pub mod tray;
