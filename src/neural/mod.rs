//! Neural decision engine using RuVector capabilities

#[cfg(target_os = "windows")]
pub mod engine;
pub mod hnsw_patterns;
#[cfg(target_os = "windows")]
pub mod attention;
#[cfg(target_os = "windows")]
pub mod ewc_learner;
