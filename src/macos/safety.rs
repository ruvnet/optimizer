//! Safety mechanisms to prevent system instability on macOS

use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Safety configuration
#[derive(Debug, Clone)]
pub struct SafetyConfig {
    /// Minimum available memory to maintain (MB)
    pub min_available_mb: f64,
    /// Maximum optimization frequency
    pub min_interval: Duration,
    /// Maximum processes to trim per optimization
    pub max_processes_per_run: usize,
    /// Enable dry-run mode (no actual changes)
    pub dry_run: bool,
    /// Protected process names (case-insensitive)
    pub protected_processes: Vec<String>,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            min_available_mb: 1024.0, // Keep at least 1GB free
            min_interval: Duration::from_secs(30),
            max_processes_per_run: 50,
            dry_run: false,
            protected_processes: vec![
                // macOS kernel and core system
                "kernel_task".into(),
                "launchd".into(),
                "WindowServer".into(),
                "loginwindow".into(),

                // Spotlight and metadata services
                "mds".into(),
                "mds_stores".into(),
                "mdworker".into(),

                // Audio and Bluetooth
                "coreaudiod".into(),
                "bluetoothd".into(),
                "audiomxd".into(),

                // Disk and storage
                "diskarbitrationd".into(),
                "diskmanagementd".into(),
                "fseventsd".into(),

                // Security and keychain daemons
                "securityd".into(),
                "trustd".into(),
                "keychaind".into(),
                "secinitd".into(),

                // System services
                "cfprefsd".into(),
                "coreservicesd".into(),
                "opendirectoryd".into(),
                "notifyd".into(),
                "configd".into(),
                "powerd".into(),
                "distnoted".into(),

                // User session
                "Finder".into(),
                "Dock".into(),
                "SystemUIServer".into(),
                "UserEventAgent".into(),

                // Networking
                "mDNSResponder".into(),
                "networkd".into(),
                "symptomsd".into(),
                "nsurlsessiond".into(),

                // Display and graphics
                "coredisplayd".into(),
                "corebrightnessd".into(),

                // System integrity
                "watchdogd".into(),
                "logd".into(),
                "syslogd".into(),

                // XPC and IPC
                "xpcproxy".into(),
                "launchservicesd".into(),
            ],
        }
    }
}

/// Safety guard for memory optimization
pub struct SafetyGuard {
    config: SafetyConfig,
    last_optimization: Option<Instant>,
    consecutive_failures: usize,
    total_optimizations: usize,
}

impl SafetyGuard {
    pub fn new(config: SafetyConfig) -> Self {
        Self {
            config,
            last_optimization: None,
            consecutive_failures: 0,
            total_optimizations: 0,
        }
    }

    /// Check if optimization is safe to proceed
    pub fn check_safe(&self, current_available_mb: f64) -> Result<(), String> {
        // Check memory floor
        if current_available_mb < self.config.min_available_mb {
            return Err(format!(
                "Available memory ({:.0}MB) below safety floor ({:.0}MB)",
                current_available_mb, self.config.min_available_mb
            ));
        }

        // Check rate limit
        if let Some(last) = self.last_optimization {
            let elapsed = last.elapsed();
            if elapsed < self.config.min_interval {
                return Err(format!(
                    "Rate limited: {:?} remaining",
                    self.config.min_interval - elapsed
                ));
            }
        }

        // Check consecutive failures
        if self.consecutive_failures >= 3 {
            return Err(format!(
                "Too many consecutive failures ({}). Manual intervention needed.",
                self.consecutive_failures
            ));
        }

        Ok(())
    }

    /// Check if a process is protected
    pub fn is_protected(&self, process_name: &str) -> bool {
        let name_lower = process_name.to_lowercase();
        self.config
            .protected_processes
            .iter()
            .any(|p| name_lower.contains(&p.to_lowercase()))
    }

    /// Record optimization attempt
    pub fn record_attempt(&mut self, success: bool) {
        self.last_optimization = Some(Instant::now());
        self.total_optimizations += 1;

        if success {
            self.consecutive_failures = 0;
        } else {
            self.consecutive_failures += 1;
            warn!(
                "Optimization failed. Consecutive failures: {}",
                self.consecutive_failures
            );
        }
    }

