//! Linux process management module
//!
//! Provides Linux-specific process enumeration, information retrieval,
//! and management using sysinfo crate and /proc filesystem parsing.

use sysinfo::{System, Pid, ProcessStatus};
use std::collections::HashSet;
use std::fs;
use std::io;

/// Process state representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    /// Running or runnable
    Running,
    /// Sleeping in an interruptible wait
    Sleeping,
    /// Waiting in uninterruptible disk sleep
    DiskSleep,
    /// Zombie (terminated but not reaped)
    Zombie,
    /// Stopped (e.g., by a signal)
    Stopped,
    /// Tracing stop
    TracingStop,
    /// Dead
    Dead,
    /// Idle (kernel thread)
    Idle,
    /// Unknown state
    Unknown,
}

impl ProcessState {
    /// Parse state from /proc/[pid]/stat state character
    pub fn from_stat_char(c: char) -> Self {
        match c {
            'R' => ProcessState::Running,
            'S' => ProcessState::Sleeping,
            'D' => ProcessState::DiskSleep,
            'Z' => ProcessState::Zombie,
            'T' => ProcessState::Stopped,
            't' => ProcessState::TracingStop,
            'X' | 'x' => ProcessState::Dead,
            'I' => ProcessState::Idle,
            _ => ProcessState::Unknown,
        }
    }

    /// Convert from sysinfo ProcessStatus
    pub fn from_sysinfo(status: ProcessStatus) -> Self {
        match status {
            ProcessStatus::Run => ProcessState::Running,
            ProcessStatus::Sleep => ProcessState::Sleeping,
            ProcessStatus::Idle => ProcessState::Idle,
            ProcessStatus::Zombie => ProcessState::Zombie,
            ProcessStatus::Stop => ProcessState::Stopped,
            ProcessStatus::Tracing => ProcessState::TracingStop,
            ProcessStatus::Dead => ProcessState::Dead,
            ProcessStatus::UninterruptibleDiskSleep => ProcessState::DiskSleep,
            _ => ProcessState::Unknown,
        }
    }

    /// Get human-readable description
    pub fn as_str(&self) -> &'static str {
        match self {
            ProcessState::Running => "Running",
            ProcessState::Sleeping => "Sleeping",
            ProcessState::DiskSleep => "Disk Sleep",
            ProcessState::Zombie => "Zombie",
            ProcessState::Stopped => "Stopped",
            ProcessState::TracingStop => "Tracing",
            ProcessState::Dead => "Dead",
            ProcessState::Idle => "Idle",
            ProcessState::Unknown => "Unknown",
        }
    }
}

/// Information about a process
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Process name (from /proc/[pid]/comm)
    pub name: String,
    /// Full command line (from /proc/[pid]/cmdline)
    pub cmdline: String,
    /// Resident Set Size in bytes
    pub memory_rss: u64,
    /// Virtual Memory Size in bytes
    pub memory_vms: u64,
    /// CPU usage percentage (0.0-100.0)
    pub cpu_usage: f32,
    /// Process state
    pub state: ProcessState,
    /// Parent process ID
    pub ppid: Option<u32>,
    /// User ID of process owner
    pub uid: Option<u32>,
    /// Number of threads
    pub threads: Option<u32>,
}

impl ProcessInfo {
    /// Check if this process is a zombie
    pub fn is_zombie(&self) -> bool {
        self.state == ProcessState::Zombie
    }

    /// Check if this process is running or runnable
    pub fn is_running(&self) -> bool {
        self.state == ProcessState::Running
    }

    /// Get memory usage in MB
    pub fn memory_mb(&self) -> f64 {
        self.memory_rss as f64 / (1024.0 * 1024.0)
    }
}

