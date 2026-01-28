//! Android Process Enumeration via /proc filesystem
//!
//! Enumerates processes by scanning /proc/[0-9]+ directories.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::memory::MemoryInfo;
// Import error types from parent module
pub use super::{AndroidError, AndroidResult};

/// Process information from /proc/[pid]/stat and /proc/[pid]/status
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
    /// Parent process ID
    pub ppid: u32,
    /// Process state (R=running, S=sleeping, D=disk sleep, Z=zombie, T=stopped)
    pub state: char,
    /// User ID owning the process
    pub uid: u32,
    /// Number of threads
    pub threads: u32,
    /// RSS memory in KB
    pub rss_kb: u64,
    /// Virtual memory size in KB
    pub virt_kb: u64,
    /// CPU time in jiffies (user + system)
    pub cpu_time: u64,
    /// Start time in jiffies since boot
    pub start_time: u64,
}

impl ProcessInfo {
    /// Get RSS in MB
    pub fn rss_mb(&self) -> f64 {
        self.rss_kb as f64 / 1024.0
    }

    /// Get virtual memory in MB
    pub fn virt_mb(&self) -> f64 {
        self.virt_kb as f64 / 1024.0
    }

    /// Check if this is a kernel thread
    pub fn is_kernel_thread(&self) -> bool {
        self.ppid == 2 || self.pid == 2
    }

    /// Check if this process is running
    pub fn is_running(&self) -> bool {
        self.state == 'R'
    }

    /// Check if this process is a zombie
    pub fn is_zombie(&self) -> bool {
        self.state == 'Z'
    }
}

/// List all process IDs from /proc
pub fn list_processes() -> AndroidResult<Vec<u32>> {
    let mut pids = Vec::new();

    let entries = fs::read_dir("/proc").map_err(|e| {
        AndroidError::ProcReadError(format!("Failed to read /proc: {}", e))
    })?;

    for entry in entries {
        if let Ok(entry) = entry {
            if let Some(name) = entry.file_name().to_str() {
                // Check if the directory name is a number (PID)
                if let Ok(pid) = name.parse::<u32>() {
                    pids.push(pid);
                }
            }
        }
    }

    pids.sort_unstable();
    Ok(pids)
}

/// List processes with their RSS memory usage
pub fn list_processes_with_memory() -> AndroidResult<HashMap<u32, u64>> {
    let mut result = HashMap::new();
    let pids = list_processes()?;

    for pid in pids {
        // Try fast path first (statm)
        match MemoryInfo::read_process_memory_fast(pid) {
            Ok((_, rss_kb)) => {
                result.insert(pid, rss_kb);
            }
            Err(_) => {
                // Fall back to status if statm fails
                if let Ok(mem_info) = MemoryInfo::read_process_memory(pid) {
                    result.insert(pid, mem_info.vm_rss_kb);
                }
                // Skip if we can't read either (permission denied, process exited, etc.)
            }
        }
    }

    Ok(result)
}

/// Get detailed process information
pub fn get_process_info(pid: u32) -> AndroidResult<ProcessInfo> {
    let proc_path = format!("/proc/{}", pid);
    let path = Path::new(&proc_path);

    if !path.exists() {
        return Err(AndroidError::ProcessNotFound(pid));
    }

    // Read /proc/[pid]/stat for basic info
    let stat_content = fs::read_to_string(format!("{}/stat", proc_path)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            AndroidError::PermissionDenied(format!(
                "Cannot read /proc/{}/stat: permission denied",
                pid
            ))
        } else {
            AndroidError::ProcReadError(format!("Failed to read /proc/{}/stat: {}", pid, e))
        }
    })?;

    // Read /proc/[pid]/status for additional info
    let status_content = fs::read_to_string(format!("{}/status", proc_path)).map_err(|e| {
        AndroidError::ProcReadError(format!("Failed to read /proc/{}/status: {}", pid, e))
    })?;

    parse_process_info(pid, &stat_content, &status_content)
}

