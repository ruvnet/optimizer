//! Platform Abstraction Layer for RuVector Memory Optimizer
//!
//! This module provides cross-platform abstractions for memory optimization,
//! process management, GPU monitoring, system tray integration, and daemon services.
//!
//! # Architecture
//!
//! The platform layer uses conditional compilation to select the appropriate
//! implementation at build time:
//!
//! ```text
//! src/platform/
//! +-- mod.rs           <- This file (module definitions)
//! +-- traits.rs        <- Platform-agnostic trait definitions
//! +-- linux/           <- Linux-specific implementations (cfg(linux))
//! |   +-- mod.rs
//! |   +-- memory.rs
//! |   +-- process.rs
//! |   +-- gpu.rs
//! |   +-- daemon.rs
//! |   +-- modes.rs
//! +-- (windows/)       <- Windows implementations use existing src/windows/
//! ```
//!
//! # Usage
//!
//! Import the platform types and they will automatically resolve to the
//! correct implementation for the target platform:
//!
//! ```ignore
//! use ruvector_memopt::platform::{
//!     PlatformMemoryOptimizer,
//!     PlatformProcessManager,
//!     PlatformGpuMonitor,
//! };
//!
//! // These types are aliases to the platform-specific implementations
//! let optimizer = PlatformMemoryOptimizer::new()?;
//! let result = optimizer.optimize_system(false)?;
//! println!("Freed {} MB", result.freed_mb());
//! ```
//!
//! # Feature Flags
//!
//! - `ai`: Enables GPU monitoring and AI workload detection
//! - `nvml`: Enables NVIDIA Management Library for detailed GPU stats
//!
//! # Platform Support
//!
//! | Feature | Windows | Linux |
//! |---------|---------|-------|
//! | Memory Optimization | Full | Full |
//! | Process Management | Full | Full |
//! | GPU (NVIDIA) | NVML/DXGI | NVML |
//! | GPU (AMD) | Limited | ROCm-SMI |
//! | GPU (Intel) | DXGI | sysfs |
//! | System Tray | Shell_NotifyIcon | AppIndicator/SNI |
//! | Daemon/Service | SCM | systemd |

// Core trait definitions - always available
pub mod traits;

// Re-export all trait types for convenience
pub use traits::{
    // Error types
    PlatformError,
    PlatformResult,
    // Memory types
    MemoryInfo,
    ProcessMemoryInfo,
    OptimizationResult,
    // Process types
    ProcessInfo,
    ProcessState,
    // GPU types
    GpuVendor,
    GpuInfo,
    GpuStats,
    // Tray types
    TrayIconStatus,
    NotificationUrgency,
    MenuItem,
    // Daemon types
    DaemonStatus,
    DaemonStatusInfo,
    // Traits
    MemoryOptimizer,
    ProcessManager,
    GpuMonitor,
    SystemTray,
    DaemonService,
    // Utility functions
    default_protected_processes,
};

// ============================================================================
// Linux Platform Implementation
// ============================================================================

/// Linux-specific implementations.
#[cfg(target_os = "linux")]
pub mod linux;

/// Platform-specific memory optimizer type alias for Linux.
#[cfg(target_os = "linux")]
pub type PlatformMemoryOptimizer = linux::LinuxMemoryOptimizer;

/// Platform-specific process manager type alias for Linux.
#[cfg(target_os = "linux")]
pub type PlatformProcessManager = linux::LinuxProcessManager;

/// Platform-specific GPU monitor type alias for Linux.
#[cfg(target_os = "linux")]
pub type PlatformGpuMonitor = linux::LinuxGpuMonitor;

/// Platform-specific daemon service type alias for Linux.
#[cfg(target_os = "linux")]
pub type PlatformDaemonService = linux::LinuxDaemonService;

// Re-export Linux-specific types when on Linux
#[cfg(target_os = "linux")]
pub use linux::{
    LinuxMemoryOptimizer,
    LinuxProcessManager,
    LinuxGpuMonitor,
    LinuxDaemonService,
    // Utility functions
    kernel_version,
    has_cgroups_v2,
    has_pressure_stall_info,
    is_linux,
};

