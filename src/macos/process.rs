//! macOS process enumeration
//!
//! Provides cross-platform process enumeration using the `sysinfo` crate.
//! This module mirrors the Windows implementation but targets macOS/Darwin systems.

use sysinfo::{Pid, System};

/// List all process IDs currently running on the system.
///
/// Uses the `sysinfo` crate for cross-platform compatibility.
/// On macOS, this queries the kernel for process information.
///
/// # Returns
/// - `Ok(Vec<u32>)` - Vector of all process IDs
/// - `Err(String)` - Error message if enumeration fails
///
/// # Example
/// ```no_run
/// use optimizer::macos::process::list_processes;
///
/// let pids = list_processes().expect("Failed to list processes");
/// println!("Found {} processes", pids.len());
/// ```
pub fn list_processes() -> Result<Vec<u32>, String> {
    let mut sys = System::new_all();
    sys.refresh_processes();

    let pids: Vec<u32> = sys.processes().keys().map(|pid| pid.as_u32()).collect();

    Ok(pids)
}

/// Get the name of a process by its PID.
///
/// # Arguments
/// * `pid` - The process ID to look up
///
/// # Returns
/// - `Some(String)` - The process name if found
/// - `None` - If the process doesn't exist or cannot be accessed
///
/// # Example
/// ```no_run
/// use optimizer::macos::process::get_process_name;
///
/// if let Some(name) = get_process_name(1) {
///     println!("PID 1 is: {}", name); // Usually "launchd" on macOS
/// }
/// ```
pub fn get_process_name(pid: u32) -> Option<String> {
    let mut sys = System::new();
    sys.refresh_processes();

    sys.process(Pid::from_u32(pid))
        .map(|p| p.name().to_string())
}

/// Get memory usage for a process in bytes.
///
/// Returns the resident set size (RSS) - the portion of memory
/// held in RAM for the process.
///
/// # Arguments
/// * `pid` - The process ID to query
///
/// # Returns
/// - `Some(u64)` - Memory usage in bytes
/// - `None` - If the process doesn't exist or cannot be accessed
///
/// # Example
/// ```no_run
/// use optimizer::macos::process::get_process_memory;
///
/// if let Some(mem) = get_process_memory(1234) {
///     println!("Process memory: {} MB", mem / 1024 / 1024);
/// }
/// ```
pub fn get_process_memory(pid: u32) -> Option<u64> {
    let mut sys = System::new();
    sys.refresh_processes();

    sys.process(Pid::from_u32(pid)).map(|p| p.memory())
}

/// Get CPU usage percentage for a process.
///
/// Returns the CPU usage as a percentage (0.0 to 100.0 per core).
///
/// # Arguments
/// * `pid` - The process ID to query
///
/// # Returns
/// - `Some(f32)` - CPU usage percentage
/// - `None` - If the process doesn't exist or cannot be accessed
pub fn get_process_cpu_usage(pid: u32) -> Option<f32> {
    let mut sys = System::new();
    sys.refresh_processes();

    sys.process(Pid::from_u32(pid)).map(|p| p.cpu_usage())
}

/// Get the parent process ID for a given process.
///
/// # Arguments
/// * `pid` - The process ID to query
///
/// # Returns
/// - `Some(u32)` - Parent process ID
/// - `None` - If the process doesn't exist or has no parent
pub fn get_parent_pid(pid: u32) -> Option<u32> {
    let mut sys = System::new();
    sys.refresh_processes();

    sys.process(Pid::from_u32(pid))
        .and_then(|p| p.parent())
        .map(|ppid| ppid.as_u32())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_processes() {
        let result = list_processes();
        assert!(result.is_ok());
        let pids = result.unwrap();
        // There should always be at least one process (ourselves)
        assert!(!pids.is_empty());
    }

    #[test]
    fn test_get_process_name_self() {
        // Get our own process ID
        let our_pid = std::process::id();
        let name = get_process_name(our_pid);
        // We should be able to get our own name
        assert!(name.is_some());
    }

    #[test]
    fn test_get_process_memory_self() {
        let our_pid = std::process::id();
        let memory = get_process_memory(our_pid);
        assert!(memory.is_some());
        // We should be using some memory
        assert!(memory.unwrap() > 0);
    }

    #[test]
    fn test_nonexistent_process() {
        // PID 0 is typically not a valid user process
        // Use a very high PID that's unlikely to exist
        let name = get_process_name(u32::MAX);
        assert!(name.is_none());
    }
}
