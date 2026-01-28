//! Real-time monitoring

#[cfg(target_os = "windows")]
pub mod realtime;
#[cfg(target_os = "windows")]
pub mod dashboard;
