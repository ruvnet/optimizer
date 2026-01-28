//! Windows platform implementation
//!
//! This module implements the platform traits for Windows by wrapping
//! the existing Windows-specific modules in `src/windows/`.

#![cfg(target_os = "windows")]

use std::time::{Duration, Instant};
use sysinfo::{System, Pid};

use super::{
    MemoryInfo, OptimizationReport, PlatformError, PlatformMemoryManager,
    PlatformPerformanceUtils, PlatformProcessManager, PlatformResult, PlatformSystemInfo,
    Platform, ProcessInfo, SystemDetails,
};

// Import existing Windows modules
use crate::windows::memory::{MemoryStatus, OptimizationResult, WindowsMemoryOptimizer};
use crate::windows::process::{get_process_memory, get_process_name, list_processes};
use crate::windows::system::{get_system_info as win_get_system_info, SystemInfo};
use crate::windows::performance::{
    cleanup_temp_files as win_cleanup_temp,
    cleanup_thumbnails as win_cleanup_thumbnails,
    flush_dns as win_flush_dns,
    set_high_performance as win_set_high_perf,
};

/// Windows platform implementation
///
/// Wraps the existing Windows-specific modules and provides a unified
/// interface through the platform traits.
pub struct WindowsPlatform {
    /// The underlying Windows memory optimizer
    optimizer: WindowsMemoryOptimizer,
    /// Cached system info
    system: System,
}

impl WindowsPlatform {
    /// Create a new Windows platform instance
    pub fn new() -> Self {
        Self {
            optimizer: WindowsMemoryOptimizer::new(),
            system: System::new_all(),
        }
    }

    /// Get access to the underlying Windows memory optimizer
    pub fn optimizer(&self) -> &WindowsMemoryOptimizer {
        &self.optimizer
    }
}

impl Default for WindowsPlatform {
    fn default() -> Self {
        Self::new()
    }
}

// Conversion from existing MemoryStatus to platform-agnostic MemoryInfo
impl From<MemoryStatus> for MemoryInfo {
    fn from(status: MemoryStatus) -> Self {
        Self {
            total_physical: (status.total_physical_mb * 1024.0 * 1024.0) as u64,
            available_physical: (status.available_physical_mb * 1024.0 * 1024.0) as u64,
            pressure_percent: status.memory_load_percent,
            total_swap: (status.total_page_file_mb * 1024.0 * 1024.0) as u64,
            available_swap: (status.available_page_file_mb * 1024.0 * 1024.0) as u64,
        }
    }
}

// Conversion from existing OptimizationResult to platform-agnostic OptimizationReport
impl From<OptimizationResult> for OptimizationReport {
    fn from(result: OptimizationResult) -> Self {
        Self {
            freed_bytes: (result.freed_mb * 1024.0 * 1024.0) as u64,
            before_available: (result.before_available_mb * 1024.0 * 1024.0) as u64,
            after_available: (result.after_available_mb * 1024.0 * 1024.0) as u64,
            processes_affected: result.processes_trimmed,
            duration: Duration::from_millis(result.duration_ms),
        }
    }
}

// Conversion from existing SystemInfo to platform-agnostic SystemDetails
impl From<SystemInfo> for SystemDetails {
    fn from(info: SystemInfo) -> Self {
        Self {
            os_name: info.os_name,
            os_version: info.kernel_version,
            hostname: info.host_name,
            cpu_count: info.cpu_count,
            cpu_usage: info.cpu_usage,
            total_memory: info.total_memory_mb * 1024 * 1024,
            used_memory: info.used_memory_mb * 1024 * 1024,
        }
    }
}

impl PlatformMemoryManager for WindowsPlatform {
    fn get_memory_info(&self) -> PlatformResult<MemoryInfo> {
        WindowsMemoryOptimizer::get_memory_status()
            .map(MemoryInfo::from)
            .map_err(PlatformError::from)
    }

    fn optimize(&self, aggressive: bool) -> PlatformResult<OptimizationReport> {
        self.optimizer
            .optimize(aggressive)
            .map(OptimizationReport::from)
            .map_err(PlatformError::from)
    }

    fn trim_process(&self, pid: u32) -> PlatformResult<u64> {
        WindowsMemoryOptimizer::trim_process_working_set(pid)
            .map_err(PlatformError::from)
    }

    fn has_elevated_privileges(&self) -> bool {
        self.optimizer.has_admin_privileges()
    }
}

impl PlatformProcessManager for WindowsPlatform {
    fn list_process_ids(&self) -> PlatformResult<Vec<u32>> {
        list_processes().map_err(PlatformError::from)
    }

