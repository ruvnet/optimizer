//! Android Memory Information via /proc filesystem
//!
//! Parses /proc/meminfo for system-wide memory statistics and
//! /proc/[pid]/status for per-process memory information.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

// Import error types from parent module
pub use super::{AndroidError, AndroidResult};

/// System-wide memory status
#[derive(Debug, Clone)]
pub struct MemoryStatus {
    /// Total physical memory in MB
    pub total_physical_mb: f64,
    /// Available physical memory in MB
    pub available_physical_mb: f64,
    /// Memory load percentage (0-100)
    pub memory_load_percent: u32,
    /// Free memory (not including cached/buffers) in MB
    pub free_mb: f64,
    /// Buffer memory in MB
    pub buffers_mb: f64,
    /// Cached memory in MB
    pub cached_mb: f64,
    /// Swap total in MB
    pub swap_total_mb: f64,
    /// Swap free in MB
    pub swap_free_mb: f64,
    /// Shared memory in MB
    pub shared_mb: f64,
    /// Slab memory in MB
    pub slab_mb: f64,
}

impl MemoryStatus {
    /// Calculate used physical memory
    pub fn used_physical_mb(&self) -> f64 {
        self.total_physical_mb - self.available_physical_mb
    }

    /// Check if memory is under high pressure (>80% used)
    pub fn is_high_pressure(&self) -> bool {
        self.memory_load_percent > 80
    }

    /// Check if memory is critical (>95% used)
    pub fn is_critical(&self) -> bool {
        self.memory_load_percent > 95
    }

    /// Get swap usage percentage
    pub fn swap_usage_percent(&self) -> u32 {
        if self.swap_total_mb > 0.0 {
            (((self.swap_total_mb - self.swap_free_mb) / self.swap_total_mb) * 100.0) as u32
        } else {
            0
        }
    }

    /// Check if swap is being heavily used (>50%)
    pub fn is_swap_heavy(&self) -> bool {
        self.swap_usage_percent() > 50
    }
}

/// Result of an optimization attempt
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// Memory freed in MB (0 on Android - we can't directly free)
    pub freed_mb: f64,
    /// Available memory before operation in MB
    pub before_available_mb: f64,
    /// Available memory after operation in MB
    pub after_available_mb: f64,
    /// Number of processes trimmed (0 on Android)
    pub processes_trimmed: usize,
    /// Duration of the operation in milliseconds
    pub duration_ms: u64,
    /// Optional note about the operation
    pub note: Option<String>,
}

/// Per-process memory information from /proc/[pid]/status
#[derive(Debug, Clone)]
pub struct ProcessMemoryInfo {
    /// Process ID
    pub pid: u32,
    /// Virtual memory size in KB
    pub vm_size_kb: u64,
    /// Resident set size (physical memory) in KB
    pub vm_rss_kb: u64,
    /// Peak virtual memory size in KB
    pub vm_peak_kb: u64,
    /// Shared memory in KB
    pub vm_shared_kb: u64,
    /// Data segment size in KB
    pub vm_data_kb: u64,
    /// Stack size in KB
    pub vm_stk_kb: u64,
    /// Text (code) segment size in KB
    pub vm_exe_kb: u64,
    /// Swapped out memory in KB
    pub vm_swap_kb: u64,
}

impl ProcessMemoryInfo {
    /// Get RSS in MB
    pub fn rss_mb(&self) -> f64 {
        self.vm_rss_kb as f64 / 1024.0
    }

    /// Get virtual size in MB
    pub fn virt_mb(&self) -> f64 {
        self.vm_size_kb as f64 / 1024.0
    }

    /// Get memory efficiency (RSS / VmSize)
    pub fn memory_efficiency(&self) -> f64 {
        if self.vm_size_kb > 0 {
            self.vm_rss_kb as f64 / self.vm_size_kb as f64
        } else {
            0.0
        }
    }
}

/// Memory information reader
pub struct MemoryInfo;

