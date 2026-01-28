//! Real-time dashboard for memory monitoring
//!
//! Provides data structures and API for WASM-based browser dashboard
//! or native terminal dashboard.

pub mod data;
#[cfg(target_os = "windows")]
pub mod server;

pub use data::{DashboardData, DashboardUpdate, SystemMetrics};
#[cfg(target_os = "windows")]
pub use server::DashboardServer;
