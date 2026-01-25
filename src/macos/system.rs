//! System-level macOS APIs
//!
//! Provides system information gathering using the `sysinfo` crate.
//! This module mirrors the Windows implementation but targets macOS/Darwin systems.

use sysinfo::System;

/// System information structure containing key metrics.
///
/// All memory values are in megabytes for convenience.
#[derive(Debug, Clone)]
pub struct SystemInfo {
    /// Operating system name (e.g., "macOS")
    pub os_name: String,
    /// Kernel version (Darwin version)
    pub kernel_version: String,
    /// Machine hostname
    pub host_name: String,
    /// Number of logical CPU cores
    pub cpu_count: usize,
    /// Current CPU usage percentage (0.0-100.0)
    pub cpu_usage: f32,
    /// Total physical memory in MB
    pub total_memory_mb: u64,
    /// Currently used memory in MB
    pub used_memory_mb: u64,
}

impl SystemInfo {
    /// Get available memory in MB
    pub fn available_memory_mb(&self) -> u64 {
        self.total_memory_mb.saturating_sub(self.used_memory_mb)
    }

    /// Get memory usage as a percentage
    pub fn memory_usage_percent(&self) -> f32 {
        if self.total_memory_mb == 0 {
            return 0.0;
        }
        (self.used_memory_mb as f32 / self.total_memory_mb as f32) * 100.0
    }
}

/// Gather current system information.
///
/// Collects various system metrics including OS details, CPU info,
/// and memory statistics.
///
/// # Returns
/// A `SystemInfo` struct populated with current system metrics.
///
/// # Example
/// ```no_run
/// use optimizer::macos::system::get_system_info;
///
/// let info = get_system_info();
/// println!("Running on {} with {} CPUs", info.os_name, info.cpu_count);
/// println!("Memory: {} MB / {} MB used", info.used_memory_mb, info.total_memory_mb);
/// ```
pub fn get_system_info() -> SystemInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    SystemInfo {
        os_name: System::name().unwrap_or_else(|| "macOS".to_string()),
        kernel_version: System::kernel_version().unwrap_or_default(),
        host_name: System::host_name().unwrap_or_default(),
        cpu_count: sys.cpus().len(),
        cpu_usage: sys.global_cpu_usage(),
        total_memory_mb: sys.total_memory() / 1024 / 1024,
        used_memory_mb: sys.used_memory() / 1024 / 1024,
    }
}

/// Get total swap space in MB.
pub fn get_total_swap_mb() -> u64 {
    let sys = System::new_all();
    sys.total_swap() / 1024 / 1024
}

/// Get used swap space in MB.
pub fn get_used_swap_mb() -> u64 {
    let sys = System::new_all();
    sys.used_swap() / 1024 / 1024
}

/// Get the system uptime in seconds.
pub fn get_uptime_seconds() -> u64 {
    System::uptime()
}

/// Get the system boot time as Unix timestamp.
pub fn get_boot_time() -> u64 {
    System::boot_time()
}

/// Get OS version string (e.g., "14.0" for macOS Sonoma).
pub fn get_os_version() -> Option<String> {
    System::os_version()
}

/// Get the long OS version string.
pub fn get_long_os_version() -> Option<String> {
    System::long_os_version()
}

/// Check if running on Apple Silicon (ARM64).
#[cfg(target_arch = "aarch64")]
pub fn is_apple_silicon() -> bool {
    true
}

/// Check if running on Apple Silicon (ARM64).
#[cfg(not(target_arch = "aarch64"))]
pub fn is_apple_silicon() -> bool {
    false
}

/// Get CPU architecture string.
pub fn get_cpu_arch() -> &'static str {
    std::env::consts::ARCH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_system_info() {
        let info = get_system_info();

        // CPU count should be at least 1
        assert!(info.cpu_count >= 1);

        // Total memory should be greater than 0
        assert!(info.total_memory_mb > 0);

        // Used memory should not exceed total
        assert!(info.used_memory_mb <= info.total_memory_mb);
    }

    #[test]
    fn test_available_memory() {
        let info = get_system_info();
        let available = info.available_memory_mb();

        // Available should be total minus used
        assert_eq!(available, info.total_memory_mb - info.used_memory_mb);
    }

    #[test]
    fn test_memory_usage_percent() {
        let info = get_system_info();
        let percent = info.memory_usage_percent();

        // Percentage should be between 0 and 100
        assert!(percent >= 0.0);
        assert!(percent <= 100.0);
    }

    #[test]
    fn test_uptime() {
        let uptime = get_uptime_seconds();
        // System should have been up for at least 1 second
        assert!(uptime >= 1);
    }

    #[test]
    fn test_cpu_arch() {
        let arch = get_cpu_arch();
        // Should be a non-empty string
        assert!(!arch.is_empty());
    }
}