impl MemoryInfo {
    /// Read system-wide memory information from /proc/meminfo
    pub fn read_system_memory() -> AndroidResult<MemoryStatus> {
        let content = fs::read_to_string("/proc/meminfo").map_err(|e| {
            AndroidError::ProcReadError(format!("Failed to read /proc/meminfo: {}", e))
        })?;

        Self::parse_meminfo(&content)
    }

    /// Parse /proc/meminfo content
    fn parse_meminfo(content: &str) -> AndroidResult<MemoryStatus> {
        let mut values: HashMap<&str, u64> = HashMap::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                // Remove trailing colon from key
                let key = parts[0].trim_end_matches(':');
                if let Ok(value) = parts[1].parse::<u64>() {
                    values.insert(key, value);
                }
            }
        }

        // Get required values (all in kB from /proc/meminfo)
        let mem_total = *values.get("MemTotal").ok_or_else(|| {
            AndroidError::MemoryParseError("MemTotal not found in /proc/meminfo".to_string())
        })?;

        let mem_free = *values.get("MemFree").unwrap_or(&0);
        let mem_available = *values.get("MemAvailable").unwrap_or(&mem_free);
        let buffers = *values.get("Buffers").unwrap_or(&0);
        let cached = *values.get("Cached").unwrap_or(&0);
        let swap_total = *values.get("SwapTotal").unwrap_or(&0);
        let swap_free = *values.get("SwapFree").unwrap_or(&0);
        let shmem = *values.get("Shmem").unwrap_or(&0);
        let slab = *values.get("Slab").unwrap_or(&0);

        // Convert from kB to MB
        let total_mb = mem_total as f64 / 1024.0;
        let available_mb = mem_available as f64 / 1024.0;
        let free_mb = mem_free as f64 / 1024.0;

        // Calculate memory load percentage
        let memory_load = if total_mb > 0.0 {
            (((total_mb - available_mb) / total_mb) * 100.0) as u32
        } else {
            0
        };

        Ok(MemoryStatus {
            total_physical_mb: total_mb,
            available_physical_mb: available_mb,
            memory_load_percent: memory_load.min(100),
            free_mb,
            buffers_mb: buffers as f64 / 1024.0,
            cached_mb: cached as f64 / 1024.0,
            swap_total_mb: swap_total as f64 / 1024.0,
            swap_free_mb: swap_free as f64 / 1024.0,
            shared_mb: shmem as f64 / 1024.0,
            slab_mb: slab as f64 / 1024.0,
        })
    }

    /// Read memory information for a specific process
    pub fn read_process_memory(pid: u32) -> AndroidResult<ProcessMemoryInfo> {
        let status_path = format!("/proc/{}/status", pid);
        let path = Path::new(&status_path);

        if !path.exists() {
            return Err(AndroidError::ProcessNotFound(pid));
        }

        let content = fs::read_to_string(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                AndroidError::PermissionDenied(format!(
                    "Cannot read /proc/{}/status: permission denied",
                    pid
                ))
            } else {
                AndroidError::ProcReadError(format!(
                    "Failed to read /proc/{}/status: {}",
                    pid, e
                ))
            }
        })?;

        Self::parse_process_status(pid, &content)
    }

    /// Parse /proc/[pid]/status content
    fn parse_process_status(pid: u32, content: &str) -> AndroidResult<ProcessMemoryInfo> {
        let mut info = ProcessMemoryInfo {
            pid,
            vm_size_kb: 0,
            vm_rss_kb: 0,
            vm_peak_kb: 0,
            vm_shared_kb: 0,
            vm_data_kb: 0,
            vm_stk_kb: 0,
            vm_exe_kb: 0,
            vm_swap_kb: 0,
        };

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let key = parts[0].trim_end_matches(':');
                // Values are in kB
                if let Ok(value) = parts[1].parse::<u64>() {
                    match key {
                        "VmSize" => info.vm_size_kb = value,
                        "VmRSS" => info.vm_rss_kb = value,
                        "VmPeak" => info.vm_peak_kb = value,
                        "RssFile" | "VmLib" => info.vm_shared_kb += value,
                        "VmData" => info.vm_data_kb = value,
                        "VmStk" => info.vm_stk_kb = value,
                        "VmExe" => info.vm_exe_kb = value,
                        "VmSwap" => info.vm_swap_kb = value,
                        _ => {}
                    }
                }
            }
        }

        Ok(info)
    }

    /// Read memory from /proc/[pid]/statm (faster but less detailed)
    pub fn read_process_memory_fast(pid: u32) -> AndroidResult<(u64, u64)> {
        let statm_path = format!("/proc/{}/statm", pid);
        let path = Path::new(&statm_path);

        if !path.exists() {
            return Err(AndroidError::ProcessNotFound(pid));
        }

        let content = fs::read_to_string(path).map_err(|e| {
            AndroidError::ProcReadError(format!("Failed to read /proc/{}/statm: {}", pid, e))
        })?;

        Self::parse_statm(&content, pid)
    }

    /// Parse /proc/[pid]/statm content
    /// Returns (virtual_size_pages, rss_pages)
    fn parse_statm(content: &str, pid: u32) -> AndroidResult<(u64, u64)> {
        let parts: Vec<&str> = content.split_whitespace().collect();

        if parts.len() < 2 {
            return Err(AndroidError::MemoryParseError(format!(
                "Invalid statm format for pid {}",
                pid
            )));
        }

        let virt_pages = parts[0].parse::<u64>().map_err(|_| {
            AndroidError::MemoryParseError(format!("Invalid virtual size in statm for pid {}", pid))
        })?;

        let rss_pages = parts[1].parse::<u64>().map_err(|_| {
            AndroidError::MemoryParseError(format!("Invalid RSS in statm for pid {}", pid))
        })?;

        // Convert from pages to KB (typically 4KB pages on Android)
        let page_size = Self::get_page_size();
        let virt_kb = virt_pages * page_size / 1024;
        let rss_kb = rss_pages * page_size / 1024;

        Ok((virt_kb, rss_kb))
    }

    /// Get system page size
    fn get_page_size() -> u64 {
        #[cfg(target_os = "android")]
        {
            unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 }
        }
        #[cfg(not(target_os = "android"))]
        {
            4096 // Default 4KB pages
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MEMINFO: &str = r#"MemTotal:        3956112 kB
MemFree:          125648 kB
MemAvailable:    1234567 kB
Buffers:          123456 kB
Cached:          1456789 kB
SwapCached:        12345 kB
Active:          2345678 kB
Inactive:        1234567 kB
Active(anon):    1234567 kB
Inactive(anon):   234567 kB
Active(file):    1111111 kB
Inactive(file):  1000000 kB
Unevictable:           0 kB
Mlocked:               0 kB
SwapTotal:       2097148 kB
SwapFree:        1897148 kB
Dirty:               123 kB
Writeback:             0 kB
AnonPages:       1234567 kB
Mapped:           345678 kB
Shmem:            234567 kB
Slab:             123456 kB
"#;

    const SAMPLE_STATUS: &str = r#"Name:   example_process
State:  S (sleeping)
Tgid:   1234
Pid:    1234
PPid:   1
VmPeak:     102400 kB
VmSize:      98304 kB
VmLck:           0 kB
VmPin:           0 kB
VmHWM:       51200 kB
VmRSS:       48640 kB
VmData:      32768 kB
VmStk:         136 kB
VmExe:        1024 kB
VmLib:        8192 kB
VmPTE:         256 kB
VmSwap:        512 kB
"#;

    #[test]
    fn test_parse_meminfo() {
        let result = MemoryInfo::parse_meminfo(SAMPLE_MEMINFO).unwrap();

        // MemTotal: 3956112 kB = ~3863.39 MB
        assert!((result.total_physical_mb - 3863.39).abs() < 0.1);

        // MemAvailable: 1234567 kB = ~1205.63 MB
        assert!((result.available_physical_mb - 1205.63).abs() < 0.1);

        // Memory load should be ~69% (3863 - 1205) / 3863
        assert!(result.memory_load_percent >= 68 && result.memory_load_percent <= 70);

        // Check swap
        assert!((result.swap_total_mb - 2048.0).abs() < 1.0);

        // Check buffers and cached
        assert!(result.buffers_mb > 100.0);
        assert!(result.cached_mb > 1000.0);
    }

    #[test]
    fn test_parse_process_status() {
        let result = MemoryInfo::parse_process_status(1234, SAMPLE_STATUS).unwrap();

        assert_eq!(result.pid, 1234);
        assert_eq!(result.vm_size_kb, 98304);
        assert_eq!(result.vm_rss_kb, 48640);
        assert_eq!(result.vm_peak_kb, 102400);
        assert_eq!(result.vm_data_kb, 32768);
        assert_eq!(result.vm_stk_kb, 136);
        assert_eq!(result.vm_exe_kb, 1024);
        assert_eq!(result.vm_swap_kb, 512);

        // VmLib contributes to shared memory
        assert_eq!(result.vm_shared_kb, 8192);

        // Test convenience methods
        assert!((result.rss_mb() - 47.5).abs() < 0.1);
        assert!((result.virt_mb() - 96.0).abs() < 0.1);
    }

    #[test]
    fn test_parse_statm() {
        // Format: size resident shared text lib data dt
        // 24576 12160 2048 256 0 8192 0
        let content = "24576 12160 2048 256 0 8192 0";
        let result = MemoryInfo::parse_statm(content, 1234).unwrap();

        // Values are in pages, converted to KB (assuming 4KB pages)
        // 24576 pages * 4 = 98304 KB
        assert_eq!(result.0, 24576 * 4);
        // 12160 pages * 4 = 48640 KB
        assert_eq!(result.1, 12160 * 4);
    }

    #[test]
    fn test_memory_status_pressure() {
        let mut status = MemoryStatus {
            total_physical_mb: 4096.0,
            available_physical_mb: 1024.0,
            memory_load_percent: 75,
            free_mb: 512.0,
            buffers_mb: 128.0,
            cached_mb: 512.0,
            swap_total_mb: 2048.0,
            swap_free_mb: 1024.0,
            shared_mb: 64.0,
            slab_mb: 128.0,
        };

        // 75% - not high pressure
        assert!(!status.is_high_pressure());
        assert!(!status.is_critical());

        // Change to 85% - high pressure
        status.memory_load_percent = 85;
        assert!(status.is_high_pressure());
        assert!(!status.is_critical());

        // Change to 96% - critical
        status.memory_load_percent = 96;
        assert!(status.is_high_pressure());
        assert!(status.is_critical());

        // Check swap usage (50%)
        assert_eq!(status.swap_usage_percent(), 50);
        assert!(!status.is_swap_heavy());

        // Reduce swap free to make it heavy
        status.swap_free_mb = 512.0;
        assert_eq!(status.swap_usage_percent(), 75);
        assert!(status.is_swap_heavy());
    }

    #[test]
    fn test_memory_parse_error_missing_memtotal() {
        let content = "MemFree: 12345 kB\nMemAvailable: 12345 kB\n";
        let result = MemoryInfo::parse_meminfo(content);

        assert!(result.is_err());
        if let Err(AndroidError::MemoryParseError(msg)) = result {
            assert!(msg.contains("MemTotal"));
        } else {
            panic!("Expected MemoryParseError");
        }
    }

    #[test]
    fn test_process_memory_efficiency() {
        let info = ProcessMemoryInfo {
            pid: 1,
            vm_size_kb: 100000,
            vm_rss_kb: 25000,
            vm_peak_kb: 100000,
            vm_shared_kb: 5000,
            vm_data_kb: 20000,
            vm_stk_kb: 1000,
            vm_exe_kb: 1000,
            vm_swap_kb: 0,
        };

        // 25000 / 100000 = 0.25
        assert!((info.memory_efficiency() - 0.25).abs() < 0.001);
    }
}
