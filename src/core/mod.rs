//! Core optimizer logic

pub mod config;
#[cfg(target_os = "windows")]
pub mod optimizer;
#[cfg(target_os = "windows")]
pub mod patterns;
pub mod process_scorer;