/// Parse /proc/[pid]/stat and /proc/[pid]/status
fn parse_process_info(pid: u32, stat: &str, status: &str) -> AndroidResult<ProcessInfo> {
    // Parse stat file
    // Format: pid (comm) state ppid pgrp session tty_nr tpgid flags minflt cminflt majflt cmajflt
    //         utime stime cutime cstime priority nice num_threads itrealvalue starttime vsize rss ...

    // Handle process names with spaces/parentheses - find the last ')'
    let comm_start = stat.find('(').ok_or_else(|| {
        AndroidError::MemoryParseError(format!("Invalid stat format for pid {}: missing '('", pid))
    })?;
    let comm_end = stat.rfind(')').ok_or_else(|| {
        AndroidError::MemoryParseError(format!("Invalid stat format for pid {}: missing ')'", pid))
    })?;

    let name = stat[comm_start + 1..comm_end].to_string();
    let rest = &stat[comm_end + 2..]; // Skip ") "
    let fields: Vec<&str> = rest.split_whitespace().collect();

    if fields.len() < 20 {
        return Err(AndroidError::MemoryParseError(format!(
            "Invalid stat format for pid {}: not enough fields",
            pid
        )));
    }

    let state = fields[0].chars().next().unwrap_or('?');
    let ppid = fields[1].parse::<u32>().unwrap_or(0);
    let utime = fields[11].parse::<u64>().unwrap_or(0);
    let stime = fields[12].parse::<u64>().unwrap_or(0);
    let num_threads = fields[17].parse::<u32>().unwrap_or(1);
    let start_time = fields[19].parse::<u64>().unwrap_or(0);
    let vsize = fields[20].parse::<u64>().unwrap_or(0) / 1024; // bytes to KB
    let rss_pages = fields[21].parse::<u64>().unwrap_or(0);

    // Convert RSS from pages to KB
    let page_size = get_page_size();
    let rss_kb = rss_pages * page_size / 1024;

    // Parse status file for UID
    let mut uid = 0u32;
    for line in status.lines() {
        if line.starts_with("Uid:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                uid = parts[1].parse().unwrap_or(0);
            }
            break;
        }
    }

    Ok(ProcessInfo {
        pid,
        name,
        ppid,
        state,
        uid,
        threads: num_threads,
        rss_kb,
        virt_kb: vsize,
        cpu_time: utime + stime,
        start_time,
    })
}

/// Get process name by PID
pub fn get_process_name(pid: u32) -> AndroidResult<String> {
    let comm_path = format!("/proc/{}/comm", pid);
    let path = Path::new(&comm_path);

    if !path.exists() {
        return Err(AndroidError::ProcessNotFound(pid));
    }

    let name = fs::read_to_string(path)
        .map_err(|e| AndroidError::ProcReadError(format!("Failed to read comm: {}", e)))?
        .trim()
        .to_string();

    Ok(name)
}

/// Get process command line
pub fn get_process_cmdline(pid: u32) -> AndroidResult<Vec<String>> {
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    let path = Path::new(&cmdline_path);

    if !path.exists() {
        return Err(AndroidError::ProcessNotFound(pid));
    }

    let content = fs::read_to_string(path)
        .map_err(|e| AndroidError::ProcReadError(format!("Failed to read cmdline: {}", e)))?;

    // cmdline is NUL-separated
    let args: Vec<String> = content
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    Ok(args)
}

/// Get process memory usage in bytes
pub fn get_process_memory(pid: u32) -> AndroidResult<u64> {
    match MemoryInfo::read_process_memory_fast(pid) {
        Ok((_, rss_kb)) => Ok(rss_kb * 1024), // Convert to bytes
        Err(_) => {
            // Fall back to status
            let mem_info = MemoryInfo::read_process_memory(pid)?;
            Ok(mem_info.vm_rss_kb * 1024)
        }
    }
}

/// List child processes of a given PID
pub fn list_child_processes(parent_pid: u32) -> AndroidResult<Vec<u32>> {
    let children_path = format!("/proc/{}/task/{}/children", parent_pid, parent_pid);
    let path = Path::new(&children_path);

    // Try the children file first (may not exist on older kernels)
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            let children: Vec<u32> = content
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            return Ok(children);
        }
    }

    // Fall back to scanning all processes
    let pids = list_processes()?;
    let mut children = Vec::new();

    for pid in pids {
        if let Ok(info) = get_process_info(pid) {
            if info.ppid == parent_pid {
                children.push(pid);
            }
        }
    }

    Ok(children)
}

