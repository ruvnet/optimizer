//! Settings persistence for the tray application
//!
//! Saves and loads user preferences to a TOML config file.
//! Supports cross-platform paths:
//! - Windows: %APPDATA%\RuVector\
//! - Linux: XDG Base Directory specification (~/.config/ruvector/, ~/.local/share/ruvector/, etc.)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application settings that persist between sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraySettings {
    /// Auto-optimization enabled
    pub auto_optimize: bool,
    /// Memory threshold for auto-optimization (percentage)
    pub threshold: u32,
    /// Auto-optimization interval in seconds
    pub interval_secs: u64,

    /// AI Mode settings
    pub ai_mode: AIModeSettings,
}

/// AI Mode specific settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIModeSettings {
    /// Game Mode auto-detection enabled
    pub game_mode: bool,
    /// Focus Mode auto-detection enabled
    pub focus_mode: bool,
    /// Thermal prediction enabled
    pub thermal_prediction: bool,
    /// Predictive preloading enabled
    pub predictive_preload: bool,
    /// GPU monitoring enabled (requires 'ai' feature)
    pub gpu_monitoring: bool,
    /// VRAM reserve percentage (keep this much free)
    pub vram_reserve_percent: u32,
}

impl Default for TraySettings {
    fn default() -> Self {
        Self {
            auto_optimize: true,
            threshold: 75,
            interval_secs: 60,
            ai_mode: AIModeSettings::default(),
        }
    }
}

impl Default for AIModeSettings {
    fn default() -> Self {
        Self {
            game_mode: true,
            focus_mode: true,
            thermal_prediction: true,
            predictive_preload: true,
            gpu_monitoring: true,
            vram_reserve_percent: 5,
        }
    }
}

/// Cross-platform directory helpers
pub mod paths {
    use std::path::PathBuf;

    /// Application name for directory paths
    const APP_NAME: &str = "ruvector";

    /// Get the configuration directory path
    /// - Windows: %APPDATA%\RuVector\
    /// - Linux: $XDG_CONFIG_HOME/ruvector/ (default: ~/.config/ruvector/)
    pub fn config_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(appdata) = std::env::var_os("APPDATA") {
                let mut path = PathBuf::from(appdata);
                path.push("RuVector");
                return path;
            }
            // Fallback to executable directory
            fallback_dir()
        }

        #[cfg(target_os = "linux")]
        {
            // XDG_CONFIG_HOME or ~/.config
            if let Some(xdg_config) = std::env::var_os("XDG_CONFIG_HOME") {
                let mut path = PathBuf::from(xdg_config);
                path.push(APP_NAME);
                return path;
            }
            if let Some(config_dir) = dirs::config_dir() {
                return config_dir.join(APP_NAME);
            }
            // Fallback to ~/.config/ruvector
            if let Some(home) = dirs::home_dir() {
                return home.join(".config").join(APP_NAME);
            }
            fallback_dir()
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            // macOS and others: use dirs crate
            if let Some(config_dir) = dirs::config_dir() {
                return config_dir.join(APP_NAME);
            }
            fallback_dir()
        }
    }

    /// Get the data directory path
    /// - Windows: %APPDATA%\RuVector\data\
    /// - Linux: $XDG_DATA_HOME/ruvector/ (default: ~/.local/share/ruvector/)
    pub fn data_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            config_dir().join("data")
        }

        #[cfg(target_os = "linux")]
        {
            // XDG_DATA_HOME or ~/.local/share
            if let Some(xdg_data) = std::env::var_os("XDG_DATA_HOME") {
                let mut path = PathBuf::from(xdg_data);
                path.push(APP_NAME);
                return path;
            }
            if let Some(data_dir) = dirs::data_dir() {
                return data_dir.join(APP_NAME);
            }
            // Fallback to ~/.local/share/ruvector
            if let Some(home) = dirs::home_dir() {
                return home.join(".local").join("share").join(APP_NAME);
            }
            fallback_dir().join("data")
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            if let Some(data_dir) = dirs::data_dir() {
                return data_dir.join(APP_NAME);
            }
            fallback_dir().join("data")
        }
    }

    /// Get the cache directory path
    /// - Windows: %LOCALAPPDATA%\RuVector\cache\ or %APPDATA%\RuVector\cache\
    /// - Linux: $XDG_CACHE_HOME/ruvector/ (default: ~/.cache/ruvector/)
    pub fn cache_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            // Prefer LOCALAPPDATA for cache
            if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA") {
                let mut path = PathBuf::from(local_appdata);
                path.push("RuVector");
                path.push("cache");
                return path;
            }
            config_dir().join("cache")
        }

        #[cfg(target_os = "linux")]
        {
            // XDG_CACHE_HOME or ~/.cache
            if let Some(xdg_cache) = std::env::var_os("XDG_CACHE_HOME") {
                let mut path = PathBuf::from(xdg_cache);
                path.push(APP_NAME);
                return path;
            }
            if let Some(cache_dir) = dirs::cache_dir() {
                return cache_dir.join(APP_NAME);
            }
            // Fallback to ~/.cache/ruvector
            if let Some(home) = dirs::home_dir() {
                return home.join(".cache").join(APP_NAME);
            }
            fallback_dir().join("cache")
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            if let Some(cache_dir) = dirs::cache_dir() {
                return cache_dir.join(APP_NAME);
            }
            fallback_dir().join("cache")
        }
    }

    /// Get the log directory path (systemd journal compatible on Linux)
    /// - Windows: %APPDATA%\RuVector\logs\
    /// - Linux: $XDG_STATE_HOME/ruvector/ (default: ~/.local/state/ruvector/) or /var/log/ruvector for system service
    pub fn log_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            config_dir().join("logs")
        }

        #[cfg(target_os = "linux")]
        {
            // For user-level logging, use XDG_STATE_HOME
            // systemd will handle journal logging separately
            if let Some(xdg_state) = std::env::var_os("XDG_STATE_HOME") {
                let mut path = PathBuf::from(xdg_state);
                path.push(APP_NAME);
                return path;
            }
            if let Some(state_dir) = dirs::state_dir() {
                return state_dir.join(APP_NAME);
            }
            // Fallback to ~/.local/state/ruvector
            if let Some(home) = dirs::home_dir() {
                return home.join(".local").join("state").join(APP_NAME);
            }
            fallback_dir().join("logs")
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            if let Some(state_dir) = dirs::state_dir() {
                return state_dir.join(APP_NAME);
            }
            fallback_dir().join("logs")
        }
    }

    /// Fallback directory (executable directory or current directory)
    fn fallback_dir() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                return dir.to_path_buf();
            }
        }
        PathBuf::from(".")
    }

    /// Ensure a directory exists, creating it if necessary
    pub fn ensure_dir(path: &PathBuf) -> std::io::Result<()> {
        if !path.exists() {
            std::fs::create_dir_all(path)?;
            tracing::debug!("Created directory: {:?}", path);
        }
        Ok(())
    }

    /// Initialize all application directories
    pub fn init_all_dirs() -> std::io::Result<()> {
        ensure_dir(&config_dir())?;
        ensure_dir(&data_dir())?;
        ensure_dir(&cache_dir())?;
        ensure_dir(&log_dir())?;
        Ok(())
    }
}

