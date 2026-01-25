//! Linux systemd daemon/service management for RuVector Memory Optimizer
//!
//! This module provides:
//! - systemd service file generation and installation
//! - System-wide and user service support
//! - Daemon mode with PID file management
//! - Signal handling (SIGTERM, SIGINT, SIGHUP)

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ============================================================================
// Constants
// ============================================================================

/// Service name used for systemd unit
pub const SERVICE_NAME: &str = "ruvector-memopt";

/// Systemd service file template for system-wide installation
pub const SYSTEMD_SERVICE_TEMPLATE: &str = r#"[Unit]
Description=RuVector Memory Optimizer
Documentation=https://github.com/ruvnet/optimizer
After=network.target
Wants=network-online.target

[Service]
Type=simple
ExecStart={exec_path} daemon
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=5
TimeoutStopSec=30

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictNamespaces=true
RestrictRealtime=true
RestrictSUIDSGID=true
MemoryDenyWriteExecute=true
LockPersonality=true

# Resource limits
MemoryMax=256M
CPUQuota=25%

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier={service_name}

[Install]
WantedBy=multi-user.target
"#;

/// Systemd user service template (less restrictive)
pub const SYSTEMD_USER_SERVICE_TEMPLATE: &str = r#"[Unit]
Description=RuVector Memory Optimizer (User Service)
Documentation=https://github.com/ruvnet/optimizer
After=default.target

[Service]
Type=simple
ExecStart={exec_path} daemon --user
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=5
TimeoutStopSec=30

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier={service_name}

[Install]
WantedBy=default.target
"#;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during daemon operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonError {
    /// Service is already running
    AlreadyRunning,
    /// Service is not running
    NotRunning,
    /// Failed to create PID file
    PidFileError(String),
    /// Failed to execute systemctl command
    SystemctlError(String),
    /// Permission denied (requires root/sudo)
    PermissionDenied(String),
    /// Service installation failed
    InstallError(String),
    /// Service uninstallation failed
    UninstallError(String),
    /// Failed to read/write service file
    ServiceFileError(String),
    /// Signal handling error
    SignalError(String),
    /// Generic IO error
    IoError(String),
    /// Configuration error
    ConfigError(String),
}

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning => write!(f, "Service is already running"),
            Self::NotRunning => write!(f, "Service is not running"),
            Self::PidFileError(msg) => write!(f, "PID file error: {}", msg),
            Self::SystemctlError(msg) => write!(f, "systemctl error: {}", msg),
            Self::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Self::InstallError(msg) => write!(f, "Installation failed: {}", msg),
            Self::UninstallError(msg) => write!(f, "Uninstallation failed: {}", msg),
            Self::ServiceFileError(msg) => write!(f, "Service file error: {}", msg),
            Self::SignalError(msg) => write!(f, "Signal error: {}", msg),
            Self::IoError(msg) => write!(f, "IO error: {}", msg),
            Self::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for DaemonError {}

impl From<std::io::Error> for DaemonError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::PermissionDenied => {
                DaemonError::PermissionDenied(err.to_string())
            }
            _ => DaemonError::IoError(err.to_string()),
        }
    }
}

// ============================================================================
// Status Types
// ============================================================================

/// Current status of the daemon service
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaemonStatus {
    /// Service is running
    Running {
        /// Process ID of the running daemon
        pid: u32,
        /// Uptime in seconds
        uptime_secs: u64,
    },
    /// Service is stopped
    Stopped,
    /// Service status is unknown or failed to query
    Unknown(String),
    /// Service is starting
    Starting,
    /// Service is stopping
    Stopping,
    /// Service failed
    Failed(String),
}

impl DaemonStatus {
    /// Check if the daemon is in a running state
    pub fn is_running(&self) -> bool {
        matches!(self, DaemonStatus::Running { .. })
    }

    /// Check if the daemon is stopped
    pub fn is_stopped(&self) -> bool {
        matches!(self, DaemonStatus::Stopped)
    }
}