/// Error type for process operations
#[derive(Debug)]
pub enum ProcessError {
    /// Process not found
    NotFound(u32),
    /// Permission denied
    PermissionDenied(String),
    /// I/O error
    IoError(io::Error),
    /// Signal sending failed
    SignalError(String),
    /// Process is protected
    Protected(String),
    /// Invalid signal number
    InvalidSignal(i32),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::NotFound(pid) => write!(f, "Process {} not found", pid),
            ProcessError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            ProcessError::IoError(e) => write!(f, "I/O error: {}", e),
            ProcessError::SignalError(msg) => write!(f, "Signal error: {}", msg),
            ProcessError::Protected(name) => write!(f, "Process '{}' is protected", name),
            ProcessError::InvalidSignal(sig) => write!(f, "Invalid signal: {}", sig),
        }
    }
}

impl std::error::Error for ProcessError {}

impl From<io::Error> for ProcessError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::PermissionDenied {
            ProcessError::PermissionDenied(e.to_string())
        } else if e.kind() == io::ErrorKind::NotFound {
            ProcessError::NotFound(0)
        } else {
            ProcessError::IoError(e)
        }
    }
}

/// Linux-specific process manager
pub struct LinuxProcessManager {
    /// System information provider
    system: System,
    /// Set of protected process names (lowercase)
    protected_processes: HashSet<String>,
}

impl LinuxProcessManager {
    /// Create a new LinuxProcessManager with default protected processes
    pub fn new() -> Self {
        let mut manager = Self {
            system: System::new_all(),
            protected_processes: HashSet::new(),
        };
        manager.init_protected_processes();
        manager
    }

    /// Initialize the default list of protected processes
    fn init_protected_processes(&mut self) {
        let protected = [
            // Init systems
            "systemd",
            "init",
            "systemd-journald",
            "systemd-logind",
            "systemd-udevd",
            "systemd-networkd",
            "systemd-resolved",
            "systemd-timesyncd",

            // D-Bus
            "dbus-daemon",
            "dbus-broker",
            "dbus-broker-launch",

            // Networking
            "networkmanager",
            "nm-dispatcher",
            "wpa_supplicant",
            "dhclient",
            "dhcpcd",
            "connmand",

            // Logging
            "rsyslogd",
            "syslog-ng",
            "journald",

            // Display servers
            "xorg",
            "x",
            "xwayland",
            "wayland",

            // Wayland compositors
            "gnome-shell",
            "kwin_wayland",
            "kwin_x11",
            "sway",
            "weston",
            "mutter",
            "enlightenment",
            "wayfire",
            "river",
            "hyprland",

            // Desktop environments
            "plasmashell",
            "gnome-session",
            "xfce4-session",
            "mate-session",
            "cinnamon-session",
            "lxsession",
            "lxqt-session",
            "budgie-wm",
            "gala",
            "marco",
            "openbox",

            // Audio systems
            "pulseaudio",
            "pipewire",
            "pipewire-pulse",
            "wireplumber",
            "jackd",
            "jackdbus",

            // Security / PAM / Polkit
            "polkitd",
            "polkit-gnome-authentication-agent-1",
            "gnome-keyring-daemon",
            "ssh-agent",
            "gpg-agent",
            "pcscd",

            // Power management
            "upowerd",
            "thermald",
            "tlp",
            "power-profiles-daemon",

            // Session / Seat management
            "gdm",
            "gdm-wayland-session",
            "gdm-x-session",
            "sddm",
            "lightdm",
            "lxdm",
            "xdm",
            "login",
            "agetty",

            // Hardware / udev
            "udevd",
            "udisksd",
            "colord",
            "cups",
            "cupsd",
            "avahi-daemon",
            "bluetoothd",

            // Filesystem
            "gvfsd",
            "gvfs-daemon",
            "udisks2",

            // System services
            "cron",
            "crond",
            "atd",
            "anacron",
            "systemd-cgroups-agent",
            "accounts-daemon",
            "rtkit-daemon",

            // Container runtimes (if running in host)
            "containerd",
            "dockerd",
            "docker",
            "podman",
            "crio",
        ];

        for name in protected {
            self.protected_processes.insert(name.to_lowercase());
        }
    }

    /// Refresh process information from the system
    pub fn refresh(&mut self) {
        self.system.refresh_all();
    }