impl TraySettings {
    /// Get the config file path
    pub fn config_path() -> PathBuf {
        paths::config_dir().join("memopt.toml")
    }

    /// Get the config directory
    pub fn config_dir() -> PathBuf {
        paths::config_dir()
    }

    /// Get the data directory
    pub fn data_dir() -> PathBuf {
        paths::data_dir()
    }

    /// Get the cache directory
    pub fn cache_dir() -> PathBuf {
        paths::cache_dir()
    }

    /// Get the log directory
    pub fn log_dir() -> PathBuf {
        paths::log_dir()
    }

    /// Load settings from config file, or return defaults
    pub fn load() -> Self {
        let path = Self::config_path();

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match toml::from_str(&content) {
                        Ok(settings) => {
                            tracing::info!("Loaded settings from {:?}", path);
                            return settings;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse settings: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read settings file: {}", e);
                }
            }
        }

        tracing::info!("Using default settings");
        Self::default()
    }

    /// Save settings to config file
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();

        // Ensure config directory exists
        paths::ensure_dir(&paths::config_dir())
            .map_err(|e| format!("Failed to create config directory: {}", e))?;

        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write settings: {}", e))?;

        tracing::info!("Saved settings to {:?}", path);
        Ok(())
    }

    /// Update a single setting and save
    pub fn update<F>(&mut self, f: F) -> Result<(), String>
    where
        F: FnOnce(&mut Self),
    {
        f(self);
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = TraySettings::default();
        assert!(settings.auto_optimize);
        assert_eq!(settings.threshold, 75);
        assert!(settings.ai_mode.game_mode);
    }

    #[test]
    fn test_serialize_deserialize() {
        let settings = TraySettings::default();
        let toml = toml::to_string(&settings).unwrap();
        let restored: TraySettings = toml::from_str(&toml).unwrap();
        assert_eq!(settings.threshold, restored.threshold);
    }

    #[test]
    fn test_config_path_not_empty() {
        let path = TraySettings::config_path();
        assert!(!path.as_os_str().is_empty());
        assert!(path.ends_with("memopt.toml"));
    }

    #[test]
    fn test_directory_paths_not_empty() {
        assert!(!paths::config_dir().as_os_str().is_empty());
        assert!(!paths::data_dir().as_os_str().is_empty());
        assert!(!paths::cache_dir().as_os_str().is_empty());
        assert!(!paths::log_dir().as_os_str().is_empty());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_xdg_paths() {
        // Test that paths contain expected components on Linux
        let config = paths::config_dir();
        let data = paths::data_dir();
        let cache = paths::cache_dir();
        let log = paths::log_dir();

        // All paths should contain "ruvector"
        assert!(config.to_string_lossy().contains("ruvector"));
        assert!(data.to_string_lossy().contains("ruvector"));
        assert!(cache.to_string_lossy().contains("ruvector"));
        assert!(log.to_string_lossy().contains("ruvector"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_paths() {
        // Test that paths contain expected components on Windows
        let config = paths::config_dir();

        // Windows paths should contain "RuVector"
        assert!(config.to_string_lossy().contains("RuVector"));
    }

    #[test]
    fn test_tray_settings_directory_methods() {
        // Test the convenience methods on TraySettings
        assert_eq!(TraySettings::config_dir(), paths::config_dir());
        assert_eq!(TraySettings::data_dir(), paths::data_dir());
        assert_eq!(TraySettings::cache_dir(), paths::cache_dir());
        assert_eq!(TraySettings::log_dir(), paths::log_dir());
    }
}