// ============================================================================
// Windows Platform Implementation
// ============================================================================

// Note: Windows implementations currently live in src/windows/
// They will be migrated to src/platform/windows/ in a future update.
// For now, we re-export from the existing module.

#[cfg(target_os = "windows")]
pub use crate::windows::WindowsMemoryOptimizer as PlatformMemoryOptimizer;

// ============================================================================
// Platform Detection Utilities
// ============================================================================

/// Information about the current platform.
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    /// Operating system name
    pub os_name: &'static str,
    /// Operating system version (if available)
    pub os_version: Option<String>,
    /// CPU architecture
    pub arch: &'static str,
    /// Whether running with elevated privileges
    pub is_elevated: bool,
    /// Init system type (Linux only)
    pub init_system: Option<InitSystem>,
    /// Desktop environment (Linux only)
    pub desktop_environment: Option<String>,
}

/// Linux init system types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitSystem {
    /// systemd
    Systemd,
    /// SysV init
    SysVInit,
    /// OpenRC
    OpenRC,
    /// Upstart
    Upstart,
    /// runit
    Runit,
    /// Unknown init system
    Unknown,
}

impl std::fmt::Display for InitSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitSystem::Systemd => write!(f, "systemd"),
            InitSystem::SysVInit => write!(f, "sysvinit"),
            InitSystem::OpenRC => write!(f, "openrc"),
            InitSystem::Upstart => write!(f, "upstart"),
            InitSystem::Runit => write!(f, "runit"),
            InitSystem::Unknown => write!(f, "unknown"),
        }
    }
}

/// Detect information about the current platform.
pub fn detect_platform() -> PlatformInfo {
    PlatformInfo {
        os_name: std::env::consts::OS,
        os_version: detect_os_version(),
        arch: std::env::consts::ARCH,
        is_elevated: detect_elevated(),
        init_system: detect_init_system(),
        desktop_environment: detect_desktop_environment(),
    }
}

/// Detect the OS version string.
fn detect_os_version() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("cmd")
            .args(["/C", "ver"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    }

    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|line| line.starts_with("PRETTY_NAME="))
                    .map(|line| {
                        line.trim_start_matches("PRETTY_NAME=")
                            .trim_matches('"')
                            .to_string()
                    })
            })
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

