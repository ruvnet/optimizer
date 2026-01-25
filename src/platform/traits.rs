//! Platform Abstraction Traits for Cross-Platform Memory Optimization
//!
//! This module defines the core traits that must be implemented by each platform
//! (Windows, Linux) to provide memory optimization, process management, GPU monitoring,
//! system tray integration, and daemon/service functionality.
//!
//! # Architecture
//!
//! The platform abstraction layer uses a trait-based design pattern:
//!
//! ```text
//! +-------------------+
//! |   Platform Traits |  <- This module (defines interfaces)
//! +-------------------+
//!          |
//!    +-----+-----+
//!    |           |
//! +--v--+     +--v--+
//! | Win |     | Lin |  <- Platform-specific implementations
//! +-----+     +-----+
//! ```
//!
//! # Usage
//!
//! Implement the traits for each target platform using conditional compilation:
//!
//! ```ignore
//! #[cfg(target_os = "windows")]
//! impl MemoryOptimizer for WindowsMemoryOptimizer { ... }
//!
//! #[cfg(target_os = "linux")]
//! impl MemoryOptimizer for LinuxMemoryOptimizer { ... }
//! ```

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

// ============================================================================
// Error Types
// ============================================================================

/// Platform-agnostic error type for all platform operations.
#[derive(Debug, Clone)]
pub enum PlatformError {
    /// Permission denied (requires elevated privileges)
    PermissionDenied(String),
    /// Resource not found (process, GPU, etc.)
    NotFound(String),
    /// Operation not supported on this platform
    NotSupported(String),
    /// I/O error occurred
    IoError(String),
    /// Invalid argument provided
    InvalidArgument(String),
    /// System call failed
    SystemError { code: i32, message: String },
    /// Resource is busy or locked
    ResourceBusy(String),
    /// Operation timed out
    Timeout(String),
    /// Internal error
    Internal(String),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            PlatformError::NotFound(msg) => write!(f, "Not found: {}", msg),
            PlatformError::NotSupported(msg) => write!(f, "Not supported: {}", msg),
            PlatformError::IoError(msg) => write!(f, "I/O error: {}", msg),
            PlatformError::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
            PlatformError::SystemError { code, message } => {
                write!(f, "System error ({}): {}", code, message)
            }
            PlatformError::ResourceBusy(msg) => write!(f, "Resource busy: {}", msg),
            PlatformError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            PlatformError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for PlatformError {}

/// Result type alias for platform operations.
pub type PlatformResult<T> = Result<T, PlatformError>;

// ============================================================================
// Memory Types
// ============================================================================

/// System memory status information.
#[derive(Debug, Clone, Default)]
pub struct MemoryInfo {
    /// Total physical memory in bytes
    pub total_physical: u64,
    /// Available physical memory in bytes
    pub available_physical: u64,
    /// Total swap/page file in bytes
    pub total_swap: u64,
    /// Available swap/page file in bytes
    pub available_swap: u64,
    /// Memory pressure percentage (0-100)
    pub pressure_percent: u32,
    /// Whether the system is under memory pressure
    pub is_under_pressure: bool,
    /// Cached memory in bytes (Linux-specific, optional)
    pub cached: Option<u64>,
    /// Buffers in bytes (Linux-specific, optional)
    pub buffers: Option<u64>,
    /// Shared memory in bytes
    pub shared: Option<u64>,
}

impl MemoryInfo {
    /// Calculate used physical memory in bytes.
    pub fn used_physical(&self) -> u64 {
        self.total_physical.saturating_sub(self.available_physical)
    }

    /// Calculate used physical memory in megabytes.
    pub fn used_physical_mb(&self) -> f64 {
        self.used_physical() as f64 / (1024.0 * 1024.0)
    }

    /// Calculate available physical memory in megabytes.
    pub fn available_physical_mb(&self) -> f64 {
        self.available_physical as f64 / (1024.0 * 1024.0)
    }

    /// Calculate total physical memory in megabytes.
    pub fn total_physical_mb(&self) -> f64 {
        self.total_physical as f64 / (1024.0 * 1024.0)
    }

    /// Check if memory is critically low (above 95% usage).
    pub fn is_critical(&self) -> bool {
        self.pressure_percent > 95
    }