    /// List all processes on the system
    pub fn list_processes(&mut self) -> Vec<ProcessInfo> {
        self.system.refresh_processes();

        self.system.processes()
            .iter()
            .filter_map(|(pid, process)| {
                let pid_u32 = pid.as_u32();
                self.build_process_info(pid_u32, process)
            })
            .collect()
    }

    /// Get information about a specific process
    pub fn get_process(&mut self, pid: u32) -> Option<ProcessInfo> {
        self.system.refresh_process(Pid::from_u32(pid));

        self.system.process(Pid::from_u32(pid))
            .and_then(|p| self.build_process_info(pid, p))
    }

    /// Build ProcessInfo from sysinfo Process and /proc data
    fn build_process_info(&self, pid: u32, process: &sysinfo::Process) -> Option<ProcessInfo> {
        // Get name from sysinfo, falling back to /proc/[pid]/comm
        let name = process.name().to_string();

        // Get cmdline from /proc/[pid]/cmdline
        let cmdline = self.read_cmdline(pid).unwrap_or_else(|| name.clone());

        // Get detailed info from /proc/[pid]/stat
        let (ppid, threads, state) = self.parse_proc_stat(pid)
            .unwrap_or((None, None, ProcessState::from_sysinfo(process.status())));

        // Get UID from process
        let uid = process.user_id().map(|u| **u);

        Some(ProcessInfo {
            pid,
            name,
            cmdline,
            memory_rss: process.memory(),
            memory_vms: process.virtual_memory(),
            cpu_usage: process.cpu_usage(),
            state,
            ppid,
            uid,
            threads,
        })
    }

    /// Read command line from /proc/[pid]/cmdline
    fn read_cmdline(&self, pid: u32) -> Option<String> {
        let path = format!("/proc/{}/cmdline", pid);
        fs::read(&path).ok().map(|bytes| {
            // Arguments are null-separated
            bytes.iter()
                .map(|&b| if b == 0 { ' ' } else { b as char })
                .collect::<String>()
                .trim()
                .to_string()
        })
    }

    /// Read process name from /proc/[pid]/comm
    #[allow(dead_code)]
    fn read_comm(&self, pid: u32) -> Option<String> {
        let path = format!("/proc/{}/comm", pid);
        fs::read_to_string(&path).ok().map(|s| s.trim().to_string())
    }

    /// Parse /proc/[pid]/stat for detailed process information
    fn parse_proc_stat(&self, pid: u32) -> Option<(Option<u32>, Option<u32>, ProcessState)> {
        let path = format!("/proc/{}/stat", pid);
        let content = fs::read_to_string(&path).ok()?;

        // Format: pid (comm) state ppid ...
        // The comm field can contain spaces and parentheses, so we need to find
        // the last ')' to properly parse
        let comm_end = content.rfind(')')?;
        let after_comm = &content[comm_end + 2..]; // Skip ") "

        let fields: Vec<&str> = after_comm.split_whitespace().collect();
        if fields.is_empty() {
            return None;
        }

        // Field 0: state (after comm)
        let state_char = fields.get(0)?.chars().next()?;
        let state = ProcessState::from_stat_char(state_char);

        // Field 1: ppid
        let ppid = fields.get(1).and_then(|s| s.parse::<u32>().ok());

        // Field 17: num_threads (19th field in original stat, 17th after comm)
        let threads = fields.get(17).and_then(|s| s.parse::<u32>().ok());

        Some((ppid, threads, state))
    }

    /// Check if a process name is protected
    pub fn is_protected(&self, name: &str) -> bool {
        let name_lower = name.to_lowercase();

        // Check exact match
        if self.protected_processes.contains(&name_lower) {
            return true;
        }

        // Check if any protected name is contained in the process name
        // This handles cases like "gnome-shell-calendar-server"
        for protected in &self.protected_processes {
            if name_lower.contains(protected) || protected.contains(&name_lower) {
                return true;
            }
        }

        // Protect PID 1 (init/systemd) regardless of name
        false
    }

