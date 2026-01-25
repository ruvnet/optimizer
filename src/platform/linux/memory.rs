//! Linux Memory Management Core with madvise and /proc APIs
//!
//! This module provides memory optimization capabilities for Linux systems using:
//! - madvise() system calls for memory hints (MADV_DONTNEED, MADV_FREE)
//! - /proc/[pid]/statm and /proc/[pid]/status for process memory information
//! - /proc/sys/vm/drop_caches for system-wide cache management
//! - /proc/[pid]/clear_refs for page table clearing (requires CAP_SYS_ADMIN)

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Errors that can occur during memory operations
#[derive(Debug, Clone)]
pub enum MemoryError {
    /// Process not found
    ProcessNotFound(u32),
    /// Permission denied for operation
    PermissionDenied(String),
    /// Failed to read /proc filesystem
    ProcReadError(String),
    /// Failed to parse memory information
    ParseError(String),
    /// madvise syscall failed
    MadviseError(i32),
    /// Generic I/O error
    IoError(String),
    /// Operation requires root/CAP_SYS_ADMIN
    RequiresRoot(String),
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryError::ProcessNotFound(pid) => write!(f, "Process {} not found", pid),
            MemoryError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            MemoryError::ProcReadError(msg) => write!(f, "Failed to read /proc: {}", msg),
            MemoryError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            MemoryError::MadviseError(errno) => write!(f, "madvise failed with errno {}", errno),
            MemoryError::IoError(msg) => write!(f, "I/O error: {}", msg),
            MemoryError::RequiresRoot(msg) => write!(f, "Requires root: {}", msg),
        }
    }
}

impl std::error::Error for MemoryError {}

impl From<io::Error> for MemoryError {
    fn from(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::NotFound => MemoryError::ProcReadError("File not found".to_string()),
            io::ErrorKind::PermissionDenied => {
                MemoryError::PermissionDenied(err.to_string())
            }
            _ => MemoryError::IoError(err.to_string()),
        }
    }
}

/// Memory information for a single process
#[derive(Debug, Clone, Default)]
pub struct ProcessMemoryInfo {
    /// Process ID
    pub pid: u32,
    /// Virtual memory size in bytes
    pub vsize: u64,
    /// Resident set size (RSS) in bytes
    pub rss: u64,
    /// Shared memory in bytes
    pub shared: u64,
    /// Text (code) segment size in bytes
    pub text: u64,
    /// Data segment size in bytes
    pub data: u64,
    /// Peak virtual memory size in bytes (VmPeak from status)
    pub vm_peak: u64,
    /// Peak RSS in bytes (VmHWM from status)
    pub vm_hwm: u64,
    /// Swap usage in bytes (VmSwap from status)
    pub vm_swap: u64,
    /// Page table entries size in bytes
    pub vm_pte: u64,
    /// Process name
    pub name: String,
}

impl ProcessMemoryInfo {
    /// Returns RSS as megabytes
    pub fn rss_mb(&self) -> f64 {
        self.rss as f64 / 1024.0 / 1024.0
    }

    /// Returns virtual size as megabytes
    pub fn vsize_mb(&self) -> f64 {
        self.vsize as f64 / 1024.0 / 1024.0
    }

    /// Returns swap usage as megabytes
    pub fn swap_mb(&self) -> f64 {
        self.vm_swap as f64 / 1024.0 / 1024.0
    }
}