    /// Check if memory usage is high (above 80%).
    pub fn is_high_pressure(&self) -> bool {
        self.pressure_percent > 80
    }
}

/// Process-specific memory information.
#[derive(Debug, Clone, Default)]
pub struct ProcessMemoryInfo {
    /// Process ID
    pub pid: u32,
    /// Resident set size (physical memory used) in bytes
    pub rss: u64,
    /// Virtual memory size in bytes
    pub vms: u64,
    /// Shared memory in bytes
    pub shared: u64,
    /// Private memory in bytes (non-shared)
    pub private: u64,
    /// Swap usage in bytes
    pub swap: u64,
    /// Peak working set / high water mark in bytes
    pub peak_rss: Option<u64>,
    /// Page faults count
    pub page_faults: Option<u64>,
}

impl ProcessMemoryInfo {
    /// Get RSS in megabytes.
    pub fn rss_mb(&self) -> f64 {
        self.rss as f64 / (1024.0 * 1024.0)
    }

    /// Get VMS in megabytes.
    pub fn vms_mb(&self) -> f64 {
        self.vms as f64 / (1024.0 * 1024.0)
    }
}

/// Result of a memory optimization operation.
#[derive(Debug, Clone, Default)]
pub struct OptimizationResult {
    /// Memory freed in bytes
    pub freed_bytes: u64,
    /// Memory available before optimization
    pub before_available: u64,
    /// Memory available after optimization
    pub after_available: u64,
    /// Number of processes that were trimmed
    pub processes_trimmed: usize,
    /// Duration of the operation in milliseconds
    pub duration_ms: u64,
    /// Errors encountered (process ID -> error message)
    pub errors: HashMap<u32, String>,
}

impl OptimizationResult {
    /// Get freed memory in megabytes.
    pub fn freed_mb(&self) -> f64 {
        self.freed_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Check if any errors occurred during optimization.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

// ============================================================================
// Process Types
// ============================================================================

/// Information about a running process.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Parent process ID
    pub ppid: u32,
    /// Process name
    pub name: String,
    /// Full command line
    pub cmdline: Vec<String>,
    /// Executable path
    pub exe_path: Option<PathBuf>,
    /// User ID (numeric on Linux, SID on Windows)
    pub user_id: Option<String>,
    /// Username
    pub username: Option<String>,
    /// Process start time (Unix timestamp)
    pub start_time: u64,
    /// CPU usage percentage (0-100 per core)
    pub cpu_percent: f32,
    /// Memory information
    pub memory: ProcessMemoryInfo,
    /// Process state
    pub state: ProcessState,
    /// Thread count
    pub thread_count: u32,
    /// Whether the process is protected from optimization
    pub is_protected: bool,
}

/// Process execution state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    /// Running or runnable
    Running,
    /// Sleeping (interruptible)
    Sleeping,
    /// Disk sleep (uninterruptible)
    DiskSleep,
    /// Stopped (e.g., by signal)
    Stopped,
    /// Zombie (terminated but not reaped)
    Zombie,
    /// Dead
    Dead,
    /// Idle (Linux-specific)
    Idle,
    /// Unknown state
    Unknown,
}

impl Default for ProcessState {
    fn default() -> Self {
        ProcessState::Unknown
    }
}

impl fmt::Display for ProcessState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessState::Running => write!(f, "Running"),
            ProcessState::Sleeping => write!(f, "Sleeping"),
            ProcessState::DiskSleep => write!(f, "Disk Sleep"),
            ProcessState::Stopped => write!(f, "Stopped"),
            ProcessState::Zombie => write!(f, "Zombie"),
            ProcessState::Dead => write!(f, "Dead"),
            ProcessState::Idle => write!(f, "Idle"),
            ProcessState::Unknown => write!(f, "Unknown"),
        }
    }
}

// ============================================================================
// GPU Types
// ============================================================================

/// GPU vendor identification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuVendor {
    /// NVIDIA GPUs
    Nvidia,
    /// AMD/ATI GPUs
    Amd,
    /// Intel integrated/discrete GPUs
    Intel,
    /// Unknown or unsupported vendor
    Unknown,
}

impl fmt::Display for GpuVendor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuVendor::Nvidia => write!(f, "NVIDIA"),
            GpuVendor::Amd => write!(f, "AMD"),
            GpuVendor::Intel => write!(f, "Intel"),
            GpuVendor::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Static GPU device information.
