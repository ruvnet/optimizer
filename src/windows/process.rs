//! Windows process enumeration

use sysinfo::{System, Pid, ProcessesToUpdate};

/// List all process IDs
pub fn list_processes() -> Result<Vec<u32>, String> {
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let pids: Vec<u32> = sys.processes()
        .keys()
        .map(|pid| pid.as_u32())
        .collect();

    Ok(pids)
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

    sys.process(Pid::from_u32(pid))
        .map(|p| p.memory())
}
