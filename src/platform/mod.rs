//! Platform abstraction layer for cross-platform memory optimization
//!
//! This module provides platform-agnostic traits that abstract away
//! OS-specific memory management APIs. Each platform implements these
//! traits to provide consistent behavior across Windows, Linux, and macOS.

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "android")]
pub mod android;

use std::time::Duration;

/// Platform-agnostic memory status
#[derive(Debug, Clone)]
pub struct MemoryInfo {
    /// Total physical memory in bytes
    pub total_physical: u64,
    /// Available physical memory in bytes
    pub available_physical: u64,
    /// Memory pressure as percentage (0-100)
    pub pressure_percent: u32,
    /// Total swap/page file in bytes
    pub total_swap: u64,
    /// Available swap/page file in bytes
    pub available_swap: u64,
}

impl MemoryInfo {
    /// Used physical memory in bytes
    pub fn used_physical(&self) -> u64 {
        self.total_physical.saturating_sub(self.available_physical)
    }

    /// Total physical memory in megabytes
    pub fn total_physical_mb(&self) -> f64 {
        self.total_physical as f64 / 1024.0 / 1024.0
    }

    /// Available physical memory in megabytes
    pub fn available_physical_mb(&self) -> f64 {
        self.available_physical as f64 / 1024.0 / 1024.0
    }

    /// Check if memory is under high pressure (>80%)
    pub fn is_high_pressure(&self) -> bool {
        self.pressure_percent > 80
    }

    /// Check if memory is critical (>95%)
    pub fn is_critical(&self) -> bool {
        self.pressure_percent > 95
    }
}

/// Platform-agnostic optimization result
#[derive(Debug, Clone)]
pub struct OptimizationReport {
    /// Memory freed in bytes
    pub freed_bytes: u64,
    /// Available memory before optimization (bytes)
    pub before_available: u64,
    /// Available memory after optimization (bytes)
    pub after_available: u64,
    /// Number of processes affected
    pub processes_affected: usize,
    /// Duration of the optimization operation
    pub duration: Duration,
}

impl OptimizationReport {
    /// Memory freed in megabytes
    pub fn freed_mb(&self) -> f64 {
        self.freed_bytes as f64 / 1024.0 / 1024.0
    }

    /// Check if optimization was effective (freed > 0)
    pub fn was_effective(&self) -> bool {
        self.freed_bytes > 0
    }
}

/// Platform-agnostic process information
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
    /// Memory usage in bytes
    pub memory_bytes: u64,
}

/// Platform-agnostic system information
#[derive(Debug, Clone)]
pub struct SystemDetails {
    /// Operating system name
    pub os_name: String,
    /// Kernel/OS version
    pub os_version: String,
    /// Hostname
    pub hostname: String,
    /// Number of CPU cores
    pub cpu_count: usize,
    /// Current CPU usage percentage
    pub cpu_usage: f32,
    /// Total memory in bytes
    pub total_memory: u64,
    /// Used memory in bytes
    pub used_memory: u64,
}

/// Platform-specific error type
#[derive(Debug, Clone)]
pub struct PlatformError {
    /// Error code (platform-specific)
    pub code: i32,
    /// Human-readable error message
    pub message: String,
    /// Additional context
    pub context: Option<String>,
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ctx) = &self.context {
            write!(f, "{} (code: {}, context: {})", self.message, self.code, ctx)
        } else {
            write!(f, "{} (code: {})", self.message, self.code)
        }
    }
}

impl std::error::Error for PlatformError {}

impl PlatformError {
    /// Create a new platform error
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            context: None,
        }
    }

    /// Add context to the error
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Create from a string error
    pub fn from_string(message: impl Into<String>) -> Self {
        Self {
            code: -1,
            message: message.into(),
            context: None,
        }
    }
}

impl From<String> for PlatformError {
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}

impl From<&str> for PlatformError {
    fn from(s: &str) -> Self {
        Self::from_string(s)
    }
}

/// Result type for platform operations
pub type PlatformResult<T> = Result<T, PlatformError>;

/// Core trait for platform-specific memory management
pub trait PlatformMemoryManager: Send + Sync {
    /// Get current memory status
    fn get_memory_info(&self) -> PlatformResult<MemoryInfo>;

    /// Perform memory optimization
    ///
    /// # Arguments
    /// * `aggressive` - If true, perform more aggressive optimization
    fn optimize(&self, aggressive: bool) -> PlatformResult<OptimizationReport>;

    /// Trim working set of a specific process
    ///
    /// # Arguments
    /// * `pid` - Process ID to trim
    ///
    /// # Returns
    /// Number of bytes freed
    fn trim_process(&self, pid: u32) -> PlatformResult<u64>;

    /// Check if running with elevated privileges
    fn has_elevated_privileges(&self) -> bool;
}