    /// Check if a specific PID is protected (including PID 1)
    pub fn is_pid_protected(&self, pid: u32) -> bool {
        // PID 1 is always protected (init/systemd)
        if pid == 1 {
            return true;
        }

        // PID 2 is kthreadd on Linux, also protected
        if pid == 2 {
            return true;
        }

        // Check by process name
        if let Some(info) = self.system.process(Pid::from_u32(pid)) {
            return self.is_protected(info.name());
        }

        false
    }

    /// Get list of all protected process names
    pub fn get_protected_processes(&self) -> Vec<String> {
        self.protected_processes.iter().cloned().collect()
    }

    /// Add a process name to the protected list
    pub fn add_protected(&mut self, name: &str) {
        self.protected_processes.insert(name.to_lowercase());
    }

    /// Remove a process name from the protected list
    pub fn remove_protected(&mut self, name: &str) -> bool {
        self.protected_processes.remove(&name.to_lowercase())
    }

    /// Send a signal to a process
    ///
    /// Common signals:
    /// - SIGTERM (15): Graceful termination
    /// - SIGKILL (9): Force kill
    /// - SIGHUP (1): Hangup
    /// - SIGSTOP (19): Pause process
    /// - SIGCONT (18): Resume process
    pub fn kill_process(&mut self, pid: u32, signal: i32) -> Result<(), ProcessError> {
        // Validate signal number (1-31 for standard signals)
        if signal < 1 || signal > 31 {
            return Err(ProcessError::InvalidSignal(signal));
        }

        // Check if process exists
        self.system.refresh_process(Pid::from_u32(pid));
        let process = self.system.process(Pid::from_u32(pid))
            .ok_or(ProcessError::NotFound(pid))?;

        // Check if protected
        if self.is_pid_protected(pid) {
            return Err(ProcessError::Protected(process.name().to_string()));
        }

        // Use libc to send signal
        // SAFETY: kill() is safe to call with any pid and signal,
        // it will return an error if invalid
        let result = unsafe { libc::kill(pid as i32, signal) };

        if result == 0 {
            Ok(())
        } else {
            let errno = std::io::Error::last_os_error();
            match errno.raw_os_error() {
                Some(libc::ESRCH) => Err(ProcessError::NotFound(pid)),
                Some(libc::EPERM) => Err(ProcessError::PermissionDenied(
                    format!("Cannot send signal {} to process {}", signal, pid)
                )),
                _ => Err(ProcessError::SignalError(errno.to_string())),
            }
        }
    }

    /// Terminate a process gracefully (SIGTERM)
    pub fn terminate_process(&mut self, pid: u32) -> Result<(), ProcessError> {
        self.kill_process(pid, libc::SIGTERM)
    }

    /// Force kill a process (SIGKILL)
    pub fn force_kill_process(&mut self, pid: u32) -> Result<(), ProcessError> {
        self.kill_process(pid, libc::SIGKILL)
    }

    /// Get processes sorted by memory usage (descending)
    pub fn processes_by_memory(&mut self) -> Vec<ProcessInfo> {
        let mut processes = self.list_processes();
        processes.sort_by(|a, b| b.memory_rss.cmp(&a.memory_rss));
        processes
    }

    /// Get processes sorted by CPU usage (descending)
    pub fn processes_by_cpu(&mut self) -> Vec<ProcessInfo> {
        let mut processes = self.list_processes();
        processes.sort_by(|a, b| b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap_or(std::cmp::Ordering::Equal));
        processes
    }

    /// Get zombie processes
    pub fn get_zombies(&mut self) -> Vec<ProcessInfo> {
        self.list_processes()
            .into_iter()
            .filter(|p| p.is_zombie())
            .collect()
    }

    /// Get total memory usage of all processes (RSS in bytes)
    pub fn total_memory_usage(&mut self) -> u64 {
        self.system.refresh_processes();
        self.system.processes()
            .values()
            .map(|p| p.memory())
            .sum()
    }