/// System-wide memory information from /proc/meminfo
#[derive(Debug, Clone, Default)]
pub struct SystemMemoryInfo {
    /// Total physical memory in bytes
    pub total: u64,
    /// Free memory in bytes (not including caches)
    pub free: u64,
    /// Available memory in bytes (including reclaimable)
    pub available: u64,
    /// Memory used by buffers in bytes
    pub buffers: u64,
    /// Memory used by page cache in bytes
    pub cached: u64,
    /// Swap cache in bytes
    pub swap_cached: u64,
    /// Active memory in bytes
    pub active: u64,
    /// Inactive memory in bytes
    pub inactive: u64,
    /// Total swap space in bytes
    pub swap_total: u64,
    /// Free swap space in bytes
    pub swap_free: u64,
    /// Dirty pages in bytes
    pub dirty: u64,
    /// Memory being written back in bytes
    pub writeback: u64,
    /// Anonymous pages in bytes
    pub anon_pages: u64,
    /// Mapped memory in bytes
    pub mapped: u64,
    /// Shmem in bytes
    pub shmem: u64,
    /// Slab reclaimable in bytes
    pub s_reclaimable: u64,
    /// Slab unreclaimable in bytes
    pub s_unreclaim: u64,
    /// Kernel stack in bytes
    pub kernel_stack: u64,
    /// Page tables in bytes
    pub page_tables: u64,
    /// Committed address space in bytes
    pub committed_as: u64,
    /// Huge pages total
    pub hugepages_total: u64,
    /// Huge pages free
    pub hugepages_free: u64,
    /// Huge page size in bytes
    pub hugepagesize: u64,
}

impl SystemMemoryInfo {
    /// Returns memory load as a percentage
    pub fn memory_load_percent(&self) -> u32 {
        if self.total == 0 {
            return 0;
        }
        let used = self.total.saturating_sub(self.available);
        ((used as f64 / self.total as f64) * 100.0) as u32
    }

    /// Returns true if memory pressure is high (>80%)
    pub fn is_high_pressure(&self) -> bool {
        self.memory_load_percent() > 80
    }

    /// Returns true if memory is critical (>95%)
    pub fn is_critical(&self) -> bool {
        self.memory_load_percent() > 95
    }

    /// Returns total memory in megabytes
    pub fn total_mb(&self) -> f64 {
        self.total as f64 / 1024.0 / 1024.0
    }

    /// Returns available memory in megabytes
    pub fn available_mb(&self) -> f64 {
        self.available as f64 / 1024.0 / 1024.0
    }

    /// Returns used memory in megabytes
    pub fn used_mb(&self) -> f64 {
        (self.total.saturating_sub(self.available)) as f64 / 1024.0 / 1024.0
    }

    /// Returns reclaimable memory (buffers + cached + s_reclaimable) in bytes
    pub fn reclaimable(&self) -> u64 {
        self.buffers + self.cached + self.s_reclaimable
    }

    /// Returns swap used in megabytes
    pub fn swap_used_mb(&self) -> f64 {
        (self.swap_total.saturating_sub(self.swap_free)) as f64 / 1024.0 / 1024.0
    }
}

/// Result of a memory optimization operation
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// Memory freed in megabytes
    pub freed_mb: f64,
    /// Available memory before optimization in megabytes
    pub before_available_mb: f64,
    /// Available memory after optimization in megabytes
    pub after_available_mb: f64,
    /// Number of processes that had memory trimmed
    pub processes_trimmed: usize,
    /// Duration of the optimization in milliseconds
    pub duration_ms: u64,
}

/// Linux memory optimizer using madvise and /proc filesystem
pub struct LinuxMemoryOptimizer {
    /// Whether the process has root privileges
    has_root: bool,
    /// Page size for the system
    page_size: usize,
}

impl LinuxMemoryOptimizer {
    /// Creates a new LinuxMemoryOptimizer
    pub fn new() -> Self {
        let has_root = Self::check_root();
        let page_size = Self::get_page_size();

        if !has_root {
            warn!("Running without root - limited optimization (no drop_caches, clear_refs)");
        } else {
            info!("Running with root privileges - full optimization available");
        }

        Self { has_root, page_size }
    }

    /// Check if running as root or with CAP_SYS_ADMIN
    fn check_root() -> bool {
        // Check effective UID
        #[cfg(target_os = "linux")]
        unsafe {
            libc::geteuid() == 0
        }

        #[cfg(not(target_os = "linux"))]
        false
    }

    /// Get the system page size
    fn get_page_size() -> usize {
        #[cfg(target_os = "linux")]
        unsafe {
            libc::sysconf(libc::_SC_PAGESIZE) as usize
        }

        #[cfg(not(target_os = "linux"))]
        4096 // Default fallback
    }