    fn get_process_info(&self, pid: u32) -> PlatformResult<Option<ProcessInfo>> {
        let name = match get_process_name(pid) {
            Some(n) => n,
            None => return Ok(None),
        };

        let memory_bytes = get_process_memory(pid).unwrap_or(0);

        Ok(Some(ProcessInfo {
            pid,
            name,
            memory_bytes,
        }))
    }

    fn list_processes(&self) -> PlatformResult<Vec<ProcessInfo>> {
        let pids = list_processes().map_err(PlatformError::from)?;
        let mut processes = Vec::with_capacity(pids.len());

        for pid in pids {
            if let Ok(Some(info)) = self.get_process_info(pid) {
                processes.push(info);
            }
        }

        Ok(processes)
    }
}

impl PlatformSystemInfo for WindowsPlatform {
    fn get_system_info(&self) -> PlatformResult<SystemDetails> {
        Ok(SystemDetails::from(win_get_system_info()))
    }

    fn refresh(&mut self) -> PlatformResult<()> {
        self.system.refresh_all();
        Ok(())
    }
}

impl PlatformPerformanceUtils for WindowsPlatform {
    fn cleanup_temp_files(&self) -> PlatformResult<String> {
        win_cleanup_temp().map_err(PlatformError::from)
    }

    fn flush_dns(&self) -> PlatformResult<String> {
        win_flush_dns().map_err(PlatformError::from)
    }

    fn set_high_performance(&self) -> PlatformResult<String> {
        win_set_high_perf().map_err(PlatformError::from)
    }

    fn cleanup_thumbnails(&self) -> PlatformResult<String> {
        win_cleanup_thumbnails().map_err(PlatformError::from)
    }
}

impl Platform for WindowsPlatform {
    fn platform_name(&self) -> &'static str {
        "windows"
    }

    fn is_supported(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_info_conversion() {
        let status = MemoryStatus {
            total_physical_mb: 16384.0,
            available_physical_mb: 8192.0,
            memory_load_percent: 50,
            total_page_file_mb: 24576.0,
            available_page_file_mb: 16384.0,
            total_virtual_mb: 32768.0,
            available_virtual_mb: 24576.0,
        };

        let info = MemoryInfo::from(status);

        assert_eq!(info.total_physical, 16384 * 1024 * 1024);
        assert_eq!(info.available_physical, 8192 * 1024 * 1024);
        assert_eq!(info.pressure_percent, 50);
        assert!(!info.is_high_pressure());
        assert!(!info.is_critical());
    }

    #[test]
    fn test_optimization_report_conversion() {
        let result = OptimizationResult {
            freed_mb: 512.0,
            before_available_mb: 4096.0,
            after_available_mb: 4608.0,
            processes_trimmed: 25,
            duration_ms: 150,
        };

        let report = OptimizationReport::from(result);

        assert_eq!(report.freed_bytes, 512 * 1024 * 1024);
        assert_eq!(report.before_available, 4096 * 1024 * 1024);
        assert_eq!(report.after_available, 4608 * 1024 * 1024);
        assert_eq!(report.processes_affected, 25);
        assert_eq!(report.duration, Duration::from_millis(150));
        assert!(report.was_effective());
    }

    #[test]
    fn test_memory_info_helpers() {
        let info = MemoryInfo {
            total_physical: 16 * 1024 * 1024 * 1024, // 16 GB
            available_physical: 4 * 1024 * 1024 * 1024, // 4 GB
            pressure_percent: 75,
            total_swap: 24 * 1024 * 1024 * 1024,
            available_swap: 20 * 1024 * 1024 * 1024,
        };

        assert_eq!(info.used_physical(), 12 * 1024 * 1024 * 1024);
        assert_eq!(info.total_physical_mb(), 16384.0);
        assert_eq!(info.available_physical_mb(), 4096.0);
        assert!(!info.is_high_pressure());
        assert!(!info.is_critical());

        let high_pressure = MemoryInfo {
            pressure_percent: 85,
            ..info.clone()
        };
        assert!(high_pressure.is_high_pressure());
        assert!(!high_pressure.is_critical());

        let critical = MemoryInfo {
            pressure_percent: 96,
            ..info
        };
        assert!(critical.is_high_pressure());
        assert!(critical.is_critical());
    }

    #[test]
    fn test_platform_error() {
        let err = PlatformError::new(42, "Something went wrong");
        assert_eq!(err.code, 42);
        assert_eq!(err.message, "Something went wrong");
        assert!(err.context.is_none());

        let err_with_ctx = err.with_context("during optimization");
        assert_eq!(err_with_ctx.context, Some("during optimization".to_string()));

        let err_from_str = PlatformError::from("Simple error");
        assert_eq!(err_from_str.code, -1);
        assert_eq!(err_from_str.message, "Simple error");
    }
}
