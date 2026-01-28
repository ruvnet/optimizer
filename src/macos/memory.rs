//! macOS Memory Management Core
//!
//! Uses sysinfo crate for cross-platform compatibility with optional
//! Mach-specific enhancements for detailed memory statistics.

use sysinfo::System;
use std::process::Command;
use std::time::Instant;
use tracing::{info, warn, debug};

/// Memory status information (cross-platform compatible)
#[derive(Debug, Clone)]
pub struct MemoryStatus {
    pub total_physical_mb: f64,
    pub available_physical_mb: f64,
    pub memory_load_percent: u32,
    pub total_swap_mb: f64,
    pub available_swap_mb: f64,
    /// macOS specific: memory pressure level (0-4)
    pub pressure_level: u32,
    /// macOS specific: is Apple Silicon
    pub is_apple_silicon: bool,
}

impl MemoryStatus {
    pub fn used_physical_mb(&self) -> f64 {
        self.total_physical_mb - self.available_physical_mb
    }

    pub fn is_high_pressure(&self) -> bool {
        self.memory_load_percent > 80 || self.pressure_level >= 2
    }

    pub fn is_critical(&self) -> bool {
        self.memory_load_percent > 95 || self.pressure_level >= 3
    }
}

/// Optimization result
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub freed_mb: f64,
    pub before_available_mb: f64,
    pub after_available_mb: f64,
    pub processes_affected: usize,
    pub duration_ms: u64,
    pub method: OptimizationMethod,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptimizationMethod {
    /// Used memory_pressure tool (requires sudo)
    PurgeTool,
    /// Used madvise hints on processes
    MadviseHints,
    /// Triggered system memory cleanup
    SystemCleanup,
    /// No optimization performed (dry run or not needed)
    None,
}

/// macOS Memory Optimizer
pub struct MacMemoryOptimizer {
    has_sudo: bool,
    is_apple_silicon: bool,
}

impl MacMemoryOptimizer {
    pub fn new() -> Self {
        let has_sudo = Self::check_sudo();
        let is_apple_silicon = Self::detect_apple_silicon();

        if has_sudo {
            info!("Running with sudo access - full optimization available");
        } else {
            warn!("Running without sudo - limited optimization (madvise hints only)");
        }

        if is_apple_silicon {
            info!("Apple Silicon detected - unified memory architecture");
        }

        Self {
            has_sudo,
            is_apple_silicon,
        }
    }

    /// Check if we have sudo access (for purge command)
    fn check_sudo() -> bool {
        // Check if running as root
        unsafe { libc::geteuid() == 0 }
    }

    /// Detect Apple Silicon (M1/M2/M3/M4)
    fn detect_apple_silicon() -> bool {
        #[cfg(target_arch = "aarch64")]
        {
            true
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            false
        }
    }

    /// Get current memory status
    pub fn get_memory_status() -> Result<MemoryStatus, String> {
        let mut sys = System::new();
        sys.refresh_memory();

        let total = sys.total_memory() as f64 / 1024.0 / 1024.0;
        let available = sys.available_memory() as f64 / 1024.0 / 1024.0;
        let used = sys.used_memory() as f64 / 1024.0 / 1024.0;

        let load = if total > 0.0 {
            ((used / total) * 100.0) as u32
        } else {
            0
        };

        let total_swap = sys.total_swap() as f64 / 1024.0 / 1024.0;
        let used_swap = sys.used_swap() as f64 / 1024.0 / 1024.0;
        let available_swap = total_swap - used_swap;

        // Get memory pressure level from vm_stat or memory_pressure
        let pressure_level = Self::get_memory_pressure_level();

        Ok(MemoryStatus {
            total_physical_mb: total,
            available_physical_mb: available,
            memory_load_percent: load,
            total_swap_mb: total_swap,
            available_swap_mb: available_swap,
            pressure_level,
            is_apple_silicon: Self::detect_apple_silicon(),
        })
    }

    /// Get memory pressure level (0=normal, 1=warn, 2=critical, 3=urgent, 4=extreme)
    fn get_memory_pressure_level() -> u32 {
        // Try to get from memory_pressure command
        if let Ok(output) = Command::new("memory_pressure").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("normal") {
                return 0;
            } else if stdout.contains("warn") {
                return 1;
            } else if stdout.contains("critical") {
                return 2;
            }
        }

        // Fallback: estimate from memory usage
        if let Ok(status) = Self::get_memory_status_basic() {
            if status.memory_load_percent > 95 {
                return 3;
            } else if status.memory_load_percent > 85 {
                return 2;
            } else if status.memory_load_percent > 75 {
                return 1;
            }
        }