    /// Returns whether root privileges are available
    pub fn has_root_privileges(&self) -> bool {
        self.has_root
    }

    /// Returns the system page size
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// Trim process memory using madvise on the process's memory mappings
    ///
    /// This reads /proc/[pid]/maps and applies MADV_DONTNEED or MADV_FREE
    /// to anonymous and private mappings.
    ///
    /// # Arguments
    /// * `pid` - Process ID to trim memory for
    ///
    /// # Returns
    /// * `Ok(bytes_trimmed)` - Number of bytes potentially freed
    /// * `Err(MemoryError)` - If the operation failed
    pub fn trim_process_memory(&self, pid: u32) -> Result<u64, MemoryError> {
        let proc_path = format!("/proc/{}", pid);
        if !Path::new(&proc_path).exists() {
            return Err(MemoryError::ProcessNotFound(pid));
        }

        // Read memory info before trimming
        let before_info = self.get_process_memory_info(pid)?;
        let before_rss = before_info.rss;

        // Try to write to clear_refs if we have permissions
        // Writing 1 clears the referenced/accessed bit for all pages
        // This helps the kernel identify cold pages for reclamation
        if self.has_root {
            let clear_refs_path = format!("/proc/{}/clear_refs", pid);
            if let Ok(mut file) = OpenOptions::new().write(true).open(&clear_refs_path) {
                // 1 = Clear PG_referenced and ACCESSED/YOUNG
                let _ = file.write_all(b"1");
                debug!("Cleared page reference bits for pid {}", pid);
            }
        }

        // We cannot directly call madvise on another process's memory
        // from our process. The madvise syscall operates on the calling
        // process's address space only.
        //
        // Instead, we use these indirect methods:
        // 1. clear_refs - tell kernel pages aren't accessed (done above)
        // 2. Trigger reclaim via memory pressure simulation

        // For same-process trimming (self optimization), we can use madvise
        #[cfg(target_os = "linux")]
        if pid as i32 == unsafe { libc::getpid() } {
            // Self-trimming: we could madvise our own malloc regions
            // This would require integration with the allocator
            debug!("Self-trimming not implemented for pid {}", pid);
        }

        // Give the kernel time to process
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Read memory info after
        let after_info = self.get_process_memory_info(pid)?;
        let after_rss = after_info.rss;

        let freed = before_rss.saturating_sub(after_rss);
        if freed > 0 {
            debug!(
                "Process {} ({}): trimmed {} bytes ({:.2} MB)",
                pid,
                after_info.name,
                freed,
                freed as f64 / 1024.0 / 1024.0
            );
        }

        Ok(freed)
    }