/// Get all threads of a process
pub fn get_process_threads(pid: u32) -> AndroidResult<Vec<u32>> {
    let task_path = format!("/proc/{}/task", pid);
    let path = Path::new(&task_path);

    if !path.exists() {
        return Err(AndroidError::ProcessNotFound(pid));
    }

    let mut threads = Vec::new();

    let entries = fs::read_dir(path).map_err(|e| {
        AndroidError::ProcReadError(format!("Failed to read /proc/{}/task: {}", pid, e))
    })?;

    for entry in entries {
        if let Ok(entry) = entry {
            if let Some(name) = entry.file_name().to_str() {
                if let Ok(tid) = name.parse::<u32>() {
                    threads.push(tid);
                }
            }
        }
    }

    threads.sort_unstable();
    Ok(threads)
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

/// Filter processes by name pattern
pub fn find_processes_by_name(pattern: &str) -> AndroidResult<Vec<ProcessInfo>> {
    let pids = list_processes()?;
    let pattern_lower = pattern.to_lowercase();
    let mut matches = Vec::new();

    for pid in pids {
        if let Ok(info) = get_process_info(pid) {
            if info.name.to_lowercase().contains(&pattern_lower) {
                matches.push(info);
            }
        }
    }

    Ok(matches)
}

/// Get top N processes by memory usage
pub fn get_top_memory_processes(n: usize) -> AndroidResult<Vec<ProcessInfo>> {
    let pids = list_processes()?;
    let mut processes: Vec<ProcessInfo> = Vec::new();

    for pid in pids {
        if let Ok(info) = get_process_info(pid) {
            processes.push(info);
        }
    }

    // Sort by RSS descending
    processes.sort_by(|a, b| b.rss_kb.cmp(&a.rss_kb));
    processes.truncate(n);

    Ok(processes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_STAT: &str = "1234 (test process) S 1 1234 1234 0 -1 4194304 1234 0 0 0 100 50 0 0 20 0 5 0 12345 102400000 12345 18446744073709551615 1 1 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0";

    const SAMPLE_STATUS: &str = r#"Name:   test process
State:  S (sleeping)
Tgid:   1234
Pid:    1234
PPid:   1
Uid:    10001   10001   10001   10001
Gid:    10001   10001   10001   10001
FDSize: 256
Threads:    5
VmPeak:   102400 kB
VmSize:    98304 kB
VmRSS:     48640 kB
"#;

    #[test]
    fn test_parse_process_info() {
        let result = parse_process_info(1234, SAMPLE_STAT, SAMPLE_STATUS).unwrap();

        assert_eq!(result.pid, 1234);
        assert_eq!(result.name, "test process");
        assert_eq!(result.ppid, 1);
        assert_eq!(result.state, 'S');
        assert_eq!(result.uid, 10001);
        assert_eq!(result.threads, 5);
        assert_eq!(result.cpu_time, 150); // 100 + 50
        assert_eq!(result.start_time, 12345);

        // Virtual size: 102400000 bytes / 1024 = 100000 KB
        assert_eq!(result.virt_kb, 100000);
    }

    #[test]
    fn test_process_info_methods() {
        let info = ProcessInfo {
            pid: 1,
            name: "test".to_string(),
            ppid: 1,
            state: 'R',
            uid: 1000,
            threads: 1,
            rss_kb: 51200,
            virt_kb: 102400,
            cpu_time: 100,
            start_time: 1000,
        };

        assert!((info.rss_mb() - 50.0).abs() < 0.01);
        assert!((info.virt_mb() - 100.0).abs() < 0.01);
        assert!(info.is_running());
        assert!(!info.is_zombie());
        assert!(!info.is_kernel_thread());
    }

    #[test]
    fn test_kernel_thread_detection() {
        let kernel_thread = ProcessInfo {
            pid: 123,
            name: "kworker".to_string(),
            ppid: 2,
            state: 'S',
            uid: 0,
            threads: 1,
            rss_kb: 0,
            virt_kb: 0,
            cpu_time: 0,
            start_time: 0,
        };

        assert!(kernel_thread.is_kernel_thread());

        let kthreadd = ProcessInfo {
            pid: 2,
            name: "kthreadd".to_string(),
            ppid: 0,
            state: 'S',
            uid: 0,
            threads: 1,
            rss_kb: 0,
            virt_kb: 0,
            cpu_time: 0,
            start_time: 0,
        };

        assert!(kthreadd.is_kernel_thread());
    }

    #[test]
    fn test_zombie_detection() {
        let zombie = ProcessInfo {
            pid: 1,
            name: "zombie".to_string(),
            ppid: 1,
            state: 'Z',
            uid: 1000,
            threads: 1,
            rss_kb: 0,
            virt_kb: 0,
            cpu_time: 0,
            start_time: 0,
        };

        assert!(zombie.is_zombie());
        assert!(!zombie.is_running());
    }

    #[test]
    fn test_parse_stat_with_parentheses_in_name() {
        // Process name with parentheses: "(sd-pam)"
        let stat = "1234 ((sd-pam)) S 1 1234 1234 0 -1 4194304 0 0 0 0 0 0 0 0 20 0 1 0 100 4096000 100 18446744073709551615 1 1 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0";
        let status = "Name: (sd-pam)\nUid: 1000 1000 1000 1000\n";

        let result = parse_process_info(1234, stat, status).unwrap();
        assert_eq!(result.name, "(sd-pam)");
    }

    #[test]
    fn test_parse_stat_with_spaces_in_name() {
        // Process name with spaces: "Web Content"
        let stat = "5678 (Web Content) S 1234 5678 5678 0 -1 4194304 0 0 0 0 50 25 0 0 20 0 10 0 200 8192000 500 18446744073709551615 1 1 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0";
        let status = "Name: Web Content\nUid: 1000 1000 1000 1000\n";

        let result = parse_process_info(5678, stat, status).unwrap();
        assert_eq!(result.name, "Web Content");
        assert_eq!(result.ppid, 1234);
        assert_eq!(result.threads, 10);
    }
}
