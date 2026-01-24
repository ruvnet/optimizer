//! Settings persistence for the tray application
//!
//! Saves and loads user preferences to a TOML config file.

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

impl TraySettings {
    /// Get the config file path
    pub fn config_path() -> PathBuf {
        // Use %APPDATA%\RuVector\memopt.toml
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let mut path = PathBuf::from(appdata);
            path.push("RuVector");
            path.push("memopt.toml");
            return path;
        }

        // Fallback to executable directory
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                return dir.join("memopt.toml");
            }
        }

        // Last resort
        PathBuf::from("memopt.toml")
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

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

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
}