    /// Find processes by name (case-insensitive partial match)
    pub fn find_by_name(&mut self, name: &str) -> Vec<ProcessInfo> {
        let name_lower = name.to_lowercase();
        self.list_processes()
            .into_iter()
            .filter(|p| p.name.to_lowercase().contains(&name_lower))
            .collect()
    }

    /// Get process tree (processes with their children)
    pub fn get_process_tree(&mut self, root_pid: u32) -> Vec<ProcessInfo> {
        let all_processes = self.list_processes();
        let mut result = Vec::new();

        // Find root process
        if let Some(root) = all_processes.iter().find(|p| p.pid == root_pid) {
            result.push(root.clone());
        }

        // Find all descendants
        let mut to_check = vec![root_pid];
        while let Some(parent_pid) = to_check.pop() {
            for proc in &all_processes {
                if proc.ppid == Some(parent_pid) && proc.pid != parent_pid {
                    result.push(proc.clone());
                    to_check.push(proc.pid);
                }
            }
        }

        result
    }
}

impl Default for LinuxProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_state_from_char() {
        assert_eq!(ProcessState::from_stat_char('R'), ProcessState::Running);
        assert_eq!(ProcessState::from_stat_char('S'), ProcessState::Sleeping);
        assert_eq!(ProcessState::from_stat_char('D'), ProcessState::DiskSleep);
        assert_eq!(ProcessState::from_stat_char('Z'), ProcessState::Zombie);
        assert_eq!(ProcessState::from_stat_char('T'), ProcessState::Stopped);
        assert_eq!(ProcessState::from_stat_char('?'), ProcessState::Unknown);
    }

    #[test]
    fn test_protected_processes() {
        let manager = LinuxProcessManager::new();

        // Test known protected processes
        assert!(manager.is_protected("systemd"));
        assert!(manager.is_protected("init"));
        assert!(manager.is_protected("gnome-shell"));
        assert!(manager.is_protected("pulseaudio"));
        assert!(manager.is_protected("dbus-daemon"));

        // Test case insensitivity
        assert!(manager.is_protected("SYSTEMD"));
        assert!(manager.is_protected("Gnome-Shell"));

        // Test non-protected
        assert!(!manager.is_protected("firefox"));
        assert!(!manager.is_protected("code"));
    }

    #[test]
    fn test_add_remove_protected() {
        let mut manager = LinuxProcessManager::new();

        // Add custom protected process
        manager.add_protected("myapp");
        assert!(manager.is_protected("myapp"));
        assert!(manager.is_protected("MYAPP"));

        // Remove it
        assert!(manager.remove_protected("myapp"));
        assert!(!manager.is_protected("myapp"));
    }

    #[test]
    fn test_pid_1_always_protected() {
        let manager = LinuxProcessManager::new();
        assert!(manager.is_pid_protected(1));
        assert!(manager.is_pid_protected(2));
    }

    #[test]
    fn test_process_error_display() {
        let err = ProcessError::NotFound(1234);
        assert_eq!(format!("{}", err), "Process 1234 not found");

        let err = ProcessError::Protected("systemd".to_string());
        assert_eq!(format!("{}", err), "Process 'systemd' is protected");
    }

    #[test]
    fn test_process_info_memory_mb() {
        let info = ProcessInfo {
            pid: 1,
            name: "test".to_string(),
            cmdline: "test".to_string(),
            memory_rss: 1024 * 1024 * 100, // 100 MB
            memory_vms: 1024 * 1024 * 200,
            cpu_usage: 0.0,
            state: ProcessState::Running,
            ppid: None,
            uid: None,
            threads: None,
        };

        assert!((info.memory_mb() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_invalid_signal() {
        let mut manager = LinuxProcessManager::new();

        // Signal 0 is invalid for our purposes
        let result = manager.kill_process(99999, 0);
        assert!(matches!(result, Err(ProcessError::InvalidSignal(0))));

        // Signal 100 is invalid
        let result = manager.kill_process(99999, 100);
        assert!(matches!(result, Err(ProcessError::InvalidSignal(100))));
    }
}