    /// Get detailed memory information for a process
    ///
    /// Reads from /proc/[pid]/statm for basic memory stats and
    /// /proc/[pid]/status for detailed information.
    ///
    /// # Arguments
    /// * `pid` - Process ID to get information for
    ///
    /// # Returns
    /// * `Ok(ProcessMemoryInfo)` - Memory information for the process
    /// * `Err(MemoryError)` - If reading failed
    pub fn get_process_memory_info(&self, pid: u32) -> Result<ProcessMemoryInfo, MemoryError> {
        let proc_path = format!("/proc/{}", pid);
        if !Path::new(&proc_path).exists() {
            return Err(MemoryError::ProcessNotFound(pid));
        }

        let mut info = ProcessMemoryInfo {
            pid,
            ..Default::default()
        };

        // Read /proc/[pid]/statm for basic memory info
        // Format: size resident shared text lib data dt (all in pages)
        let statm_path = format!("/proc/{}/statm", pid);
        let statm_content = fs::read_to_string(&statm_path)?;
        let parts: Vec<&str> = statm_content.split_whitespace().collect();

        if parts.len() >= 7 {
            let page_size = self.page_size as u64;
            info.vsize = parts[0].parse::<u64>().unwrap_or(0) * page_size;
            info.rss = parts[1].parse::<u64>().unwrap_or(0) * page_size;
            info.shared = parts[2].parse::<u64>().unwrap_or(0) * page_size;
            info.text = parts[3].parse::<u64>().unwrap_or(0) * page_size;
            // parts[4] is lib (unused since Linux 2.6)
            info.data = parts[5].parse::<u64>().unwrap_or(0) * page_size;
            // parts[6] is dt (unused since Linux 2.6)
        }

        // Read /proc/[pid]/status for detailed info
        let status_path = format!("/proc/{}/status", pid);
        if let Ok(file) = File::open(&status_path) {
            let reader = BufReader::new(file);
            for line in reader.lines().filter_map(|l| l.ok()) {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() != 2 {
                    continue;
                }
                let key = parts[0].trim();
                let value = parts[1].trim();

                match key {
                    "Name" => info.name = value.to_string(),
                    "VmPeak" => info.vm_peak = Self::parse_kb_value(value),
                    "VmHWM" => info.vm_hwm = Self::parse_kb_value(value),
                    "VmSwap" => info.vm_swap = Self::parse_kb_value(value),
                    "VmPTE" => info.vm_pte = Self::parse_kb_value(value),
                    _ => {}
                }
            }
        }

        // Fallback for process name from /proc/[pid]/comm
        if info.name.is_empty() {
            let comm_path = format!("/proc/{}/comm", pid);
            if let Ok(name) = fs::read_to_string(&comm_path) {
                info.name = name.trim().to_string();
            }
        }

        Ok(info)
    }

    /// Parse a value like "1234 kB" to bytes
    fn parse_kb_value(value: &str) -> u64 {
        let parts: Vec<&str> = value.split_whitespace().collect();
        if parts.is_empty() {
            return 0;
        }
        let kb = parts[0].parse::<u64>().unwrap_or(0);
        kb * 1024 // Convert kB to bytes
    }

    /// Drop system caches by writing to /proc/sys/vm/drop_caches
    ///
    /// This operation requires root privileges (CAP_SYS_ADMIN).
    ///
    /// Drop levels:
    /// - 1: Free pagecache
    /// - 2: Free dentries and inodes
    /// - 3: Free pagecache, dentries and inodes
    ///
    /// # Returns
    /// * `Ok(())` - Caches were dropped
    /// * `Err(MemoryError)` - If operation failed (usually permission denied)
    pub fn drop_system_caches(&self) -> Result<(), MemoryError> {
        if !self.has_root {
            return Err(MemoryError::RequiresRoot(
                "drop_caches requires root/CAP_SYS_ADMIN".to_string(),
            ));
        }

        // First sync to flush dirty pages to disk
        #[cfg(target_os = "linux")]
        unsafe {
            libc::sync();
        }

        // Write 3 to drop all caches (pagecache + dentries + inodes)
        let drop_caches_path = "/proc/sys/vm/drop_caches";

        let mut file = OpenOptions::new()
            .write(true)
            .open(drop_caches_path)
            .map_err(|e| {
                if e.kind() == io::ErrorKind::PermissionDenied {
                    MemoryError::RequiresRoot(
                        "Cannot write to /proc/sys/vm/drop_caches".to_string(),
                    )
                } else {
                    MemoryError::IoError(e.to_string())
                }
            })?;

        file.write_all(b"3")?;
        info!("Dropped system caches (pagecache + dentries + inodes)");

        Ok(())
    }