impl std::fmt::Display for DaemonStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running { pid, uptime_secs } => {
                let hours = uptime_secs / 3600;
                let mins = (uptime_secs % 3600) / 60;
                let secs = uptime_secs % 60;
                write!(f, "Running (PID: {}, Uptime: {:02}:{:02}:{:02})", pid, hours, mins, secs)
            }
            Self::Stopped => write!(f, "Stopped"),
            Self::Unknown(msg) => write!(f, "Unknown: {}", msg),
            Self::Starting => write!(f, "Starting"),
            Self::Stopping => write!(f, "Stopping"),
            Self::Failed(msg) => write!(f, "Failed: {}", msg),
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the daemon service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// Whether to run as a user service (vs system-wide)
    pub user_mode: bool,
    /// Custom path to the executable
    pub exec_path: Option<PathBuf>,
    /// PID file location override
    pub pid_file: Option<PathBuf>,
    /// Log level (debug, info, warn, error)
    pub log_level: String,
    /// Optimization interval in seconds
    pub interval_secs: u64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            user_mode: false,
            exec_path: None,
            pid_file: None,
            log_level: "info".to_string(),
            interval_secs: 60,
        }
    }
}

// ============================================================================
// Signal Handler
// ============================================================================

/// Thread-safe signal state for daemon shutdown coordination
#[derive(Clone)]
pub struct SignalState {
    /// Flag indicating shutdown was requested
    shutdown_requested: Arc<AtomicBool>,
    /// Flag indicating reload was requested
    reload_requested: Arc<AtomicBool>,
}

impl SignalState {
    /// Create a new signal state
    pub fn new() -> Self {
        Self {
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            reload_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if shutdown was requested
    pub fn shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }

    /// Request shutdown
    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
    }

    /// Check if reload was requested
    pub fn reload_requested(&self) -> bool {
        self.reload_requested.load(Ordering::SeqCst)
    }

    /// Request reload
    pub fn request_reload(&self) {
        self.reload_requested.store(true, Ordering::SeqCst);
    }

    /// Clear reload flag after handling
    pub fn clear_reload(&self) {
        self.reload_requested.store(false, Ordering::SeqCst);
    }
}

impl Default for SignalState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Linux Daemon Service
// ============================================================================

/// Linux systemd daemon service manager
///
/// Provides methods to install, uninstall, start, stop, and manage the
/// RuVector Memory Optimizer as a systemd service on Linux systems.
///
/// # Example
///
/// ```no_run
/// use ruvector_memopt::platform::linux::daemon::{LinuxDaemonService, DaemonConfig};
///
/// let config = DaemonConfig::default();
/// let service = LinuxDaemonService::new(config);
///
/// // Install as system-wide service (requires root)
/// service.install(true).expect("Failed to install service");
///
/// // Start the service
/// service.start().expect("Failed to start service");
///
/// // Check status
/// let status = service.status();
/// println!("Service status: {}", status);
/// ```
pub struct LinuxDaemonService {
    /// Service configuration
    config: DaemonConfig,
    /// Signal state for daemon coordination
    signal_state: SignalState,
}

