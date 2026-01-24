//! Real-time dashboard for memory monitoring
//!
//! Provides data structures and API for WASM-based browser dashboard
//! or native terminal dashboard.

pub mod data;
pub mod server;

pub use data::{DashboardData, DashboardUpdate, SystemMetrics};
pub use server::DashboardServer;