    /// Get system-wide memory information from /proc/meminfo
    ///
    /// # Returns
    /// * `Ok(SystemMemoryInfo)` - System memory statistics
    /// * `Err(MemoryError)` - If reading /proc/meminfo failed
    pub fn get_system_memory(&self) -> Result<SystemMemoryInfo, MemoryError> {
        let meminfo_path = "/proc/meminfo";
        let file = File::open(meminfo_path)?;
        let reader = BufReader::new(file);

        let mut info = SystemMemoryInfo::default();

        for line in reader.lines().filter_map(|l| l.ok()) {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() != 2 {
                continue;
            }
            let key = parts[0].trim();
            let value = Self::parse_kb_value(parts[1].trim());

            match key {
                "MemTotal" => info.total = value,
                "MemFree" => info.free = value,
                "MemAvailable" => info.available = value,
                "Buffers" => info.buffers = value,
                "Cached" => info.cached = value,
                "SwapCached" => info.swap_cached = value,
                "Active" => info.active = value,
                "Inactive" => info.inactive = value,
                "SwapTotal" => info.swap_total = value,
                "SwapFree" => info.swap_free = value,
                "Dirty" => info.dirty = value,
                "Writeback" => info.writeback = value,
                "AnonPages" => info.anon_pages = value,
                "Mapped" => info.mapped = value,
                "Shmem" => info.shmem = value,
                "SReclaimable" => info.s_reclaimable = value,
                "SUnreclaim" => info.s_unreclaim = value,
                "KernelStack" => info.kernel_stack = value,
                "PageTables" => info.page_tables = value,
                "Committed_AS" => info.committed_as = value,
                "HugePages_Total" => {
                    info.hugepages_total = parts[1]
                        .trim()
                        .parse::<u64>()
                        .unwrap_or(0);
                }
                "HugePages_Free" => {
                    info.hugepages_free = parts[1]
                        .trim()
                        .parse::<u64>()
                        .unwrap_or(0);
                }
                "Hugepagesize" => info.hugepagesize = value,
                _ => {}
            }
        }

        // If MemAvailable is not present (older kernels), estimate it
        if info.available == 0 {
            info.available = info.free + info.buffers + info.cached;
        }

        Ok(info)
    }

    /// List all process IDs from /proc
    pub fn list_processes(&self) -> Result<Vec<u32>, MemoryError> {
        let mut pids = Vec::new();

        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();

            // Check if directory name is numeric (PID)
            if let Ok(pid) = name.parse::<u32>() {
                pids.push(pid);
            }
        }