    /// Check if dry-run mode
    pub fn is_dry_run(&self) -> bool {
        self.config.dry_run
    }

    /// Get max processes per run
    pub fn max_processes(&self) -> usize {
        self.config.max_processes_per_run
    }

    /// Emergency stop - disable further optimizations
    pub fn emergency_stop(&mut self) {
        self.consecutive_failures = 100; // Triggers safety check failure
        warn!("Emergency stop activated - optimizations disabled");
    }

    /// Reset safety counters
    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
        self.last_optimization = None;
        info!("Safety counters reset");
    }

    /// Get statistics
    pub fn stats(&self) -> SafetyStats {
        SafetyStats {
            total_optimizations: self.total_optimizations,
            consecutive_failures: self.consecutive_failures,
            is_healthy: self.consecutive_failures < 3,
        }
    }

    /// Get reference to config
    pub fn config(&self) -> &SafetyConfig {
        &self.config
    }

    /// Update config
    pub fn set_config(&mut self, config: SafetyConfig) {
        self.config = config;
    }
}

#[derive(Debug, Clone)]
pub struct SafetyStats {
    pub total_optimizations: usize,
    pub consecutive_failures: usize,
    pub is_healthy: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SafetyConfig::default();
        assert_eq!(config.min_available_mb, 1024.0);
        assert!(!config.dry_run);
        assert!(config.protected_processes.contains(&"kernel_task".to_string()));
        assert!(config.protected_processes.contains(&"launchd".to_string()));
        assert!(config.protected_processes.contains(&"securityd".to_string()));
    }

    #[test]
    fn test_is_protected() {
        let guard = SafetyGuard::new(SafetyConfig::default());

        // Protected processes
        assert!(guard.is_protected("kernel_task"));
        assert!(guard.is_protected("KERNEL_TASK")); // Case insensitive
        assert!(guard.is_protected("launchd"));
        assert!(guard.is_protected("WindowServer"));
        assert!(guard.is_protected("securityd"));
        assert!(guard.is_protected("mds_stores"));

        // Not protected
        assert!(!guard.is_protected("Safari"));
        assert!(!guard.is_protected("Chrome"));
        assert!(!guard.is_protected("SomeRandomApp"));
    }

    #[test]
    fn test_check_safe_memory() {
        let guard = SafetyGuard::new(SafetyConfig::default());

        // Should fail - below minimum
        let result = guard.check_safe(500.0);
        assert!(result.is_err());

        // Should pass - above minimum
        let result = guard.check_safe(2000.0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_consecutive_failures() {
        let mut guard = SafetyGuard::new(SafetyConfig::default());

        // Record failures
        guard.record_attempt(false);
        guard.record_attempt(false);
        assert!(guard.check_safe(2000.0).is_ok());

        guard.record_attempt(false);
        assert!(guard.check_safe(2000.0).is_err());

        // Reset should fix it
        guard.reset();
        assert!(guard.check_safe(2000.0).is_ok());
    }

    #[test]
    fn test_emergency_stop() {
        let mut guard = SafetyGuard::new(SafetyConfig::default());

        assert!(guard.check_safe(2000.0).is_ok());

        guard.emergency_stop();
        assert!(guard.check_safe(2000.0).is_err());
    }

    #[test]
    fn test_stats() {
        let mut guard = SafetyGuard::new(SafetyConfig::default());

        let stats = guard.stats();
        assert_eq!(stats.total_optimizations, 0);
        assert_eq!(stats.consecutive_failures, 0);
        assert!(stats.is_healthy);

        guard.record_attempt(true);
        guard.record_attempt(false);

        let stats = guard.stats();
        assert_eq!(stats.total_optimizations, 2);
        assert_eq!(stats.consecutive_failures, 1);
        assert!(stats.is_healthy);
    }

    #[test]
    fn test_dry_run() {
        let mut config = SafetyConfig::default();
        config.dry_run = true;

        let guard = SafetyGuard::new(config);
        assert!(guard.is_dry_run());
    }
}
