//! Configuration for the memory optimizer

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerConfig {
    /// Memory pressure threshold to trigger optimization (0-100)
    pub pressure_threshold: u32,
    
    /// Critical threshold for aggressive optimization
    pub critical_threshold: u32,
    
    /// Minimum interval between optimizations (seconds)
    pub min_interval_secs: u64,
    
    /// Enable neural decision making
    pub neural_enabled: bool,
    
    /// Path to neural model data
    pub model_path: PathBuf,
    
    /// Processes to never trim
    pub protected_processes: Vec<String>,
    
    /// Enable startup optimization mode
    pub startup_mode: bool,
    
    /// Aggressive mode clears system caches (requires admin)
    pub aggressive_mode: bool,
    
    /// Enable learning from optimization results
    pub learning_enabled: bool,
    
    /// EWC lambda for forgetting prevention
    pub ewc_lambda: f32,
    
    /// Benchmark mode - log detailed metrics
    pub benchmark_mode: bool,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            pressure_threshold: 80,
            critical_threshold: 95,
            min_interval_secs: 30,
            neural_enabled: true,
            model_path: PathBuf::from("./data/neural"),
            protected_processes: vec![
                "System".into(),
                "csrss.exe".into(),
                "smss.exe".into(),
                "lsass.exe".into(),
                "services.exe".into(),
            ],
            startup_mode: false,
            aggressive_mode: false,
            learning_enabled: true,
            ewc_lambda: 0.4,
            benchmark_mode: false,
        }
    }
}

impl OptimizerConfig {
    /// Load config from TOML file
    pub fn load(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// Save config to TOML file
    pub fn save(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