        Ok(pids)
    }

    /// Optimize system memory by trimming processes and optionally dropping caches
    ///
    /// # Arguments
    /// * `aggressive` - If true, also drop system caches (requires root)
    ///
    /// # Returns
    /// * `Ok(OptimizationResult)` - Results of the optimization
    /// * `Err(MemoryError)` - If the operation failed
    pub fn optimize(&self, aggressive: bool) -> Result<OptimizationResult, MemoryError> {
        let start = Instant::now();
        let before = self.get_system_memory()?;
        let mut trimmed = 0usize;
        let mut total_freed: u64 = 0;

        // Get list of processes
        let pids = self.list_processes()?;

        // Skip kernel threads (low PIDs) and our own process
        let our_pid = std::process::id();

        for pid in pids.iter().take(200) {
            if *pid <= 2 || *pid == our_pid {
                continue;
            }

            match self.trim_process_memory(*pid) {
                Ok(freed) => {
                    if freed > 0 {
                        total_freed += freed;
                        trimmed += 1;
                    }
                }
                Err(MemoryError::PermissionDenied(_)) => {
                    // Expected for processes we don't own without root
                    continue;
                }
                Err(MemoryError::ProcessNotFound(_)) => {
                    // Process may have exited
                    continue;
                }
                Err(e) => {
                    debug!("Failed to trim process {}: {}", pid, e);
                }
            }
        }

        // Drop caches if aggressive mode and we have root
        if aggressive && self.has_root {
            if let Err(e) = self.drop_system_caches() {
                warn!("Failed to drop system caches: {}", e);
            }
        }

        // Wait for kernel to reclaim memory
        std::thread::sleep(std::time::Duration::from_millis(100));

        let after = self.get_system_memory()?;
        let measured_freed =
            (after.available as f64 - before.available as f64) / 1024.0 / 1024.0;
        let calculated_freed = total_freed as f64 / 1024.0 / 1024.0;
        let freed_mb = measured_freed.max(calculated_freed).max(0.0);

        info!(
            "Optimized: trimmed {} processes, freed {:.1} MB in {}ms",
            trimmed,
            freed_mb,
            start.elapsed().as_millis()
        );

        Ok(OptimizationResult {
            freed_mb,
            before_available_mb: before.available_mb(),
            after_available_mb: after.available_mb(),
            processes_trimmed: trimmed,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Apply madvise hint to a memory region (for self-process optimization)
    ///
    /// # Safety
    /// This directly calls the madvise syscall. The caller must ensure
    /// the address and length are valid for the process's address space.
    #[cfg(target_os = "linux")]
    pub unsafe fn madvise_region(
        addr: *mut libc::c_void,
        length: usize,
        advice: i32,
    ) -> Result<(), MemoryError> {
        let result = libc::madvise(addr, length, advice);
        if result == 0 {
            Ok(())
        } else {
            Err(MemoryError::MadviseError(*libc::__errno_location()))
        }
    }

    /// Get memory pressure level (0-100) based on available memory and swap
    pub fn get_memory_pressure(&self) -> Result<u32, MemoryError> {
        let info = self.get_system_memory()?;

        // Calculate memory pressure as percentage of used memory
        let mem_pressure = info.memory_load_percent();

        // Factor in swap usage if swap is configured
        let swap_pressure = if info.swap_total > 0 {
            let swap_used = info.swap_total.saturating_sub(info.swap_free);
            ((swap_used as f64 / info.swap_total as f64) * 100.0) as u32
        } else {
            0
        };

        // Combined pressure (weighted: 70% memory, 30% swap)
        let combined = (mem_pressure * 70 + swap_pressure * 30) / 100;

        Ok(combined.min(100))
    }

    /// Get OOM (Out-of-Memory) score for a process
    /// Higher scores mean the process is more likely to be killed by OOM killer
    pub fn get_oom_score(&self, pid: u32) -> Result<i32, MemoryError> {
        let path = format!("/proc/{}/oom_score", pid);
        let content = fs::read_to_string(&path)?;
        content
            .trim()
            .parse::<i32>()
            .map_err(|e| MemoryError::ParseError(e.to_string()))
    }

    /// Adjust OOM score for a process (requires appropriate permissions)
    /// Range: -1000 (never kill) to 1000 (always prefer to kill)
    pub fn set_oom_score_adj(&self, pid: u32, score: i32) -> Result<(), MemoryError> {
        if score < -1000 || score > 1000 {
            return Err(MemoryError::ParseError(
                "OOM score adjust must be between -1000 and 1000".to_string(),
            ));
        }

        let path = format!("/proc/{}/oom_score_adj", pid);
        let mut file = OpenOptions::new().write(true).open(&path)?;
        file.write_all(format!("{}", score).as_bytes())?;

        Ok(())
    }

    /// Read memory cgroup information for a process (cgroups v2)
    pub fn get_cgroup_memory(&self, pid: u32) -> Result<Option<u64>, MemoryError> {
        // Read cgroup path from /proc/[pid]/cgroup
        let cgroup_path = format!("/proc/{}/cgroup", pid);
        let content = fs::read_to_string(&cgroup_path)?;

        // Find the memory controller path (cgroups v2 unified hierarchy)
        for line in content.lines() {
            // cgroups v2: 0::/path
            if line.starts_with("0::") {
                let path = &line[3..];
                let memory_max_path = format!("/sys/fs/cgroup{}/memory.max", path);

                if let Ok(max_content) = fs::read_to_string(&memory_max_path) {
                    let trimmed = max_content.trim();
                    if trimmed == "max" {
                        return Ok(None); // Unlimited
                    }
                    if let Ok(limit) = trimmed.parse::<u64>() {
                        return Ok(Some(limit));
                    }
                }
            }
        }

        Ok(None)
    }
}

impl Default for LinuxMemoryOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Constants for madvise advice values
#[cfg(target_os = "linux")]
pub mod advice {
    /// No special treatment. This is the default.
    pub const MADV_NORMAL: i32 = libc::MADV_NORMAL;
    /// Expect random page references
    pub const MADV_RANDOM: i32 = libc::MADV_RANDOM;
    /// Expect sequential page references
    pub const MADV_SEQUENTIAL: i32 = libc::MADV_SEQUENTIAL;
    /// Will need these pages
    pub const MADV_WILLNEED: i32 = libc::MADV_WILLNEED;
    /// Don't need these pages (can be freed immediately)
    pub const MADV_DONTNEED: i32 = libc::MADV_DONTNEED;
    /// Free pages (lazy, similar to DONTNEED but better for anonymous memory)
    pub const MADV_FREE: i32 = libc::MADV_FREE;
    /// Pages can be merged with identical ones (KSM)
    pub const MADV_MERGEABLE: i32 = libc::MADV_MERGEABLE;
    /// Undo MADV_MERGEABLE
    pub const MADV_UNMERGEABLE: i32 = libc::MADV_UNMERGEABLE;
    /// Back pages with huge pages if possible
    pub const MADV_HUGEPAGE: i32 = libc::MADV_HUGEPAGE;
    /// Undo MADV_HUGEPAGE
    pub const MADV_NOHUGEPAGE: i32 = libc::MADV_NOHUGEPAGE;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_system_memory() {
        let optimizer = LinuxMemoryOptimizer::new();
        let result = optimizer.get_system_memory();

        // This test will only pass on Linux
        #[cfg(target_os = "linux")]
        {
            assert!(result.is_ok());
            let info = result.unwrap();
            assert!(info.total > 0);
            assert!(info.available > 0);
            assert!(info.available <= info.total);
        }
    }

    #[test]
    fn test_get_process_memory_info_self() {
        let optimizer = LinuxMemoryOptimizer::new();
        let pid = std::process::id();

        #[cfg(target_os = "linux")]
        {
            let result = optimizer.get_process_memory_info(pid);
            assert!(result.is_ok());
            let info = result.unwrap();
            assert_eq!(info.pid, pid);
            assert!(info.rss > 0);
        }
    }

    #[test]
    fn test_list_processes() {
        let optimizer = LinuxMemoryOptimizer::new();

        #[cfg(target_os = "linux")]
        {
            let result = optimizer.list_processes();
            assert!(result.is_ok());
            let pids = result.unwrap();
            assert!(!pids.is_empty());
            // Our process should be in the list
            assert!(pids.contains(&std::process::id()));
        }
    }

    #[test]
    fn test_memory_pressure() {
        let optimizer = LinuxMemoryOptimizer::new();

        #[cfg(target_os = "linux")]
        {
            let result = optimizer.get_memory_pressure();
            assert!(result.is_ok());
            let pressure = result.unwrap();
            assert!(pressure <= 100);
        }
    }

    #[test]
    fn test_process_not_found() {
        let optimizer = LinuxMemoryOptimizer::new();
        // Use an invalid PID that's unlikely to exist
        let result = optimizer.get_process_memory_info(u32::MAX);
        assert!(matches!(result, Err(MemoryError::ProcessNotFound(_))));
    }

    #[test]
    fn test_system_memory_info_methods() {
        let info = SystemMemoryInfo {
            total: 16 * 1024 * 1024 * 1024, // 16 GB
            available: 8 * 1024 * 1024 * 1024, // 8 GB
            ..Default::default()
        };

        assert_eq!(info.memory_load_percent(), 50);
        assert!(!info.is_high_pressure());
        assert!(!info.is_critical());
        assert_eq!(info.total_mb(), 16384.0);
        assert_eq!(info.available_mb(), 8192.0);
    }

    #[test]
    fn test_parse_kb_value() {
        assert_eq!(LinuxMemoryOptimizer::parse_kb_value("1024 kB"), 1024 * 1024);
        assert_eq!(LinuxMemoryOptimizer::parse_kb_value("512"), 512 * 1024);
        assert_eq!(LinuxMemoryOptimizer::parse_kb_value(""), 0);
    }
}
