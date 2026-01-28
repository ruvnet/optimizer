//! Android Platform Support
//!
//! This module provides Android-specific functionality including JNI bindings
//! for Kotlin/Java integration.
//!
//! # Features
//!
//! - Memory status monitoring via /proc filesystem and sysinfo
//! - Process listing (read-only)
//! - Neural engine integration for pattern learning
//! - Read-only optimization recommendations (Android restricts memory manipulation)
//!
//! # Architecture
//!
//! Android memory optimization is **read-only** because:
//! - Android does not allow direct memory manipulation from userspace
//! - Apps cannot trim other apps' memory without root
//! - System manages memory via Low Memory Killer (LMK)
//!
//! This module provides:
//! - Memory monitoring and pressure detection
//! - Process enumeration and memory usage tracking
//! - Intelligent recommendations based on neural patterns
//! - JNI bridge for Kotlin/Java Android app integration
//!
//! # Usage from Kotlin
//!
//! ```kotlin
//! class NativeLib {
//!     companion object {
//!         init {
//!             System.loadLibrary("ruvector_memopt")
//!         }
//!     }
//!
//!     external fun getMemoryStatus(): String
//!     external fun getProcessList(): String
//!     external fun runOptimization(): String
//!     external fun getRecommendations(): String
//!     external fun initNeuralEngine(modelPath: String): Boolean
//!     external fun trainPattern(patternJson: String): Boolean
//!     external fun getPatternCount(): Long
//!     external fun getNeuralStatus(): String
//! }
//! ```

pub mod memory;
pub mod process;

#[cfg(target_os = "android")]
mod jni;

#[cfg(target_os = "android")]
pub use jni::*;

use std::time::{Duration, Instant};

use crate::platform::{
    MemoryInfo, OptimizationReport, Platform, PlatformError, PlatformMemoryManager,
    PlatformPerformanceUtils, PlatformProcessManager, PlatformResult, PlatformSystemInfo,
    ProcessInfo as PlatformProcessInfo, SystemDetails,
};

/// Android-specific error types
#[derive(Debug, Clone)]
pub enum AndroidError {
    /// Error reading from /proc filesystem
    ProcReadError(String),
    /// Error parsing memory information
    MemoryParseError(String),
    /// Process not found
    ProcessNotFound(u32),
    /// Permission denied
    PermissionDenied(String),
    /// Generic error
    Other(String),
}

impl std::fmt::Display for AndroidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProcReadError(msg) => write!(f, "Proc read error: {}", msg),
            Self::MemoryParseError(msg) => write!(f, "Memory parse error: {}", msg),
            Self::ProcessNotFound(pid) => write!(f, "Process not found: {}", pid),
            Self::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Self::Other(msg) => write!(f, "Android error: {}", msg),
        }
    }
}

impl std::error::Error for AndroidError {}

impl From<AndroidError> for PlatformError {
    fn from(err: AndroidError) -> Self {
        let code = match &err {
            AndroidError::ProcReadError(_) => 1,
            AndroidError::MemoryParseError(_) => 2,
            AndroidError::ProcessNotFound(_) => 3,
            AndroidError::PermissionDenied(_) => 4,
            AndroidError::Other(_) => -1,
        };
        PlatformError::new(code, err.to_string())
    }
}

/// Result type for Android operations
pub type AndroidResult<T> = Result<T, AndroidError>;

/// Android platform implementation
///
/// Provides read-only memory monitoring and process enumeration.
/// Memory optimization is not directly possible on Android without root,
/// so this implementation focuses on providing accurate monitoring and
/// intelligent recommendations.
pub struct AndroidPlatform {
    sys: sysinfo::System,
}

impl AndroidPlatform {
    /// Create a new Android platform instance
    pub fn new() -> Self {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        sys.refresh_processes();
        Self { sys }
    }
}

impl Default for AndroidPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformMemoryManager for AndroidPlatform {
    fn get_memory_info(&self) -> PlatformResult<MemoryInfo> {
        // Try /proc/meminfo first for detailed info
        match memory::MemoryInfo::read_system_memory() {
            Ok(status) => Ok(MemoryInfo {
                total_physical: (status.total_physical_mb * 1024.0 * 1024.0) as u64,
                available_physical: (status.available_physical_mb * 1024.0 * 1024.0) as u64,
                pressure_percent: status.memory_load_percent,
                total_swap: (status.swap_total_mb * 1024.0 * 1024.0) as u64,
                available_swap: (status.swap_free_mb * 1024.0 * 1024.0) as u64,
            }),
            Err(e) => Err(e.into()),
        }
    }

    fn optimize(&self, _aggressive: bool) -> PlatformResult<OptimizationReport> {
        // Android does not allow direct memory manipulation from userspace
        // Return a report indicating this limitation
        let start = Instant::now();
        let mem_info = self.get_memory_info()?;

        Ok(OptimizationReport {
            freed_bytes: 0,
            before_available: mem_info.available_physical,
            after_available: mem_info.available_physical,
            processes_affected: 0,
            duration: start.elapsed(),
        })
    }

