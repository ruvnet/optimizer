//! Safety mechanisms for macOS memory optimization
//!
//! Prevents system instability by protecting critical processes
//! and enforcing rate limits.

use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Protected process names that should never be optimized
pub const PROTECTED_PROCESSES: &[&str] = &[
    // Kernel and core
    "kernel_task",
    "launchd",
    "syslogd",
    "configd",
    "mDNSResponder",

    // Window system
    "WindowServer",
    "loginwindow",
    "Dock",
    "Finder",
    "SystemUIServer",
    "ControlCenter",
    "NotificationCenter",

    // Security
    "securityd",
    "trustd",
    "secd",
    "keybagd",
    "BiometricKit",

    // Core services
    "coreaudiod",
    "bluetoothd",
    "airportd",
    "wifid",
    "usbd",
    "coreduetd",

    // System daemons
    "cfprefsd",
    "diskarbitrationd",
    "fseventsd",
    "mds_stores",
    "backupd",
    "powerd",
    "thermald",

    // Apple Silicon specific
    "AMPDeviceDiscoveryAgent",
    "AMPLibraryAgent",
    "gpu_driver_spawn",
    "mediaremoted",
    "audiomxd",

    // Development tools (don't kill dev processes)
    "Xcode",
    "SourceKitService",
    "lldb",
    "swift-frontend",
    "clangd",
];

/// Safety configuration
#[derive(Debug, Clone)]
pub struct SafetyConfig {
    /// Minimum available memory to maintain (MB)
    pub min_available_mb: f64,
    /// Maximum optimization frequency
    pub min_interval: Duration,
    /// Maximum processes to affect per optimization
    pub max_processes_per_run: usize,
    /// Enable dry-run mode (no actual changes)
    pub dry_run: bool,
    /// Additional protected process names
    pub additional_protected: Vec<String>,
    /// Respect system memory pressure (don't optimize if already low)
    pub respect_system_pressure: bool,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            min_available_mb: 2048.0, // Keep at least 2GB free on macOS
            min_interval: Duration::from_secs(60),
            max_processes_per_run: 30,
            dry_run: false,
            additional_protected: vec![],
            respect_system_pressure: true,
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
                "Available memory ({:.0}MB) already below safety floor ({:.0}MB) - no optimization needed",
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

        // Check built-in protected list
        if PROTECTED_PROCESSES
            .iter()
            .any(|p| name_lower.contains(&p.to_lowercase()))
        {
            return true;
        }

        // Check user-added protected list
        self.config
            .additional_protected
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
        self.consecutive_failures = 100;
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
    fn test_protected_processes() {
        let guard = SafetyGuard::new(SafetyConfig::default());

        assert!(guard.is_protected("kernel_task"));
        assert!(guard.is_protected("WindowServer"));
        assert!(guard.is_protected("Finder"));
        assert!(!guard.is_protected("my_custom_app"));
    }

    #[test]
    fn test_safety_check() {
        let guard = SafetyGuard::new(SafetyConfig::default());

        // Should pass with high available memory
        assert!(guard.check_safe(8000.0).is_ok());

        // Should fail with low available memory
        assert!(guard.check_safe(1000.0).is_err());
    }
}
