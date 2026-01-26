//! macOS Memory Management Core
//!
//! This module provides memory monitoring and optimization for macOS systems.
//! It mirrors the Windows API but uses macOS-appropriate mechanisms:
//!
//! - Native Mach APIs (`host_statistics64`) for accurate memory statistics
//! - `madvise` for memory hints (MADV_FREE, MADV_DONTNEED)
//! - `purge` command for disk cache clearing (requires root)
//! - Memory pressure level detection
//!
//! # Key Differences from Windows
//!
//! Unlike Windows `SetProcessWorkingSetSizeEx`, macOS doesn't provide direct
//! APIs to trim another process's working set. The kernel (Jetsam/memory
//! compressor) handles this automatically. This module focuses on:
//!
//! 1. **Monitoring**: Accurate memory status reporting
//! 2. **Signaling**: Notifying processes of memory pressure
//! 3. **Purging**: Clearing disk caches (with privileges)
//! 4. **Hints**: Using madvise for current process memory management
//!
//! # Examples
//!
//! ```no_run
//! use optimizer::macos::memory::{MacOSMemoryOptimizer, MemoryStatus};
//!
//! // Get current memory status
//! let status = MacOSMemoryOptimizer::get_memory_status().unwrap();
//! println!("Memory load: {}%", status.memory_load_percent);
//!
//! // Create optimizer and run optimization
//! let optimizer = MacOSMemoryOptimizer::new();
//! let result = optimizer.optimize(false).unwrap();
//! println!("Freed: {:.1} MB", result.freed_mb);
//! ```

use std::time::Instant;
use tracing::{debug, info, warn};

#[cfg(target_os = "macos")]
use std::mem::MaybeUninit;

/// Memory status information for macOS
///
/// This struct provides comprehensive memory statistics that mirror
/// the Windows `MemoryStatus` struct for API compatibility.
#[derive(Debug, Clone)]
pub struct MemoryStatus {
    /// Total physical RAM in megabytes
    pub total_physical_mb: f64,
    /// Available (free + inactive) physical RAM in megabytes
    pub available_physical_mb: f64,
    /// Memory load as percentage (0-100)
    pub memory_load_percent: u32,
    /// Total swap space in MB (macOS equivalent of Windows page file)
    pub total_page_file_mb: f64,
    /// Available swap space in MB
    pub available_page_file_mb: f64,
    /// Total virtual memory in MB
    pub total_virtual_mb: f64,
    /// Available virtual memory in MB
    pub available_virtual_mb: f64,
}

impl MemoryStatus {
    /// Calculate used physical memory in MB
    #[inline]
    pub fn used_physical_mb(&self) -> f64 {
        self.total_physical_mb - self.available_physical_mb
    }

    /// Check if memory is under high pressure (>80% used)
    #[inline]
    pub fn is_high_pressure(&self) -> bool {
        self.memory_load_percent > 80
    }

    /// Check if memory is in critical state (>95% used)
    #[inline]
    pub fn is_critical(&self) -> bool {
        self.memory_load_percent > 95
    }

    /// Get a human-readable pressure description
    pub fn pressure_description(&self) -> &'static str {
        if self.is_critical() {
            "critical"
        } else if self.is_high_pressure() {
            "high"
        } else if self.memory_load_percent > 60 {
            "moderate"
        } else {
            "normal"
        }
    }
}

/// Result of a memory optimization operation
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// Amount of memory freed in MB (may be 0 if kernel handles it)
    pub freed_mb: f64,
    /// Available memory before optimization in MB
    pub before_available_mb: f64,
    /// Available memory after optimization in MB
    pub after_available_mb: f64,
    /// Number of processes signaled/processed
    pub processes_trimmed: usize,
    /// Duration of the optimization in milliseconds
    pub duration_ms: u64,
}