#[derive(Debug, Clone)]
pub struct GpuInfo {
    /// Device index (for multi-GPU systems)
    pub index: u32,
    /// Device name/model
    pub name: String,
    /// GPU vendor
    pub vendor: GpuVendor,
    /// Total VRAM in bytes
    pub total_vram: u64,
    /// Driver version string
    pub driver_version: String,
    /// Compute capability (CUDA version for NVIDIA)
    pub compute_capability: Option<String>,
    /// PCI bus ID (e.g., "0000:01:00.0")
    pub pci_bus_id: Option<String>,
    /// Architecture name (e.g., "Ampere", "RDNA2")
    pub architecture: Option<String>,
}

impl GpuInfo {
    /// Get total VRAM in megabytes.
    pub fn total_vram_mb(&self) -> u64 {
        self.total_vram / (1024 * 1024)
    }

    /// Get total VRAM in gigabytes.
    pub fn total_vram_gb(&self) -> f64 {
        self.total_vram as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

/// Real-time GPU statistics.
#[derive(Debug, Clone, Default)]
pub struct GpuStats {
    /// GPU device index
    pub gpu_index: u32,
    /// Total VRAM in bytes
    pub vram_total: u64,
    /// Used VRAM in bytes
    pub vram_used: u64,
    /// Free VRAM in bytes
    pub vram_free: u64,
    /// GPU core utilization percentage (0-100)
    pub gpu_utilization: Option<u32>,
    /// Memory controller utilization percentage (0-100)
    pub memory_utilization: Option<u32>,
    /// GPU temperature in Celsius
    pub temperature: Option<u32>,
    /// Power usage in milliwatts
    pub power_mw: Option<u32>,
    /// Power limit in milliwatts
    pub power_limit_mw: Option<u32>,
    /// Current GPU clock speed in MHz
    pub clock_mhz: Option<u32>,
    /// Current memory clock speed in MHz
    pub memory_clock_mhz: Option<u32>,
    /// Fan speed percentage (0-100)
    pub fan_speed_percent: Option<u32>,
    /// Encoder utilization percentage
    pub encoder_utilization: Option<u32>,
    /// Decoder utilization percentage
    pub decoder_utilization: Option<u32>,
}

impl GpuStats {
    /// Calculate VRAM usage percentage.
    pub fn vram_usage_percent(&self) -> f64 {
        if self.vram_total == 0 {
            return 0.0;
        }
        (self.vram_used as f64 / self.vram_total as f64) * 100.0
    }

    /// Get used VRAM in megabytes.
    pub fn vram_used_mb(&self) -> u64 {
        self.vram_used / (1024 * 1024)
    }

    /// Get free VRAM in megabytes.
    pub fn vram_free_mb(&self) -> u64 {
        self.vram_free / (1024 * 1024)
    }

    /// Get power usage in watts.
    pub fn power_watts(&self) -> Option<f64> {
        self.power_mw.map(|mw| mw as f64 / 1000.0)
    }

    /// Check if VRAM is under pressure (above threshold percentage).
    pub fn is_vram_pressure(&self, threshold_percent: f64) -> bool {
        self.vram_usage_percent() > threshold_percent
    }
}

// ============================================================================
// System Tray Types
// ============================================================================

/// Tray icon status for visual indication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIconStatus {
    /// Normal operation (green)
    Normal,
    /// Warning state (orange/yellow)
    Warning,
    /// Critical/error state (red)
    Critical,
    /// Busy/processing state
    Busy,
    /// Paused/disabled state
    Paused,
}

/// Notification urgency level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationUrgency {
    /// Low priority, may be hidden
    Low,
    /// Normal priority
    Normal,
    /// High priority, should be shown prominently
    Critical,
}

/// Menu item representation for system tray.
#[derive(Debug, Clone)]
pub enum MenuItem {
    /// Regular clickable item
    Action {
        id: String,
        label: String,
        enabled: bool,
    },
    /// Checkbox item with checked state
    Checkbox {
        id: String,
        label: String,
        checked: bool,
        enabled: bool,
    },
    /// Submenu containing child items
    Submenu {
        label: String,
        items: Vec<MenuItem>,
    },
    /// Visual separator
    Separator,
}

// ============================================================================
// Daemon Service Types
// ============================================================================

