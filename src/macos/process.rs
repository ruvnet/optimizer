//! macOS process enumeration and management

use sysinfo::{System, Pid, ProcessesToUpdate, UpdateKind};
use std::collections::HashSet;

/// Process info tuple: (pid, name, memory_mb)
pub type ProcessInfo = (u32, String, f64);

/// List all process IDs
pub fn list_processes() -> Result<Vec<u32>, String> {
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let pids: Vec<u32> = sys
        .processes()
        .keys()
        .map(|pid| pid.as_u32())
        .collect();

    Ok(pids)
}

/// List user processes with memory usage (sorted by memory descending)
pub fn list_user_processes() -> Result<Vec<ProcessInfo>, String> {
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let current_uid = unsafe { libc::getuid() };

    let mut processes: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            // Filter to user's processes (or all if root)
            let proc_uid = process.user_id().map(|u| **u).unwrap_or(0);
            if current_uid == 0 || proc_uid == current_uid {
                let memory_mb = process.memory() as f64 / 1024.0 / 1024.0;
                Some((pid.as_u32(), process.name().to_string_lossy().to_string(), memory_mb))
            } else {
                None
            }
        })
        .collect();

    // Sort by memory usage (highest first)
    processes.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    Ok(processes)
}

/// Get process name by PID
pub fn get_process_name(pid: u32) -> Option<String> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    sys.process(Pid::from_u32(pid))
        .map(|p| p.name().to_string_lossy().to_string())
}

/// Get memory usage for a process (bytes)
pub fn get_process_memory(pid: u32) -> Option<u64> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    sys.process(Pid::from_u32(pid)).map(|p| p.memory())
}

/// Get process parent PID
pub fn get_parent_pid(pid: u32) -> Option<u32> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    sys.process(Pid::from_u32(pid))
        .and_then(|p| p.parent())
        .map(|p| p.as_u32())
}

/// Check if process is a system process
pub fn is_system_process(pid: u32) -> bool {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    if let Some(process) = sys.process(Pid::from_u32(pid)) {
        // System processes typically run as root (uid 0)
        if let Some(uid) = process.user_id() {
            if **uid == 0 {
                return true;
            }
        }

        // Check against known system processes
        let name = process.name().to_string_lossy().to_lowercase();
        super::safety::PROTECTED_PROCESSES
            .iter()
            .any(|p| name.contains(&p.to_lowercase()))
    } else {
        false
    }
}

/// Get top memory consumers
pub fn get_top_memory_processes(count: usize) -> Vec<ProcessInfo> {
    list_user_processes()
        .unwrap_or_default()
        .into_iter()
        .take(count)
        .collect()
}

/// Get processes by name pattern
pub fn find_processes_by_name(pattern: &str) -> Vec<ProcessInfo> {
    let pattern_lower = pattern.to_lowercase();

    list_user_processes()
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, name, _)| name.to_lowercase().contains(&pattern_lower))
        .collect()
}

/// Get total memory used by processes matching pattern
pub fn get_memory_by_pattern(pattern: &str) -> f64 {
    find_processes_by_name(pattern)
        .iter()
        .map(|(_, _, mem)| mem)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_processes() {
        let pids = list_processes().unwrap();
        assert!(!pids.is_empty());
    }

    #[test]
    fn test_list_user_processes() {
        let procs = list_user_processes().unwrap();
        // Should have at least our own process
        assert!(!procs.is_empty());
    }
}