/// VM statistics structure matching mach/vm_statistics.h
/// IMPORTANT: natural_t is 32-bit (unsigned int) on macOS, even on 64-bit systems
/// The structure has a mix of natural_t (u32) and uint64_t (u64) fields
#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Debug, Default)]
struct VmStatistics64 {
    free_count: u32,           // natural_t
    active_count: u32,         // natural_t
    inactive_count: u32,       // natural_t
    wire_count: u32,           // natural_t
    zero_fill_count: u64,      // uint64_t
    reactivations: u64,        // uint64_t
    pageins: u64,              // uint64_t
    pageouts: u64,             // uint64_t
    faults: u64,               // uint64_t
    cow_faults: u64,           // uint64_t
    lookups: u64,              // uint64_t
    hits: u64,                 // uint64_t
    purges: u64,               // uint64_t
    purgeable_count: u32,      // natural_t
    speculative_count: u32,    // natural_t
    decompressions: u64,       // uint64_t
    compressions: u64,         // uint64_t
    swapins: u64,              // uint64_t
    swapouts: u64,             // uint64_t
    compressor_page_count: u32, // natural_t
    throttled_count: u32,      // natural_t
    external_page_count: u32,  // natural_t
    internal_page_count: u32,  // natural_t
    total_uncompressed_pages_in_compressor: u64, // uint64_t
}

/// macOS Memory Optimizer
///
/// Provides memory monitoring and optimization capabilities for macOS.
/// Unlike Windows, macOS doesn't provide direct APIs to trim other processes'
/// working sets. Instead, this optimizer:
///
/// 1. Monitors memory status using `sysinfo`
/// 2. Uses SIGINFO signals to encourage memory release
/// 3. Can purge disk caches with root privileges
/// 4. Provides memory advice APIs for current process
pub struct MacOSMemoryOptimizer {
    /// Whether running with root/admin privileges
    has_admin: bool,
}

impl MacOSMemoryOptimizer {
    /// Create a new macOS memory optimizer
    ///
    /// Checks for root privileges on creation and logs the status.
    pub fn new() -> Self {
        let has_admin = Self::check_admin();
        if !has_admin {
            warn!("Running without admin - limited optimization");
        } else {
            info!("Running with admin privileges - full optimization available");
        }
        Self { has_admin }
    }

    /// Check if running as root or with appropriate privileges
    #[cfg(target_os = "macos")]
    fn check_admin() -> bool {
        // Check if effective user ID is 0 (root)
        // SAFETY: geteuid is always safe to call
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(not(target_os = "macos"))]
    fn check_admin() -> bool {
        false
    }

    /// Get current memory status using native macOS Mach APIs
    ///
    /// Uses `host_statistics64` with `HOST_VM_INFO64` for accurate memory data.
    /// This provides more reliable results than cross-platform libraries on macOS.
    ///
    /// Available memory is calculated as: free + inactive + speculative + purgeable
    /// This matches what Activity Monitor shows as "Memory Available".
    ///
    /// # Returns
    /// - `Ok(MemoryStatus)` - Current memory statistics
    /// - `Err(String)` - Error message if query fails
    #[cfg(target_os = "macos")]
    pub fn get_memory_status() -> Result<MemoryStatus, String> {
        // Get total physical memory via sysctl
        let total_bytes = Self::get_total_memory_bytes()?;
        let total_mb = total_bytes as f64 / 1024.0 / 1024.0;

        // Get VM statistics via Mach API
        let vm_stats = Self::get_vm_statistics()?;
        let page_size = Self::get_page_size()? as u64;

        // Calculate memory values in bytes (counts are u32, need to cast to u64 before multiply)
        let free_bytes = (vm_stats.free_count as u64) * page_size;
        let inactive_bytes = (vm_stats.inactive_count as u64) * page_size;
        let speculative_bytes = (vm_stats.speculative_count as u64) * page_size;
        let purgeable_bytes = (vm_stats.purgeable_count as u64) * page_size;

        // Available = free + inactive + speculative + purgeable
        // This matches Activity Monitor's "Memory Available" calculation
        let available_bytes = free_bytes + inactive_bytes + speculative_bytes + purgeable_bytes;
        let available_mb = available_bytes as f64 / 1024.0 / 1024.0;

        let load = if total_mb > 0.0 {
            (((total_mb - available_mb) / total_mb) * 100.0).round() as u32
        } else {
            0
        };

        // Get swap info
        let (swap_total, swap_used) = Self::get_swap_info();
        let swap_free = swap_total - swap_used;

        debug!(
            "Memory: total={:.0}MB, avail={:.0}MB (free={:.0}MB, inactive={:.0}MB, spec={:.0}MB, purg={:.0}MB), load={}%",
            total_mb,
            available_mb,
            free_bytes as f64 / 1024.0 / 1024.0,
            inactive_bytes as f64 / 1024.0 / 1024.0,
            speculative_bytes as f64 / 1024.0 / 1024.0,
            purgeable_bytes as f64 / 1024.0 / 1024.0,
            load
        );

        Ok(MemoryStatus {
            total_physical_mb: total_mb,
            available_physical_mb: available_mb,
            memory_load_percent: load,
            total_page_file_mb: swap_total,
            available_page_file_mb: swap_free,
            total_virtual_mb: total_mb + swap_total,
            available_virtual_mb: available_mb + swap_free,
        })
    }

