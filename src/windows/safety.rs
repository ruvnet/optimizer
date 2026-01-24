//! Safety mechanisms to prevent system instability

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
                // Windows critical
                "system".into(),
                "csrss.exe".into(),
                "smss.exe".into(),
                "lsass.exe".into(),
                "services.exe".into(),
                "wininit.exe".into(),
                "winlogon.exe".into(),
                "dwm.exe".into(),
                "explorer.exe".into(),
                // Security
                "msmpeng.exe".into(), // Windows Defender
                "securityhealthservice.exe".into(),
                // Anti-virus common
                "avgnt.exe".into(),
                "avp.exe".into(),
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
        self.config.protected_processes.iter()
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
            warn!("Optimization failed. Consecutive failures: {}", self.consecutive_failures);
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
}

#[derive(Debug, Clone)]
pub struct SafetyStats {
    pub total_optimizations: usize,
    pub consecutive_failures: usize,
    pub is_healthy: bool,
}