/// Status of the daemon/service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonStatus {
    /// Service is running
    Running,
    /// Service is stopped
    Stopped,
    /// Service is starting up
    Starting,
    /// Service is shutting down
    Stopping,
    /// Service failed to start or crashed
    Failed,
    /// Service status is unknown
    Unknown,
}

impl fmt::Display for DaemonStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonStatus::Running => write!(f, "Running"),
            DaemonStatus::Stopped => write!(f, "Stopped"),
            DaemonStatus::Starting => write!(f, "Starting"),
            DaemonStatus::Stopping => write!(f, "Stopping"),
            DaemonStatus::Failed => write!(f, "Failed"),
            DaemonStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Detailed daemon status information.
#[derive(Debug, Clone)]
pub struct DaemonStatusInfo {
    /// Current daemon status
    pub status: DaemonStatus,
    /// Process ID if running
    pub pid: Option<u32>,
    /// Uptime in seconds if running
    pub uptime_secs: Option<u64>,
    /// Last error message if failed
    pub last_error: Option<String>,
    /// Whether the daemon is enabled to start at boot
    pub enabled_at_boot: bool,
    /// Resource usage if available
    pub memory_usage: Option<u64>,
    pub cpu_percent: Option<f32>,
}

// ============================================================================
// Platform Traits
// ============================================================================

/// Memory optimization operations.
///
/// This trait provides methods for querying system memory status,
/// trimming process working sets, and performing system-wide memory optimization.
///
/// # Platform Behavior
///
/// - **Windows**: Uses Win32 APIs (SetProcessWorkingSetSizeEx, EmptyWorkingSet)
/// - **Linux**: Uses cgroups v2, /proc filesystem, and madvise hints
///
/// # Example
///
/// ```ignore
/// let optimizer = PlatformMemoryOptimizer::new()?;
/// let info = optimizer.get_system_memory_info()?;
/// println!("Memory pressure: {}%", info.pressure_percent);
///
/// if info.is_high_pressure() {
///     let result = optimizer.optimize_system(false)?;
///     println!("Freed {} MB", result.freed_mb());
/// }
/// ```
pub trait MemoryOptimizer: Send + Sync {
    /// Create a new memory optimizer instance.
    fn new() -> PlatformResult<Self>
    where
        Self: Sized;

    /// Check if the optimizer has elevated privileges.
    fn has_elevated_privileges(&self) -> bool;

    /// Get system-wide memory information.
    fn get_system_memory_info(&self) -> PlatformResult<MemoryInfo>;

    /// Get memory information for a specific process.
    ///
    /// # Arguments
    ///
    /// * `pid` - Process ID to query
    fn get_process_memory_info(&self, pid: u32) -> PlatformResult<ProcessMemoryInfo>;

    /// Trim the working set of a specific process.
    ///
    /// On Windows, this calls SetProcessWorkingSetSizeEx to release memory.
    /// On Linux, this uses MADV_DONTNEED or cgroup memory.reclaim.
    ///
    /// # Arguments
    ///
    /// * `pid` - Process ID to trim
    ///
    /// # Returns
    ///
    /// The number of bytes freed (may be approximate).
    fn trim_working_set(&self, pid: u32) -> PlatformResult<u64>;

    /// Perform system-wide memory optimization.
    ///
    /// This iterates through eligible processes and attempts to reclaim memory.
    ///
    /// # Arguments
    ///
    /// * `aggressive` - If true, uses more aggressive optimization techniques
    fn optimize_system(&self, aggressive: bool) -> PlatformResult<OptimizationResult>;

    /// Drop filesystem caches (requires elevated privileges).
    ///
    /// On Linux, writes to /proc/sys/vm/drop_caches.
    /// On Windows, this is a no-op as cache management differs.
    fn drop_caches(&self) -> PlatformResult<()>;

    /// Compact memory (if supported by the platform).
    ///
    /// On Linux, triggers /proc/sys/vm/compact_memory.
    /// On Windows, uses memory compaction APIs if available.
    fn compact_memory(&self) -> PlatformResult<()>;
}

/// Process enumeration and management.
///
/// This trait provides methods for listing processes, querying process information,
/// and determining if processes should be protected from optimization.
///
/// # Example
///
/// ```ignore
/// let manager = PlatformProcessManager::new()?;
/// for process in manager.list_processes()? {
///     if !manager.is_protected(&process.name) {
///         println!("Can optimize: {} (PID {})", process.name, process.pid);
///     }
/// }
/// ```
pub trait ProcessManager: Send + Sync {
    /// Create a new process manager instance.
    fn new() -> PlatformResult<Self>
    where
        Self: Sized;