impl LinuxDaemonService {
    /// Create a new daemon service manager
    pub fn new(config: DaemonConfig) -> Self {
        Self {
            config,
            signal_state: SignalState::new(),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(DaemonConfig::default())
    }

    /// Get the signal state for external coordination
    pub fn signal_state(&self) -> &SignalState {
        &self.signal_state
    }

    // ========================================================================
    // Path Helpers
    // ========================================================================

    /// Get the path to the systemd service file
    fn service_file_path(&self, system_wide: bool) -> PathBuf {
        if system_wide {
            PathBuf::from(format!("/etc/systemd/system/{}.service", SERVICE_NAME))
        } else {
            // User service directory
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(format!(
                "{}/.config/systemd/user/{}.service",
                home, SERVICE_NAME
            ))
        }
    }

    /// Get the path to the PID file
    fn pid_file_path(&self) -> PathBuf {
        if let Some(ref path) = self.config.pid_file {
            return path.clone();
        }

        if self.config.user_mode {
            // User PID file in /tmp or XDG_RUNTIME_DIR
            let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
                .unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(format!("{}/{}.pid", runtime_dir, SERVICE_NAME))
        } else {
            // System PID file in /run
            PathBuf::from(format!("/run/{}.pid", SERVICE_NAME))
        }
    }

    /// Get the path to the executable
    fn exec_path(&self) -> PathBuf {
        if let Some(ref path) = self.config.exec_path {
            return path.clone();
        }

        // Try to find the executable
        if let Ok(current_exe) = std::env::current_exe() {
            return current_exe;
        }

        // Default system path
        PathBuf::from("/usr/bin/ruvector-memopt")
    }

    // ========================================================================
    // Service Management
    // ========================================================================

    /// Start the daemon service via systemctl
    pub fn start(&self) -> Result<(), DaemonError> {
        if self.is_running() {
            return Err(DaemonError::AlreadyRunning);
        }

        let service_unit = service_unit_name();
        let args: Vec<&str> = if self.config.user_mode {
            vec!["--user", "start", &service_unit]
        } else {
            vec!["start", &service_unit]
        };

        run_systemctl(&args)?;

        // Wait briefly and verify it started
        std::thread::sleep(Duration::from_millis(500));

        if !self.is_running() {
            return Err(DaemonError::SystemctlError(
                "Service failed to start".to_string()
            ));
        }

        Ok(())
    }

    /// Stop the daemon service via systemctl
    pub fn stop(&self) -> Result<(), DaemonError> {
        if !self.is_running() {
            return Err(DaemonError::NotRunning);
        }

        let service_unit = service_unit_name();
        let args: Vec<&str> = if self.config.user_mode {
            vec!["--user", "stop", &service_unit]
        } else {
            vec!["stop", &service_unit]
        };

        run_systemctl(&args)?;

        // Clean up PID file
        let pid_file = self.pid_file_path();
        if pid_file.exists() {
            let _ = fs::remove_file(&pid_file);
        }

        Ok(())
    }

    /// Get the current status of the daemon service
    pub fn status(&self) -> DaemonStatus {
        // First check via systemctl
        let service_unit = service_unit_name();
        let args: Vec<&str> = if self.config.user_mode {
            vec!["--user", "is-active", &service_unit]
        } else {
            vec!["is-active", &service_unit]
        };

        let output = Command::new("systemctl")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        match output {
            Ok(out) => {
                let status_str = String::from_utf8_lossy(&out.stdout).trim().to_string();

                match status_str.as_str() {
                    "active" => {
                        // Try to get PID and uptime
                        if let Some(pid) = self.read_pid() {
                            let uptime = self.get_uptime().unwrap_or(0);
                            DaemonStatus::Running { pid, uptime_secs: uptime }
                        } else {
                            DaemonStatus::Running { pid: 0, uptime_secs: 0 }
                        }
                    }
                    "inactive" | "dead" => DaemonStatus::Stopped,
                    "activating" => DaemonStatus::Starting,
                    "deactivating" => DaemonStatus::Stopping,
                    "failed" => {
                        // Get failure reason
                        let reason = self.get_failure_reason().unwrap_or_else(|| "Unknown".to_string());
                        DaemonStatus::Failed(reason)
                    }
                    _ => DaemonStatus::Unknown(status_str),
                }
            }
            Err(e) => DaemonStatus::Unknown(e.to_string()),
        }
    }

    /// Check if the daemon is currently running
    pub fn is_running(&self) -> bool {
        self.status().is_running()
    }

    /// Reload the daemon configuration (sends SIGHUP)
    pub fn reload(&self) -> Result<(), DaemonError> {
        if !self.is_running() {
            return Err(DaemonError::NotRunning);
        }

        let service_unit = service_unit_name();
        let args: Vec<&str> = if self.config.user_mode {
            vec!["--user", "reload", &service_unit]
        } else {
            vec!["reload", &service_unit]
        };

        run_systemctl(&args)
    }

    /// Restart the daemon service
    pub fn restart(&self) -> Result<(), DaemonError> {
        let service_unit = service_unit_name();
        let args: Vec<&str> = if self.config.user_mode {
            vec!["--user", "restart", &service_unit]
        } else {
            vec!["restart", &service_unit]
        };

        run_systemctl(&args)
    }

    // ========================================================================
    // Installation
    // ========================================================================

    /// Install the systemd service
    ///
    /// # Arguments
    /// * `system_wide` - If true, install to /etc/systemd/system/ (requires root).
    ///                   If false, install to ~/.config/systemd/user/
    pub fn install(&self, system_wide: bool) -> Result<(), DaemonError> {
        // Check permissions for system-wide install
        if system_wide && !is_root() {
            return Err(DaemonError::PermissionDenied(
                "System-wide installation requires root privileges. Use sudo.".to_string()
            ));
        }

        let service_path = self.service_file_path(system_wide);

        // Create parent directory if needed (for user services)
        if !system_wide {
            if let Some(parent) = service_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    DaemonError::InstallError(format!(
                        "Failed to create directory {}: {}",
                        parent.display(),
                        e
                    ))
                })?;
            }
        }

        // Generate service file content
        let service_content = self.generate_service_file(system_wide);

        // Write service file
        let mut file = File::create(&service_path).map_err(|e| {
            DaemonError::ServiceFileError(format!(
                "Failed to create service file {}: {}",
                service_path.display(),
                e
            ))
        })?;

        file.write_all(service_content.as_bytes()).map_err(|e| {
            DaemonError::ServiceFileError(format!(
                "Failed to write service file: {}",
                e
            ))
        })?;

        // Reload systemd daemon
        let reload_args: Vec<&str> = if system_wide {
            vec!["daemon-reload"]
        } else {
            vec!["--user", "daemon-reload"]
        };

        run_systemctl(&reload_args)?;

        // Enable the service (auto-start on boot)
        let service_unit = service_unit_name();
        let enable_args: Vec<&str> = if system_wide {
            vec!["enable", &service_unit]
        } else {
            vec!["--user", "enable", &service_unit]
        };

        run_systemctl(&enable_args)?;

        Ok(())
    }

    /// Uninstall the systemd service
    pub fn uninstall(&self) -> Result<(), DaemonError> {
        // Try to stop if running
        if self.is_running() {
            let _ = self.stop();
        }

        let service_unit = service_unit_name();

        // Disable and remove both user and system services
        for system_wide in [true, false] {
            let service_path = self.service_file_path(system_wide);

            if service_path.exists() {
                // Check permissions
                if system_wide && !is_root() {
                    return Err(DaemonError::PermissionDenied(
                        "System-wide uninstallation requires root privileges".to_string()
                    ));
                }

                // Disable the service
                let disable_args: Vec<&str> = if system_wide {
                    vec!["disable", &service_unit]
                } else {
                    vec!["--user", "disable", &service_unit]
                };

                let _ = run_systemctl(&disable_args);

                // Remove service file
                fs::remove_file(&service_path).map_err(|e| {
                    DaemonError::UninstallError(format!(
                        "Failed to remove service file {}: {}",
                        service_path.display(),
                        e
                    ))
                })?;

                // Reload daemon
                let reload_args: Vec<&str> = if system_wide {
                    vec!["daemon-reload"]
                } else {
                    vec!["--user", "daemon-reload"]
                };

                let _ = run_systemctl(&reload_args);
            }
        }

        // Clean up PID file
        let pid_file = self.pid_file_path();
        if pid_file.exists() {
            let _ = fs::remove_file(&pid_file);
        }

        Ok(())
    }

    /// Generate the systemd service file content
    fn generate_service_file(&self, system_wide: bool) -> String {
        let template = if system_wide {
            SYSTEMD_SERVICE_TEMPLATE
        } else {
            SYSTEMD_USER_SERVICE_TEMPLATE
        };

        template
            .replace("{exec_path}", &self.exec_path().display().to_string())
            .replace("{service_name}", SERVICE_NAME)
    }

    // ========================================================================
    // Daemon Mode (Running as a Daemon)
    // ========================================================================

    /// Run in daemon mode (foreground process managed by systemd)
    ///
    /// This method sets up signal handlers and runs the main daemon loop.
    /// It should be called when the binary is invoked with the `daemon` subcommand.
    ///
    /// # Arguments
    /// * `main_loop` - A closure that runs the main optimization loop.
    ///                 It receives a `SignalState` and should check `shutdown_requested()`
    ///                 to know when to exit.
    pub fn run_daemon<F>(&self, main_loop: F) -> Result<(), DaemonError>
    where
        F: FnOnce(SignalState) -> Result<(), Box<dyn std::error::Error>>,
    {
        // Write PID file
        self.write_pid()?;

        // Set up signal handlers
        self.setup_signal_handlers()?;

        // Run the main loop
        let result = main_loop(self.signal_state.clone());

        // Clean up PID file
        let _ = self.remove_pid();

        result.map_err(|e| DaemonError::IoError(e.to_string()))
    }

    /// Set up signal handlers for SIGTERM, SIGINT, and SIGHUP
    #[cfg(target_os = "linux")]
    fn setup_signal_handlers(&self) -> Result<(), DaemonError> {
        use signal_hook::consts::{SIGTERM, SIGINT, SIGHUP};
        use signal_hook::flag;

        let shutdown = self.signal_state.shutdown_requested.clone();
        let reload = self.signal_state.reload_requested.clone();

        // SIGTERM and SIGINT trigger shutdown
        flag::register(SIGTERM, shutdown.clone())
            .map_err(|e| DaemonError::SignalError(format!("Failed to register SIGTERM: {}", e)))?;

        flag::register(SIGINT, shutdown)
            .map_err(|e| DaemonError::SignalError(format!("Failed to register SIGINT: {}", e)))?;

        // SIGHUP triggers reload
        flag::register(SIGHUP, reload)
            .map_err(|e| DaemonError::SignalError(format!("Failed to register SIGHUP: {}", e)))?;

        Ok(())
    }

    /// Fallback signal handler setup for non-Linux platforms
    #[cfg(not(target_os = "linux"))]
    fn setup_signal_handlers(&self) -> Result<(), DaemonError> {
        // On non-Linux platforms, we don't set up signal handlers
        // The daemon will need to be stopped via other means
        tracing::warn!("Signal handlers not available on this platform");
        Ok(())
    }

    // ========================================================================
    // PID File Management
    // ========================================================================

    /// Write the current process PID to the PID file
    fn write_pid(&self) -> Result<(), DaemonError> {
        let pid_path = self.pid_file_path();

        // Check if already running
        if let Some(existing_pid) = self.read_pid() {
            if process_exists(existing_pid) {
                return Err(DaemonError::AlreadyRunning);
            }
            // Stale PID file, remove it
            let _ = fs::remove_file(&pid_path);
        }

        // Ensure parent directory exists
        if let Some(parent) = pid_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    DaemonError::PidFileError(format!(
                        "Failed to create PID directory {}: {}",
                        parent.display(),
                        e
                    ))
                })?;
            }
        }

        // Write PID
        let pid = std::process::id();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&pid_path)
            .map_err(|e| {
                DaemonError::PidFileError(format!(
                    "Failed to create PID file {}: {}",
                    pid_path.display(),
                    e
                ))
            })?;

        writeln!(file, "{}", pid).map_err(|e| {
            DaemonError::PidFileError(format!("Failed to write PID: {}", e))
        })?;

        Ok(())
    }

    /// Read PID from the PID file
    fn read_pid(&self) -> Option<u32> {
        let pid_path = self.pid_file_path();

        if !pid_path.exists() {
            return None;
        }

        let mut content = String::new();
        if let Ok(mut file) = File::open(&pid_path) {
            if file.read_to_string(&mut content).is_ok() {
                return content.trim().parse().ok();
            }
        }

        None
    }

    /// Remove the PID file
    fn remove_pid(&self) -> Result<(), DaemonError> {
        let pid_path = self.pid_file_path();

        if pid_path.exists() {
            fs::remove_file(&pid_path).map_err(|e| {
                DaemonError::PidFileError(format!(
                    "Failed to remove PID file: {}",
                    e
                ))
            })?;
        }

        Ok(())
    }

    // ========================================================================
    // Status Helpers
    // ========================================================================

    /// Get the uptime of the service in seconds
    fn get_uptime(&self) -> Option<u64> {
        let service_unit = service_unit_name();
        let output = if self.config.user_mode {
            Command::new("systemctl")
                .args(["--user", "show", &service_unit, "--property=ActiveEnterTimestamp"])
                .stdout(Stdio::piped())
                .output()
                .ok()?
        } else {
            Command::new("systemctl")
                .args(["show", &service_unit, "--property=ActiveEnterTimestamp"])
                .stdout(Stdio::piped())
                .output()
                .ok()?
        };

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse timestamp: ActiveEnterTimestamp=Thu 2024-01-15 10:30:00 UTC
        if let Some(ts_str) = stdout.strip_prefix("ActiveEnterTimestamp=") {
            let ts_str = ts_str.trim();
            if ts_str.is_empty() || ts_str == "n/a" {
                return None;
            }

            // Use systemd-analyze to get uptime
            let uptime_output = if self.config.user_mode {
                Command::new("systemctl")
                    .args(["--user", "show", &service_unit, "--property=ActiveEnterTimestampMonotonic"])
                    .stdout(Stdio::piped())
                    .output()
                    .ok()?
            } else {
                Command::new("systemctl")
                    .args(["show", &service_unit, "--property=ActiveEnterTimestampMonotonic"])
                    .stdout(Stdio::piped())
                    .output()
                    .ok()?
            };

            let uptime_str = String::from_utf8_lossy(&uptime_output.stdout);
            if let Some(usec_str) = uptime_str.strip_prefix("ActiveEnterTimestampMonotonic=") {
                if let Ok(start_usec) = usec_str.trim().parse::<u64>() {
                    // Get current monotonic time
                    // This is an approximation since we can't easily get monotonic time
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_micros() as u64)
                        .unwrap_or(0);

                    // Return approximate uptime in seconds
                    return Some((now.saturating_sub(start_usec)) / 1_000_000);
                }
            }
        }

        None
    }

    /// Get the failure reason if the service has failed
    fn get_failure_reason(&self) -> Option<String> {
        let service_unit = service_unit_name();
        let output = if self.config.user_mode {
            Command::new("systemctl")
                .args(["--user", "show", &service_unit, "--property=Result"])
                .stdout(Stdio::piped())
                .output()
                .ok()?
        } else {
            Command::new("systemctl")
                .args(["show", &service_unit, "--property=Result"])
                .stdout(Stdio::piped())
                .output()
                .ok()?
        };

        let stdout = String::from_utf8_lossy(&output.stdout);

        if let Some(result) = stdout.strip_prefix("Result=") {
            let result = result.trim();
            if result != "success" {
                return Some(result.to_string());
            }
        }

        None
    }

    /// Get recent log entries for the service
    pub fn get_logs(&self, lines: u32) -> Result<String, DaemonError> {
        let service_unit = service_unit_name();
        let lines_str = lines.to_string();

        let output = if self.config.user_mode {
            Command::new("journalctl")
                .args(["--user", "-u", &service_unit, "-n", &lines_str, "--no-pager"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| DaemonError::IoError(format!("Failed to get logs: {}", e)))?
        } else {
            Command::new("journalctl")
                .args(["-u", &service_unit, "-n", &lines_str, "--no-pager"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| DaemonError::IoError(format!("Failed to get logs: {}", e)))?
        };

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(DaemonError::IoError(format!(
                "journalctl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Run a systemctl command and return the result
fn run_systemctl(args: &[&str]) -> Result<(), DaemonError> {
    let output = Command::new("systemctl")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            DaemonError::SystemctlError(format!("Failed to execute systemctl: {}", e))
        })?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Check for permission errors
        if stderr.contains("Access denied") || stderr.contains("Permission denied") {
            Err(DaemonError::PermissionDenied(stderr.to_string()))
        } else {
            Err(DaemonError::SystemctlError(stderr.to_string()))
        }
    }
}

/// Check if running as root
fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Check if a process with the given PID exists
fn process_exists(pid: u32) -> bool {
    // Check if /proc/PID exists
    Path::new(&format!("/proc/{}", pid)).exists()
}

/// Send a signal to a process
pub fn send_signal(pid: u32, signal: i32) -> Result<(), DaemonError> {
    let result = unsafe { libc::kill(pid as i32, signal) };

    if result == 0 {
        Ok(())
    } else {
        Err(DaemonError::SignalError(format!(
            "Failed to send signal {} to PID {}",
            signal, pid
        )))
    }
}

/// Daemonize the current process (double-fork method)
///
/// Note: When running under systemd, daemonization is not needed as systemd
/// handles process management. This function is provided for standalone use.
#[cfg(target_os = "linux")]
pub fn daemonize() -> Result<(), DaemonError> {
    use std::os::unix::io::AsRawFd;

    // First fork
    match unsafe { libc::fork() } {
        -1 => return Err(DaemonError::IoError("First fork failed".to_string())),
        0 => {} // Child continues
        _ => std::process::exit(0), // Parent exits
    }

    // Create new session
    if unsafe { libc::setsid() } == -1 {
        return Err(DaemonError::IoError("setsid failed".to_string()));
    }

    // Second fork
    match unsafe { libc::fork() } {
        -1 => return Err(DaemonError::IoError("Second fork failed".to_string())),
        0 => {} // Child continues
        _ => std::process::exit(0), // Parent exits
    }

    // Change working directory to root
    let _ = std::env::set_current_dir("/");

    // Close standard file descriptors and redirect to /dev/null
    let dev_null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|e| DaemonError::IoError(format!("Failed to open /dev/null: {}", e)))?;

    let null_fd = dev_null.as_raw_fd();

    unsafe {
        libc::dup2(null_fd, libc::STDIN_FILENO);
        libc::dup2(null_fd, libc::STDOUT_FILENO);
        libc::dup2(null_fd, libc::STDERR_FILENO);
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn daemonize() -> Result<(), DaemonError> {
    Err(DaemonError::IoError("Daemonization not supported on this platform".to_string()))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        assert!(!config.user_mode);
        assert_eq!(config.log_level, "info");
        assert_eq!(config.interval_secs, 60);
    }

    #[test]
    fn test_signal_state() {
        let state = SignalState::new();
        assert!(!state.shutdown_requested());
        assert!(!state.reload_requested());

        state.request_shutdown();
        assert!(state.shutdown_requested());

        state.request_reload();
        assert!(state.reload_requested());

        state.clear_reload();
        assert!(!state.reload_requested());
    }

    #[test]
    fn test_daemon_status_display() {
        let running = DaemonStatus::Running { pid: 1234, uptime_secs: 3661 };
        assert!(running.to_string().contains("1234"));
        assert!(running.to_string().contains("01:01:01"));

        let stopped = DaemonStatus::Stopped;
        assert_eq!(stopped.to_string(), "Stopped");

        let failed = DaemonStatus::Failed("timeout".to_string());
        assert!(failed.to_string().contains("timeout"));
    }

    #[test]
    fn test_service_file_paths() {
        let config = DaemonConfig {
            user_mode: false,
            ..Default::default()
        };
        let service = LinuxDaemonService::new(config);

        let system_path = service.service_file_path(true);
        assert!(system_path.to_string_lossy().contains("/etc/systemd/system/"));

        let user_path = service.service_file_path(false);
        assert!(user_path.to_string_lossy().contains(".config/systemd/user/"));
    }

    #[test]
    fn test_generate_service_file() {
        let config = DaemonConfig {
            exec_path: Some(PathBuf::from("/usr/local/bin/test")),
            ..Default::default()
        };
        let service = LinuxDaemonService::new(config);

        let content = service.generate_service_file(true);
        assert!(content.contains("/usr/local/bin/test"));
        assert!(content.contains("multi-user.target"));
        assert!(content.contains("Description=RuVector Memory Optimizer"));

        let user_content = service.generate_service_file(false);
        assert!(user_content.contains("default.target"));
        assert!(user_content.contains("User Service"));
    }

    #[test]
    fn test_daemon_error_display() {
        let err = DaemonError::AlreadyRunning;
        assert_eq!(err.to_string(), "Service is already running");

        let err = DaemonError::PermissionDenied("test".to_string());
        assert!(err.to_string().contains("Permission denied"));
    }

    #[test]
    fn test_daemon_status_is_running() {
        assert!(DaemonStatus::Running { pid: 1, uptime_secs: 0 }.is_running());
        assert!(!DaemonStatus::Stopped.is_running());
        assert!(!DaemonStatus::Starting.is_running());
        assert!(!DaemonStatus::Failed("error".to_string()).is_running());
    }
}