    #[cfg(not(target_os = "macos"))]
    pub fn get_memory_status() -> Result<MemoryStatus, String> {
        Err("Not supported on this platform".to_string())
    }

    /// Get total physical memory in bytes using sysctl
    #[cfg(target_os = "macos")]
    fn get_total_memory_bytes() -> Result<u64, String> {
        use std::ptr;

        let mut size: libc::size_t = std::mem::size_of::<u64>();
        let mut memsize: u64 = 0;
        let mut mib: [libc::c_int; 2] = [libc::CTL_HW, libc::HW_MEMSIZE];

        let result = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                2,
                &mut memsize as *mut u64 as *mut libc::c_void,
                &mut size,
                ptr::null_mut(),
                0,
            )
        };

        if result == 0 {
            Ok(memsize)
        } else {
            Err("Failed to get total memory via sysctl".to_string())
        }
    }

    /// Get VM statistics using Mach host_statistics64
    #[cfg(target_os = "macos")]
    fn get_vm_statistics() -> Result<VmStatistics64, String> {
        // Mach constants
        const HOST_VM_INFO64: libc::c_int = 4;
        const HOST_VM_INFO64_COUNT: u32 =
            (std::mem::size_of::<VmStatistics64>() / std::mem::size_of::<i32>()) as u32;

        extern "C" {
            fn mach_host_self() -> u32;
            fn host_statistics64(
                host: u32,
                flavor: libc::c_int,
                info: *mut VmStatistics64,
                count: *mut u32,
            ) -> i32;
        }

        let mut stats: MaybeUninit<VmStatistics64> = MaybeUninit::uninit();
        let mut count: u32 = HOST_VM_INFO64_COUNT;

        let result = unsafe {
            host_statistics64(
                mach_host_self(),
                HOST_VM_INFO64,
                stats.as_mut_ptr(),
                &mut count,
            )
        };

        if result == 0 {
            Ok(unsafe { stats.assume_init() })
        } else {
            Err(format!("host_statistics64 failed with error: {}", result))
        }
    }

    /// Get page size using sysconf
    #[cfg(target_os = "macos")]
    fn get_page_size() -> Result<usize, String> {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if page_size > 0 {
            Ok(page_size as usize)
        } else {
            // Default to 16KB for Apple Silicon, 4KB for Intel
            Ok(16384)
        }
    }

    /// Get swap info using sysinfo crate (swap detection is reliable)
    #[cfg(target_os = "macos")]
    fn get_swap_info() -> (f64, f64) {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_memory();

        let swap_total = sys.total_swap() as f64 / 1024.0 / 1024.0;
        let swap_used = sys.used_swap() as f64 / 1024.0 / 1024.0;
        (swap_total, swap_used)
    }

    /// Trim working set for a specific process
    ///
    /// This function provides API compatibility with the Windows implementation.
    /// On macOS, we cannot directly trim another process's working set without
    /// using private Mach APIs. Instead, we:
    ///
    /// 1. Send SIGINFO to the process (some apps respond by freeing caches)
    /// 2. Let the kernel's Jetsam handle actual memory reclamation
    ///
    /// # Arguments
    /// * `pid` - The process ID to signal
    ///
    /// # Returns
    /// * `Ok(u64)` - Estimated bytes freed (usually 0 as kernel handles it)
    /// * `Err(String)` - Error message if operation fails
    #[cfg(target_os = "macos")]
    pub fn trim_process_working_set(pid: u32) -> Result<u64, String> {
        use std::process::Command;

        // Skip system-critical processes
        if pid <= 1 {
            return Ok(0);
        }

        // Get memory before (for estimation)
        let mem_before = super::process::get_process_memory(pid).unwrap_or(0);

        // Send SIGINFO signal - this is a gentle way to signal the process
        // Some applications respond to this by reporting status and/or
        // releasing cached memory
        let result = Command::new("kill")
            .args(["-INFO", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match result {
            Ok(status) => {
                if status.success() {
                    // Small delay to allow process to respond
                    std::thread::sleep(std::time::Duration::from_millis(5));

                    // Get memory after
                    let mem_after = super::process::get_process_memory(pid).unwrap_or(mem_before);
                    let freed = mem_before.saturating_sub(mem_after);

                    if freed > 0 {
                        debug!("Process {} released {} bytes after signal", pid, freed);
                    }

                    Ok(freed)
                } else {
                    // Process may not exist or we don't have permission - that's OK
                    Ok(0)
                }
            }
            Err(_) => {
                // Failed to execute kill command - not critical
                Ok(0)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn trim_process_working_set(_pid: u32) -> Result<u64, String> {
        Ok(0)
    }

    /// Purge inactive memory on macOS using the `purge` command
    ///
    /// This clears the disk cache and other purgeable memory.
    /// Requires root privileges to execute.
    ///
    /// # Returns
    /// * `Ok(u64)` - Always 0 (purge doesn't report freed amount)
    /// * `Err(String)` - Error if purge command fails
    #[cfg(target_os = "macos")]
    pub fn purge_inactive_memory() -> Result<u64, String> {
        use std::process::Command;

        // The `purge` command requires root privileges
        let output = Command::new("purge")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .status();

        match output {
            Ok(status) => {
                if status.success() {
                    info!("System purge completed successfully");
                    Ok(0)
                } else {
                    Err("purge command failed (requires root)".to_string())
                }
            }
            Err(e) => Err(format!("Failed to execute purge: {}", e)),
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn purge_inactive_memory() -> Result<u64, String> {
        Ok(0)
    }

    /// Apply memory advice using madvise
    ///
    /// Provides hints to the kernel about memory usage for the current process.
    /// This is the macOS equivalent of memory optimization for the optimizer itself.
    ///
    /// # Arguments
    /// * `addr` - Start address of the memory region
    /// * `len` - Length of the memory region in bytes
    /// * `advice` - madvise constant (MADV_FREE, MADV_DONTNEED, etc.)
    ///
    /// # Safety
    /// Caller must ensure the memory region is valid and properly aligned.
    #[cfg(target_os = "macos")]
    pub unsafe fn madvise_memory(
        addr: *mut libc::c_void,
        len: usize,
        advice: i32,
    ) -> Result<(), String> {
        if addr.is_null() || len == 0 {
            return Err("Invalid memory region".to_string());
        }

        let result = libc::madvise(addr, len, advice);
        if result == 0 {
            Ok(())
        } else {
            let errno = *libc::__error();
            Err(format!("madvise failed with errno: {}", errno))
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub unsafe fn madvise_memory(
        _addr: *mut libc::c_void,
        _len: usize,
        _advice: i32,
    ) -> Result<(), String> {
        Err("madvise not available on this platform".to_string())
    }

    /// Get the MADV_FREE constant for memory that can be freed
    #[cfg(target_os = "macos")]
    pub fn madv_free() -> i32 {
        libc::MADV_FREE
    }

    #[cfg(not(target_os = "macos"))]
    pub fn madv_free() -> i32 {
        0
    }

    /// Get the MADV_DONTNEED constant for pages not needed
    #[cfg(target_os = "macos")]
    pub fn madv_dontneed() -> i32 {
        libc::MADV_DONTNEED
    }

    #[cfg(not(target_os = "macos"))]
    pub fn madv_dontneed() -> i32 {
        0
    }

    /// Optimize memory on macOS
    ///
    /// Performs memory optimization by:
    /// 1. Measuring current memory state
    /// 2. Running system purge if root and aggressive mode
    /// 3. Signaling processes to release memory
    /// 4. Measuring final memory state
    ///
    /// # Arguments
    /// * `aggressive` - If true, attempts more aggressive optimization
    ///
    /// # Returns
    /// * `Ok(OptimizationResult)` - Statistics about the optimization
    /// * `Err(String)` - Error if optimization fails
    pub fn optimize(&self, aggressive: bool) -> Result<OptimizationResult, String> {
        let start = Instant::now();
        let before = Self::get_memory_status()?;
        let mut trimmed = 0usize;
        let mut total_freed: u64 = 0;

        info!(
            "Starting macOS memory optimization (aggressive={}, admin={})",
            aggressive, self.has_admin
        );

        // Step 1: Purge inactive memory if we have admin privileges
        if self.has_admin && aggressive {
            match Self::purge_inactive_memory() {
                Ok(_) => {
                    debug!("Purged inactive memory");
                }
                Err(e) => {
                    warn!("System purge failed: {}", e);
                }
            }
        }

        // Step 2: Signal processes to release memory
        if let Ok(procs) = super::process::list_processes() {
            // Limit to avoid overwhelming the system
            let process_limit = if aggressive { 200 } else { 150 };

            for pid in procs.iter().take(process_limit) {
                // Skip critical system processes
                if *pid <= 1 {
                    continue;
                }

                match Self::trim_process_working_set(*pid) {
                    Ok(freed) => {
                        if freed > 0 {
                            total_freed += freed;
                            trimmed += 1;
                        }
                    }
                    Err(_) => {
                        // Ignore errors - process may have exited or we lack permissions
                    }
                }
            }
        }

        // Step 3: Allow time for memory to be reclaimed
        let delay = if aggressive { 150 } else { 100 };
        std::thread::sleep(std::time::Duration::from_millis(delay));

        // Step 4: Measure results
        let after = Self::get_memory_status()?;
        let measured_freed = after.available_physical_mb - before.available_physical_mb;
        let calculated_freed = total_freed as f64 / 1024.0 / 1024.0;
        let freed_mb = measured_freed.max(calculated_freed).max(0.0);

        info!(
            "Optimized: signaled {} processes, freed {:.1} MB in {}ms",
            trimmed,
            freed_mb,
            start.elapsed().as_millis()
        );

        Ok(OptimizationResult {
            freed_mb,
            before_available_mb: before.available_physical_mb,
            after_available_mb: after.available_physical_mb,
            processes_trimmed: trimmed,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Check if the optimizer has admin/root privileges
    #[inline]
    pub fn has_admin_privileges(&self) -> bool {
        self.has_admin
    }
}

impl Default for MacOSMemoryOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_status() {
        let status = MacOSMemoryOptimizer::get_memory_status();
        assert!(status.is_ok());

        let status = status.unwrap();
        assert!(status.total_physical_mb > 0.0);
        assert!(status.available_physical_mb >= 0.0);
        assert!(status.available_physical_mb <= status.total_physical_mb);
        assert!(status.memory_load_percent <= 100);
    }

    #[test]
    fn test_used_physical_mb() {
        let status = MemoryStatus {
            total_physical_mb: 16000.0,
            available_physical_mb: 8000.0,
            memory_load_percent: 50,
            total_page_file_mb: 4000.0,
            available_page_file_mb: 4000.0,
            total_virtual_mb: 20000.0,
            available_virtual_mb: 12000.0,
        };

        assert!((status.used_physical_mb() - 8000.0).abs() < 0.001);
    }

    #[test]
    fn test_memory_pressure_levels() {
        let mut status = MemoryStatus {
            total_physical_mb: 16000.0,
            available_physical_mb: 8000.0,
            memory_load_percent: 50,
            total_page_file_mb: 4000.0,
            available_page_file_mb: 4000.0,
            total_virtual_mb: 20000.0,
            available_virtual_mb: 12000.0,
        };

        assert!(!status.is_high_pressure());
        assert!(!status.is_critical());
        assert_eq!(status.pressure_description(), "normal");

        status.memory_load_percent = 65;
        assert!(!status.is_high_pressure());
        assert_eq!(status.pressure_description(), "moderate");

        status.memory_load_percent = 85;
        assert!(status.is_high_pressure());
        assert!(!status.is_critical());
        assert_eq!(status.pressure_description(), "high");

        status.memory_load_percent = 97;
        assert!(status.is_high_pressure());
        assert!(status.is_critical());
        assert_eq!(status.pressure_description(), "critical");
    }

    #[test]
    fn test_optimizer_creation() {
        let optimizer = MacOSMemoryOptimizer::new();
        // Just verify it doesn't panic
        let _ = optimizer.has_admin_privileges();
    }

    #[test]
    fn test_trim_process_returns_ok() {
        // Should return Ok for any process (even if it doesn't exist)
        let result = MacOSMemoryOptimizer::trim_process_working_set(1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_trim_process_skips_pid_zero() {
        // PID 0 is the kernel, should be skipped
        let result = MacOSMemoryOptimizer::trim_process_working_set(0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_default_impl() {
        let optimizer = MacOSMemoryOptimizer::default();
        let _ = optimizer.has_admin_privileges();
    }

    #[test]
    fn test_madvise_constants() {
        // Just verify constants are accessible
        let _ = MacOSMemoryOptimizer::madv_free();
        let _ = MacOSMemoryOptimizer::madv_dontneed();
    }
}