    /// List all running processes.
    fn list_processes(&self) -> PlatformResult<Vec<ProcessInfo>>;

    /// Get information about a specific process.
    ///
    /// # Arguments
    ///
    /// * `pid` - Process ID to query
    fn get_process(&self, pid: u32) -> PlatformResult<ProcessInfo>;

    /// Get list of process IDs only (faster than full enumeration).
    fn list_pids(&self) -> PlatformResult<Vec<u32>>;

    /// Check if a process name is protected from optimization.
    ///
    /// Protected processes include:
    /// - System processes (init, systemd, kernel threads)
    /// - Critical services (sshd, dbus, etc.)
    /// - User-defined protected list
    ///
    /// # Arguments
    ///
    /// * `name` - Process name to check
    fn is_protected(&self, name: &str) -> bool;

    /// Add a process name to the protected list.
    ///
    /// # Arguments
    ///
    /// * `name` - Process name to protect
    fn add_protected(&mut self, name: &str);

    /// Remove a process name from the protected list.
    ///
    /// # Arguments
    ///
    /// * `name` - Process name to unprotect
    fn remove_protected(&mut self, name: &str);

    /// Get the current protected process list.
    fn get_protected_list(&self) -> Vec<String>;

    /// Check if a process is a system/kernel process.
    ///
    /// # Arguments
    ///
    /// * `pid` - Process ID to check
    fn is_system_process(&self, pid: u32) -> bool;

    /// Get processes sorted by memory usage (highest first).
    fn get_top_memory_consumers(&self, limit: usize) -> PlatformResult<Vec<ProcessInfo>>;
}

/// GPU detection and monitoring.
///
/// This trait provides methods for detecting GPUs, querying VRAM usage,
/// and monitoring GPU performance metrics.
///
/// # Platform Support
///
/// - **NVIDIA**: Full support via NVML on both Windows and Linux
/// - **AMD**: ROCm-SMI on Linux, limited on Windows
/// - **Intel**: Basic support via sysfs on Linux, DirectX on Windows
///
/// # Example
///
/// ```ignore
/// let monitor = PlatformGpuMonitor::new()?;
/// for gpu in monitor.detect_gpus()? {
///     let stats = monitor.get_gpu_stats(gpu.index)?;
///     println!("{}: VRAM {:.1}% used", gpu.name, stats.vram_usage_percent());
/// }
/// ```
pub trait GpuMonitor: Send + Sync {
    /// Create a new GPU monitor instance.
    fn new() -> PlatformResult<Self>
    where
        Self: Sized;

    /// Detect all available GPUs in the system.
    fn detect_gpus(&self) -> PlatformResult<Vec<GpuInfo>>;

    /// Get real-time statistics for a specific GPU.
    ///
    /// # Arguments
    ///
    /// * `index` - GPU device index
    fn get_gpu_stats(&self, index: u32) -> PlatformResult<GpuStats>;

    /// Get statistics for all detected GPUs.
    fn get_all_gpu_stats(&self) -> PlatformResult<Vec<GpuStats>>;

    /// Get total VRAM across all GPUs.
    fn get_total_vram(&self) -> PlatformResult<u64>;

    /// Get total used VRAM across all GPUs.
    fn get_total_vram_used(&self) -> PlatformResult<u64>;

    /// Check if any GPU is under VRAM pressure.
    ///
    /// # Arguments
    ///
    /// * `threshold_percent` - VRAM usage threshold (0-100)
    fn is_vram_pressure(&self, threshold_percent: f64) -> PlatformResult<bool>;

    /// Get processes using GPU memory (if supported).
    fn get_gpu_processes(&self, index: u32) -> PlatformResult<Vec<(u32, u64)>>; // (pid, vram_bytes)
}

