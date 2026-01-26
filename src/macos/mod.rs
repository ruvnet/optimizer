//! macOS Platform Support
//!
//! This module provides macOS-specific functionality for the optimizer,
//! including memory management, process enumeration, and system information.
//!
//! The implementations use the cross-platform `sysinfo` crate, which
//! provides native access to macOS system APIs through the IOKit and
//! sysctl interfaces.
//!
//! # Modules
//!
//! - [`memory`] - Memory monitoring and optimization
//! - [`process`] - Process enumeration and information
//! - [`safety`] - Safety mechanisms to prevent system instability
//! - [`system`] - System-level information and metrics
//!
//! # Example
//!
//! ```no_run
//! use optimizer::macos::{list_processes, get_system_info, MacOSMemoryOptimizer};
//!
//! // Get memory status
//! let status = MacOSMemoryOptimizer::get_memory_status().unwrap();
//! println!("Memory load: {}%", status.memory_load_percent);
//!
//! // List all processes
//! let pids = list_processes().expect("Failed to list processes");
//! println!("Found {} processes", pids.len());
//!
//! // Get system information
//! let info = get_system_info();
//! println!("Running on {} ({} cores)", info.os_name, info.cpu_count);
//! ```

pub mod memory;
pub mod process;
pub mod safety;
pub mod system;

// Re-export commonly used items
pub use memory::{MacOSMemoryOptimizer, MemoryStatus, OptimizationResult};
pub use process::{
    get_parent_pid, get_process_cpu_usage, get_process_memory, get_process_name, list_processes,
};
pub use safety::{SafetyConfig, SafetyGuard, SafetyStats};
pub use system::{
    get_boot_time, get_cpu_arch, get_long_os_version, get_os_version, get_system_info,
    get_total_swap_mb, get_uptime_seconds, get_used_swap_mb, is_apple_silicon, SystemInfo,
};
