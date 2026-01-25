//! Linux Platform Support
//!
//! This module provides Linux-specific implementations for memory optimization,
//! process management, daemon services, GPU monitoring, and mode detection
//! using the /proc filesystem, sysfs, and libc system calls.
//!
//! ## Modules
//!
//! - **memory**: Memory optimization using madvise() and /proc APIs
//! - **process**: Process management with protected process list
//! - **daemon**: systemd service management and signal handling
//! - **gpu**: GPU monitoring for NVIDIA, AMD, and Intel
//! - **modes**: Game and focus mode detection
//!
//! ## Required Capabilities
//!
//! Some operations require elevated privileges:
//! - `drop_system_caches()`: Requires root or CAP_SYS_ADMIN
//! - `clear_refs`: Requires root or CAP_SYS_ADMIN
//! - `set_oom_score_adj()`: Requires appropriate permissions
//! - systemd service installation: Requires root for system-wide

pub mod daemon;
pub mod gpu;
pub mod memory;
pub mod modes;
pub mod process;

// Memory optimization exports
pub use memory::{
    LinuxMemoryOptimizer, MemoryError, OptimizationResult, ProcessMemoryInfo, SystemMemoryInfo,
};

// GPU monitoring exports
pub use gpu::{GpuInfo, GpuStats, GpuVendor, LinuxGpuMonitor, VramInfo};

// Daemon/service exports
pub use daemon::{
    daemonize, send_signal, DaemonConfig, DaemonError, DaemonStatus, LinuxDaemonService,
    SignalState, SERVICE_NAME, SYSTEMD_SERVICE_TEMPLATE, SYSTEMD_USER_SERVICE_TEMPLATE,
};

// Process management exports
pub use process::{LinuxProcessManager, ProcessError, ProcessInfo, ProcessState};

// Mode detection exports
pub use modes::{
    FocusAppDetection, FocusCategory, GameCategory, GameDetection, LinuxModeDetector,
    ProcessInfo as ModeProcessInfo,
};

#[cfg(target_os = "linux")]
pub use memory::advice;

/// Check if running on Linux
pub fn is_linux() -> bool {
    cfg!(target_os = "linux")
}

/// Get the Linux kernel version from /proc/version
pub fn kernel_version() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/version").ok().map(|s| {
            s.split_whitespace()
                .nth(2)
                .unwrap_or("unknown")
                .to_string()
        })
    }

    #[cfg(not(target_os = "linux"))]
    None
}

/// Check if cgroups v2 is available
pub fn has_cgroups_v2() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").exists()
    }

    #[cfg(not(target_os = "linux"))]
    false
}

/// Check if memory pressure notifications are available (PSI)
pub fn has_pressure_stall_info() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new("/proc/pressure/memory").exists()
    }

    #[cfg(not(target_os = "linux"))]
    false
}

/// Read pressure stall information for memory
#[cfg(target_os = "linux")]
pub fn read_memory_pressure() -> Option<(f64, f64, f64, f64, f64, f64)> {
    let content = std::fs::read_to_string("/proc/pressure/memory").ok()?;
    let mut some_avg = (0.0, 0.0, 0.0);
    let mut full_avg = (0.0, 0.0, 0.0);

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { continue; }

        let is_some = parts[0] == "some";
        let is_full = parts[0] == "full";

        for part in &parts[1..] {
            if let Some((key, value)) = part.split_once('=') {
                let val: f64 = value.parse().unwrap_or(0.0);
                match (key, is_some, is_full) {
                    ("avg10", true, _) => some_avg.0 = val,
                    ("avg60", true, _) => some_avg.1 = val,
                    ("avg300", true, _) => some_avg.2 = val,
                    ("avg10", _, true) => full_avg.0 = val,
                    ("avg60", _, true) => full_avg.1 = val,
                    ("avg300", _, true) => full_avg.2 = val,
                    _ => {}
                }
            }
        }
    }

    Some((some_avg.0, some_avg.1, some_avg.2, full_avg.0, full_avg.1, full_avg.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_linux() {
        #[cfg(target_os = "linux")]
        assert!(is_linux());
        #[cfg(not(target_os = "linux"))]
        assert!(!is_linux());
    }
}