    fn trim_process(&self, _pid: u32) -> PlatformResult<u64> {
        // Cannot trim processes on Android without root
        Err(PlatformError::new(
            5,
            "Direct memory trimming not available on Android. \
             Android manages memory via Low Memory Killer (LMK).",
        ))
    }

    fn has_elevated_privileges(&self) -> bool {
        // Check if we're running as root
        #[cfg(target_os = "android")]
        {
            unsafe { libc::getuid() == 0 }
        }
        #[cfg(not(target_os = "android"))]
        {
            false
        }
    }
}

impl PlatformProcessManager for AndroidPlatform {
    fn list_process_ids(&self) -> PlatformResult<Vec<u32>> {
        process::list_processes().map_err(|e| e.into())
    }

    fn get_process_info(&self, pid: u32) -> PlatformResult<Option<PlatformProcessInfo>> {
        match process::get_process_info(pid) {
            Ok(info) => Ok(Some(PlatformProcessInfo {
                pid: info.pid,
                name: info.name,
                memory_bytes: info.rss_kb * 1024,
            })),
            Err(AndroidError::ProcessNotFound(_)) => Ok(None),
            Err(AndroidError::PermissionDenied(_)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn list_processes(&self) -> PlatformResult<Vec<PlatformProcessInfo>> {
        let pids = process::list_processes().map_err(|e| PlatformError::from(e))?;
        let mut processes = Vec::new();

        for pid in pids {
            if let Ok(info) = process::get_process_info(pid) {
                processes.push(PlatformProcessInfo {
                    pid: info.pid,
                    name: info.name,
                    memory_bytes: info.rss_kb * 1024,
                });
            }
        }

        Ok(processes)
    }
}

impl PlatformSystemInfo for AndroidPlatform {
    fn get_system_info(&self) -> PlatformResult<SystemDetails> {
        use sysinfo::System;

        Ok(SystemDetails {
            os_name: System::name().unwrap_or_else(|| "Android".to_string()),
            os_version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),
            hostname: System::host_name().unwrap_or_else(|| "android-device".to_string()),
            cpu_count: self.sys.cpus().len(),
            cpu_usage: self.sys.global_cpu_usage(),
            total_memory: self.sys.total_memory(),
            used_memory: self.sys.used_memory(),
        })
    }

    fn refresh(&mut self) -> PlatformResult<()> {
        self.sys.refresh_memory();
        self.sys.refresh_processes();
        self.sys.refresh_cpu_all();
        Ok(())
    }
}

impl PlatformPerformanceUtils for AndroidPlatform {
    fn cleanup_temp_files(&self) -> PlatformResult<String> {
        // On Android, app-specific temp files can be cleaned
        // but system-wide cleanup requires special permissions
        Ok("Android temp cleanup requires app-specific implementation. \
            Use Context.getCacheDir() in your Android app."
            .to_string())
    }

    fn flush_dns(&self) -> PlatformResult<String> {
        // DNS flushing on Android requires system permissions
        Ok("DNS flush on Android requires system permissions. \
            Toggle airplane mode as an alternative."
            .to_string())
    }

    fn set_high_performance(&self) -> PlatformResult<String> {
        // Power management on Android is controlled by the system
        Ok("Power management on Android is system-controlled. \
            Use PowerManager API in your Android app."
            .to_string())
    }

    fn cleanup_thumbnails(&self) -> PlatformResult<String> {
        Ok("Thumbnail cleanup on Android is app-specific. \
            Clear individual app caches through Settings."
            .to_string())
    }
}

impl Platform for AndroidPlatform {
    fn platform_name(&self) -> &'static str {
        "android"
    }

    fn is_supported(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_android_platform_creation() {
        let platform = AndroidPlatform::new();
        assert_eq!(platform.platform_name(), "android");
        assert!(platform.is_supported());
    }

    #[test]
    fn test_memory_info() {
        let platform = AndroidPlatform::new();
        // This test will work on Linux too since it uses /proc
        let result = platform.get_memory_info();
        // May fail on non-Linux systems, that's OK
        if result.is_ok() {
            let info = result.unwrap();
            assert!(info.total_physical > 0);
        }
    }

    #[test]
    fn test_optimize_returns_noop() {
        let platform = AndroidPlatform::new();
        let result = platform.optimize(false);
        if result.is_ok() {
            let report = result.unwrap();
            // On Android, we can't actually free memory
            assert_eq!(report.freed_bytes, 0);
            assert_eq!(report.processes_affected, 0);
        }
    }

    #[test]
    fn test_trim_process_not_supported() {
        let platform = AndroidPlatform::new();
        let result = platform.trim_process(1);
        assert!(result.is_err());
    }

    #[test]
    fn test_elevated_privileges() {
        let platform = AndroidPlatform::new();
        // Usually false unless running as root
        let _ = platform.has_elevated_privileges();
    }
}

// Re-export key types
pub use memory::{MemoryStatus, OptimizationResult, ProcessMemoryInfo};
pub use process::ProcessInfo as AndroidProcessInfo;