/// System tray integration.
///
/// This trait provides methods for displaying a system tray icon,
/// showing notifications, and managing a tray menu.
///
/// # Platform Behavior
///
/// - **Windows**: Uses Shell_NotifyIcon and WM_CONTEXTMENU
/// - **Linux**: Uses libappindicator, StatusNotifierItem, or XEmbed fallback
///
/// # Example
///
/// ```ignore
/// let tray = PlatformSystemTray::new("RuVector", TrayIconStatus::Normal)?;
/// tray.show_notification("Optimization Complete", "Freed 512 MB", NotificationUrgency::Normal)?;
/// ```
pub trait SystemTray: Send + Sync {
    /// Create a new system tray instance.
    ///
    /// # Arguments
    ///
    /// * `tooltip` - Default tooltip text
    /// * `status` - Initial icon status
    fn new(tooltip: &str, status: TrayIconStatus) -> PlatformResult<Self>
    where
        Self: Sized;

    /// Show a notification/toast message.
    ///
    /// # Arguments
    ///
    /// * `title` - Notification title
    /// * `message` - Notification body
    /// * `urgency` - Urgency level affecting display behavior
    fn show_notification(
        &self,
        title: &str,
        message: &str,
        urgency: NotificationUrgency,
    ) -> PlatformResult<()>;

    /// Update the tray icon based on status.
    ///
    /// # Arguments
    ///
    /// * `status` - New icon status
    fn set_icon(&self, status: TrayIconStatus) -> PlatformResult<()>;

    /// Update the tooltip text.
    ///
    /// # Arguments
    ///
    /// * `tooltip` - New tooltip text
    fn set_tooltip(&self, tooltip: &str) -> PlatformResult<()>;

    /// Create or update the tray context menu.
    ///
    /// # Arguments
    ///
    /// * `items` - Menu items to display
    fn set_menu(&self, items: Vec<MenuItem>) -> PlatformResult<()>;

    /// Update a specific menu item's state.
    ///
    /// # Arguments
    ///
    /// * `id` - Menu item identifier
    /// * `checked` - New checked state (for checkbox items)
    /// * `enabled` - New enabled state
    fn update_menu_item(&self, id: &str, checked: Option<bool>, enabled: Option<bool>)
        -> PlatformResult<()>;

    /// Run the event loop (blocking).
    ///
    /// This processes menu events and system messages. Use with a callback
    /// to handle menu item selections.
    fn run<F>(&self, callback: F) -> PlatformResult<()>
    where
        F: FnMut(&str) + Send + 'static;

    /// Request the event loop to exit.
    fn quit(&self) -> PlatformResult<()>;
}

/// Daemon/service management.
///
/// This trait provides methods for running as a background service,
/// installing/uninstalling the service, and querying service status.
///
/// # Platform Behavior
///
/// - **Windows**: Windows Service Control Manager (SCM)
/// - **Linux**: systemd (preferred), SysV init, or OpenRC
///
/// # Example
///
/// ```ignore
/// let daemon = PlatformDaemon::new("ruvector-memopt")?;
/// if !daemon.is_installed()? {
///     daemon.install()?;
/// }
/// daemon.start()?;
/// ```
pub trait DaemonService: Send + Sync {
    /// Create a new daemon service instance.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name (used for registration)
    fn new(name: &str) -> PlatformResult<Self>
    where
        Self: Sized;

    /// Start the daemon/service.
    fn start(&self) -> PlatformResult<()>;

    /// Stop the daemon/service.
    fn stop(&self) -> PlatformResult<()>;

    /// Restart the daemon/service.
    fn restart(&self) -> PlatformResult<()>;

    /// Get the current daemon status.
    fn status(&self) -> PlatformResult<DaemonStatusInfo>;

    /// Check if the daemon is currently running.
    fn is_running(&self) -> PlatformResult<bool>;

    /// Install the daemon as a system service.
    ///
    /// # Arguments
    ///
    /// * `executable_path` - Path to the service executable
    /// * `args` - Command line arguments to pass
    fn install(&self, executable_path: &PathBuf, args: &[&str]) -> PlatformResult<()>;

    /// Uninstall the daemon from the system.
    fn uninstall(&self) -> PlatformResult<()>;

    /// Check if the daemon is installed as a service.
    fn is_installed(&self) -> PlatformResult<bool>;

    /// Enable the daemon to start at system boot.
    fn enable(&self) -> PlatformResult<()>;

    /// Disable the daemon from starting at system boot.
    fn disable(&self) -> PlatformResult<()>;