/// Detect if the current process has elevated privileges.
fn detect_elevated() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("net")
            .args(["session"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(target_os = "linux")]
    {
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

/// Detect the Linux init system.
fn detect_init_system() -> Option<InitSystem> {
    #[cfg(target_os = "linux")]
    {
        use std::path::Path;

        // Check for systemd (most common)
        if Path::new("/run/systemd/system").exists() {
            return Some(InitSystem::Systemd);
        }

        // Check for OpenRC
        if Path::new("/sbin/openrc").exists() || Path::new("/usr/sbin/openrc").exists() {
            return Some(InitSystem::OpenRC);
        }

        // Check for runit
        if Path::new("/run/runit").exists() {
            return Some(InitSystem::Runit);
        }

        // Check for Upstart
        if Path::new("/sbin/initctl").exists() {
            if std::process::Command::new("initctl")
                .arg("--version")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).contains("upstart"))
                .unwrap_or(false)
            {
                return Some(InitSystem::Upstart);
            }
        }

        // Check for SysV init (fallback)
        if Path::new("/etc/init.d").exists() {
            return Some(InitSystem::SysVInit);
        }

        Some(InitSystem::Unknown)
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Detect the desktop environment (Linux).
fn detect_desktop_environment() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        // Check XDG_CURRENT_DESKTOP first
        if let Ok(de) = std::env::var("XDG_CURRENT_DESKTOP") {
            if !de.is_empty() {
                return Some(de);
            }
        }

        // Check DESKTOP_SESSION
        if let Ok(session) = std::env::var("DESKTOP_SESSION") {
            if !session.is_empty() {
                return Some(session);
            }
        }

        // Check for specific environment variables
        if std::env::var("GNOME_DESKTOP_SESSION_ID").is_ok() {
            return Some("GNOME".to_string());
        }
        if std::env::var("KDE_FULL_SESSION").is_ok() {
            return Some("KDE".to_string());
        }

        None
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Check if a specific feature is supported on the current platform.
pub fn is_feature_supported(feature: PlatformFeature) -> bool {
    match feature {
        PlatformFeature::MemoryTrimming => {
            cfg!(target_os = "windows") || cfg!(target_os = "linux")
        }
        PlatformFeature::CacheDrop => {
            cfg!(target_os = "linux")
        }
        PlatformFeature::MemoryCompaction => {
            cfg!(target_os = "linux")
        }
        PlatformFeature::GpuNvidia => {
            cfg!(feature = "nvml") || cfg!(target_os = "windows")
        }
        PlatformFeature::GpuAmd => {
            cfg!(target_os = "linux") // ROCm-SMI
        }
        PlatformFeature::SystemTray => {
            cfg!(target_os = "windows") || cfg!(target_os = "linux")
        }
        PlatformFeature::DaemonService => {
            cfg!(target_os = "windows") || cfg!(target_os = "linux")
        }
        PlatformFeature::CgroupsV2 => {
            cfg!(target_os = "linux")
        }
    }
}

/// Platform-specific features that may or may not be available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformFeature {
    /// Process working set trimming
    MemoryTrimming,
    /// Filesystem cache dropping
    CacheDrop,
    /// Memory compaction
    MemoryCompaction,
    /// NVIDIA GPU monitoring
    GpuNvidia,
    /// AMD GPU monitoring
    GpuAmd,
    /// System tray integration
    SystemTray,
    /// Daemon/service management
    DaemonService,
    /// Linux cgroups v2 support
    CgroupsV2,
}

// ============================================================================
// Cross-Platform Utilities
// ============================================================================

/// Format bytes into a human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Parse a human-readable byte string (e.g., "1.5GB") into bytes.
pub fn parse_bytes(s: &str) -> Option<u64> {
    let s = s.trim().to_uppercase();

    let (num_str, multiplier) = if s.ends_with("TB") {
        (&s[..s.len()-2], 1024u64 * 1024 * 1024 * 1024)
    } else if s.ends_with("GB") {
        (&s[..s.len()-2], 1024u64 * 1024 * 1024)
    } else if s.ends_with("MB") {
        (&s[..s.len()-2], 1024u64 * 1024)
    } else if s.ends_with("KB") {
        (&s[..s.len()-2], 1024u64)
    } else if s.ends_with("B") {
        (&s[..s.len()-1], 1u64)
    } else {
        return s.parse().ok();
    };

    num_str.trim().parse::<f64>().ok().map(|n| (n * multiplier as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_bytes(1024u64 * 1024 * 1024 * 1024), "1.00 TB");
    }

    #[test]
    fn test_parse_bytes() {
        assert_eq!(parse_bytes("1024"), Some(1024));
        assert_eq!(parse_bytes("1KB"), Some(1024));
        assert_eq!(parse_bytes("1.5 MB"), Some(1572864));
        assert_eq!(parse_bytes("2GB"), Some(2147483648));
        assert_eq!(parse_bytes("1TB"), Some(1099511627776));
        assert_eq!(parse_bytes("invalid"), None);
    }

    #[test]
    fn test_detect_platform() {
        let info = detect_platform();
        assert!(!info.os_name.is_empty());
        assert!(!info.arch.is_empty());

        #[cfg(target_os = "linux")]
        assert!(info.init_system.is_some());
    }

    #[test]
    fn test_feature_supported() {
        // Memory trimming should be supported on Windows and Linux
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        assert!(is_feature_supported(PlatformFeature::MemoryTrimming));

        // Cache drop is Linux-only
        #[cfg(target_os = "linux")]
        assert!(is_feature_supported(PlatformFeature::CacheDrop));

        #[cfg(target_os = "windows")]
        assert!(!is_feature_supported(PlatformFeature::CacheDrop));
    }
}