/// Trait for process enumeration and management
pub trait PlatformProcessManager: Send + Sync {
    /// List all process IDs
    fn list_process_ids(&self) -> PlatformResult<Vec<u32>>;

    /// Get information about a specific process
    fn get_process_info(&self, pid: u32) -> PlatformResult<Option<ProcessInfo>>;

    /// List all processes with their information
    fn list_processes(&self) -> PlatformResult<Vec<ProcessInfo>>;
}

/// Trait for system information retrieval
pub trait PlatformSystemInfo: Send + Sync {
    /// Get system details
    fn get_system_info(&self) -> PlatformResult<SystemDetails>;

    /// Refresh system information cache
    fn refresh(&mut self) -> PlatformResult<()>;
}

/// Trait for performance utilities
pub trait PlatformPerformanceUtils: Send + Sync {
    /// Clean up temporary files
    ///
    /// # Returns
    /// Description of what was cleaned
    fn cleanup_temp_files(&self) -> PlatformResult<String>;

    /// Flush DNS cache
    fn flush_dns(&self) -> PlatformResult<String>;

    /// Set high performance power mode (if supported)
    fn set_high_performance(&self) -> PlatformResult<String>;

    /// Clean up thumbnail cache
    fn cleanup_thumbnails(&self) -> PlatformResult<String>;
}

/// Unified platform interface combining all traits
pub trait Platform:
    PlatformMemoryManager + PlatformProcessManager + PlatformSystemInfo + PlatformPerformanceUtils
{
    /// Get platform name
    fn platform_name(&self) -> &'static str;

    /// Check if this platform is supported
    fn is_supported(&self) -> bool {
        true
    }
}

// Re-export platform implementations
#[cfg(target_os = "windows")]
pub use windows::WindowsPlatform;

#[cfg(target_os = "android")]
pub use android::AndroidPlatform;

/// Create the default platform implementation for the current OS
#[cfg(target_os = "windows")]
pub fn create_platform() -> impl Platform {
    windows::WindowsPlatform::new()
}

/// Create the default platform implementation for Android
#[cfg(target_os = "android")]
pub fn create_platform() -> impl Platform {
    android::AndroidPlatform::new()
}

/// Create the default platform implementation (stub for unsupported platforms)
#[cfg(all(not(target_os = "windows"), not(target_os = "android")))]
pub fn create_platform() -> impl Platform {
    StubPlatform
}

/// Stub platform for unsupported operating systems
#[cfg(all(not(target_os = "windows"), not(target_os = "android")))]
pub struct StubPlatform;

#[cfg(all(not(target_os = "windows"), not(target_os = "android")))]
impl PlatformMemoryManager for StubPlatform {
    fn get_memory_info(&self) -> PlatformResult<MemoryInfo> {
        Err(PlatformError::new(1, "Platform not supported"))
    }

    fn optimize(&self, _aggressive: bool) -> PlatformResult<OptimizationReport> {
        Err(PlatformError::new(1, "Platform not supported"))
    }

    fn trim_process(&self, _pid: u32) -> PlatformResult<u64> {
        Err(PlatformError::new(1, "Platform not supported"))
    }

    fn has_elevated_privileges(&self) -> bool {
        false
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "android")))]
impl PlatformProcessManager for StubPlatform {
    fn list_process_ids(&self) -> PlatformResult<Vec<u32>> {
        Err(PlatformError::new(1, "Platform not supported"))
    }

    fn get_process_info(&self, _pid: u32) -> PlatformResult<Option<ProcessInfo>> {
        Err(PlatformError::new(1, "Platform not supported"))
    }

    fn list_processes(&self) -> PlatformResult<Vec<ProcessInfo>> {
        Err(PlatformError::new(1, "Platform not supported"))
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "android")))]
impl PlatformSystemInfo for StubPlatform {
    fn get_system_info(&self) -> PlatformResult<SystemDetails> {
        Err(PlatformError::new(1, "Platform not supported"))
    }

    fn refresh(&mut self) -> PlatformResult<()> {
        Err(PlatformError::new(1, "Platform not supported"))
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "android")))]
impl PlatformPerformanceUtils for StubPlatform {
    fn cleanup_temp_files(&self) -> PlatformResult<String> {
        Err(PlatformError::new(1, "Platform not supported"))
    }

    fn flush_dns(&self) -> PlatformResult<String> {
        Err(PlatformError::new(1, "Platform not supported"))
    }

    fn set_high_performance(&self) -> PlatformResult<String> {
        Err(PlatformError::new(1, "Platform not supported"))
    }

    fn cleanup_thumbnails(&self) -> PlatformResult<String> {
        Err(PlatformError::new(1, "Platform not supported"))
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "android")))]
impl Platform for StubPlatform {
    fn platform_name(&self) -> &'static str {
        "unsupported"
    }

    fn is_supported(&self) -> bool {
        false
    }
}