    /// Run the main service loop.
    ///
    /// This is called when the process is started as a service.
    /// The callback is invoked periodically until stop is requested.
    ///
    /// # Arguments
    ///
    /// * `callback` - Function called periodically during service operation
    fn run_service<F>(&self, callback: F) -> PlatformResult<()>
    where
        F: FnMut() -> bool + Send + 'static; // Return false to stop

    /// Reload service configuration without restart.
    fn reload(&self) -> PlatformResult<()>;
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Get the default list of protected process names for the current platform.
#[cfg(target_os = "windows")]
pub fn default_protected_processes() -> Vec<&'static str> {
    vec![
        // Windows core
        "System",
        "Registry",
        "smss.exe",
        "csrss.exe",
        "wininit.exe",
        "services.exe",
        "lsass.exe",
        "svchost.exe",
        "dwm.exe",
        "explorer.exe",
        "winlogon.exe",
        // Security
        "MsMpEng.exe",
        "SecurityHealthService.exe",
        // Critical services
        "spoolsv.exe",
        "SearchIndexer.exe",
        // Our own processes
        "ruvector-memopt.exe",
        "ruvector-memopt-service.exe",
        "ruvector-memopt-tray.exe",
    ]
}

/// Get the default list of protected process names for the current platform.
#[cfg(target_os = "linux")]
pub fn default_protected_processes() -> Vec<&'static str> {
    vec![
        // Init systems
        "systemd",
        "init",
        // Kernel threads (usually in brackets)
        "kthreadd",
        "ksoftirqd",
        "kworker",
        "rcu_sched",
        "migration",
        // Critical daemons
        "sshd",
        "dbus-daemon",
        "polkitd",
        "NetworkManager",
        "systemd-journald",
        "systemd-logind",
        "systemd-udevd",
        "rsyslogd",
        "cron",
        "atd",
        // Display
        "Xorg",
        "Xwayland",
        "gdm",
        "sddm",
        "lightdm",
        "gnome-shell",
        "kwin",
        "pulseaudio",
        "pipewire",
        // Our own processes
        "ruvector-memopt",
        "ruvector-memopt-service",
        "ruvector-memopt-tray",
    ]
}

/// Get the default list of protected process names for the current platform.
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn default_protected_processes() -> Vec<&'static str> {
    vec!["ruvector-memopt", "ruvector-memopt-service"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_info_calculations() {
        let info = MemoryInfo {
            total_physical: 16 * 1024 * 1024 * 1024, // 16 GB
            available_physical: 8 * 1024 * 1024 * 1024, // 8 GB
            total_swap: 8 * 1024 * 1024 * 1024,
            available_swap: 8 * 1024 * 1024 * 1024,
            pressure_percent: 50,
            is_under_pressure: false,
            cached: None,
            buffers: None,
            shared: None,
        };

        assert_eq!(info.used_physical(), 8 * 1024 * 1024 * 1024);
        assert!((info.total_physical_mb() - 16384.0).abs() < 0.1);
        assert!(!info.is_high_pressure());
        assert!(!info.is_critical());
    }

    #[test]
    fn test_gpu_stats_calculations() {
        let stats = GpuStats {
            gpu_index: 0,
            vram_total: 8 * 1024 * 1024 * 1024, // 8 GB
            vram_used: 4 * 1024 * 1024 * 1024,  // 4 GB
            vram_free: 4 * 1024 * 1024 * 1024,
            power_mw: Some(150_000), // 150W
            ..Default::default()
        };

        assert!((stats.vram_usage_percent() - 50.0).abs() < 0.1);
        assert_eq!(stats.vram_used_mb(), 4096);
        assert!((stats.power_watts().unwrap() - 150.0).abs() < 0.1);
        assert!(!stats.is_vram_pressure(75.0));
        assert!(stats.is_vram_pressure(40.0));
    }

    #[test]
    fn test_platform_error_display() {
        let err = PlatformError::PermissionDenied("Need root".to_string());
        assert!(err.to_string().contains("Permission denied"));

        let err = PlatformError::SystemError {
            code: 5,
            message: "Access denied".to_string(),
        };
        assert!(err.to_string().contains("5"));
    }

    #[test]
    fn test_default_protected_processes() {
        let protected = default_protected_processes();
        assert!(!protected.is_empty());
        // Our own processes should always be protected
        assert!(protected.iter().any(|p| p.contains("ruvector")));
    }
}