        0
    }

    /// Basic memory status without pressure (to avoid recursion)
    fn get_memory_status_basic() -> Result<MemoryStatus, String> {
        let mut sys = System::new();
        sys.refresh_memory();

        let total = sys.total_memory() as f64 / 1024.0 / 1024.0;
        let available = sys.available_memory() as f64 / 1024.0 / 1024.0;
        let used = sys.used_memory() as f64 / 1024.0 / 1024.0;
        let load = if total > 0.0 { ((used / total) * 100.0) as u32 } else { 0 };

        Ok(MemoryStatus {
            total_physical_mb: total,
            available_physical_mb: available,
            memory_load_percent: load,
            total_swap_mb: 0.0,
            available_swap_mb: 0.0,
            pressure_level: 0,
            is_apple_silicon: false,
        })
    }

    /// Run memory optimization
    pub fn optimize(&self, aggressive: bool) -> Result<OptimizationResult, String> {
        let start = Instant::now();
        let before = Self::get_memory_status()?;

        let (method, processes_affected) = if self.has_sudo && aggressive {
            // Use purge command (requires sudo)
            self.run_purge()?
        } else {
            // Use madvise hints on processes
            self.run_madvise_hints()?
        };

        // Wait for memory to settle
        std::thread::sleep(std::time::Duration::from_millis(500));

        let after = Self::get_memory_status()?;
        let freed = after.available_physical_mb - before.available_physical_mb;

        info!(
            "Optimized: method={:?}, affected {} processes, freed {:.1} MB in {}ms",
            method,
            processes_affected,
            freed.max(0.0),
            start.elapsed().as_millis()
        );

        Ok(OptimizationResult {
            freed_mb: freed.max(0.0),
            before_available_mb: before.available_physical_mb,
            after_available_mb: after.available_physical_mb,
            processes_affected,
            duration_ms: start.elapsed().as_millis() as u64,
            method,
        })
    }

    /// Run purge command (requires sudo)
    fn run_purge(&self) -> Result<(OptimizationMethod, usize), String> {
        info!("Running purge command (sudo required)");

        let output = Command::new("sudo")
            .arg("purge")
            .output()
            .map_err(|e| format!("Failed to run purge: {}", e))?;

        if output.status.success() {
            Ok((OptimizationMethod::PurgeTool, 0))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Purge failed: {}", stderr);
            // Fallback to madvise hints
            self.run_madvise_hints()
        }
    }

    /// Use madvise hints to suggest memory cleanup
    fn run_madvise_hints(&self) -> Result<(OptimizationMethod, usize), String> {
        debug!("Using madvise hints for memory optimization");

        // Get list of user processes with high memory usage
        let processes = super::process::list_user_processes()?;
        let mut affected = 0;

        for (pid, _name, memory_mb) in processes.iter().take(50) {
            // Only target processes using significant memory
            if *memory_mb > 100.0 {
                if self.hint_process_memory(*pid) {
                    affected += 1;
                }
            }
        }

        Ok((OptimizationMethod::MadviseHints, affected))
    }

    /// Send memory hint to a process (best effort)
    fn hint_process_memory(&self, _pid: u32) -> bool {
        // On macOS, we can't directly manipulate other process memory
        // without being the parent process or having special entitlements.
        //
        // For now, we rely on:
        // 1. The purge command (sudo)
        // 2. System memory pressure notifications
        // 3. Jetsam (system automatic)
        //
        // Future: Could use XPC to request apps release memory
        false
    }

    /// Clear disk caches (file system caches)
    pub fn clear_disk_cache(&self) -> Result<f64, String> {
        if !self.has_sudo {
            return Err("sudo required to clear disk cache".into());
        }

        let before = Self::get_memory_status()?;

        // sync and purge
        let _ = Command::new("sync").output();
        let _ = Command::new("sudo").arg("purge").output();

        std::thread::sleep(std::time::Duration::from_millis(500));

        let after = Self::get_memory_status()?;
        let freed = after.available_physical_mb - before.available_physical_mb;

        Ok(freed.max(0.0))
    }

    pub fn has_sudo_privileges(&self) -> bool {
        self.has_sudo
    }

    pub fn is_apple_silicon(&self) -> bool {
        self.is_apple_silicon
    }
}

impl Default for MacMemoryOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_status() {
        let status = MacMemoryOptimizer::get_memory_status().unwrap();
        assert!(status.total_physical_mb > 0.0);
        assert!(status.available_physical_mb > 0.0);
        assert!(status.memory_load_percent <= 100);
    }
}
