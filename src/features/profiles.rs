//! ADR-013: Workspace Profiles
//!
//! Context-aware memory optimization profiles with persistence, import/export,
//! and profile-switch history tracking.
//!
//! IPC messages handled:
//!   get_profiles, get_profile_history, set_profile_setting,
//!   set_profile, create_profile, import_profile, export_profile, delete_profile

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::config_dir;

// ── Data Structures ────────────────────────────────────────────────────

/// A single workspace profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Unique identifier (lowercase, slug-like).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Short description.
    #[serde(default)]
    pub description: String,
    /// Memory threshold percentage for auto-optimize.
    #[serde(default = "default_threshold")]
    pub threshold: u32,
    /// Auto-optimization interval in seconds.
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    /// Whether auto-optimize is enabled.
    #[serde(default = "default_true")]
    pub auto_optimize: bool,
    /// AI sub-mode toggles.
    #[serde(default)]
    pub ai_modes: AIModes,
    /// Process priority hint (informational).
    #[serde(default = "default_priority")]
    pub priority: String,
    /// Memory allocation percentages keyed by category.
    #[serde(default)]
    pub mem_alloc: HashMap<String, u32>,
    /// ISO-8601 creation timestamp.
    #[serde(default)]
    pub created_at: String,
    /// Whether this is a built-in (non-deletable) profile.
    #[serde(default)]
    pub builtin: bool,
}

/// AI mode toggles stored per profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIModes {
    #[serde(default = "default_true")]
    pub game_mode: bool,
    #[serde(default = "default_true")]
    pub focus_mode: bool,
    #[serde(default = "default_true")]
    pub thermal_prediction: bool,
    #[serde(default = "default_true")]
    pub predictive_preload: bool,
}

impl Default for AIModes {
    fn default() -> Self {
        Self {
            game_mode: true,
            focus_mode: true,
            thermal_prediction: true,
            predictive_preload: true,
        }
    }
}

fn default_threshold() -> u32 {
    75
}
fn default_interval() -> u64 {
    60
}
fn default_true() -> bool {
    true
}
fn default_priority() -> String {
    "normal".into()
}

/// A single entry in the profile-switch history log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub profile_id: String,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Global profile settings (auto-detect, detection method, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSettings {
    #[serde(default = "default_true")]
    pub auto_detect: bool,
    #[serde(default = "default_detection_method")]
    pub detection_method: String,
    #[serde(default = "default_switch_delay")]
    pub switch_delay: u32,
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            auto_detect: true,
            detection_method: default_detection_method(),
            switch_delay: default_switch_delay(),
            hotkey: default_hotkey(),
        }
    }
}

fn default_detection_method() -> String {
    "foreground".into()
}
fn default_switch_delay() -> u32 {
    5
}
fn default_hotkey() -> String {
    "Ctrl+Shift+P".into()
}

/// Persistent state serialized to TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileStore {
    active_id: String,
    settings: ProfileSettings,
    profiles: Vec<Profile>,
}

// ── ProfileManager ─────────────────────────────────────────────────────

/// Manages profile CRUD, persistence, and history.
pub struct ProfileManager {
    store: ProfileStore,
    history: Vec<HistoryEntry>,
}

impl ProfileManager {
    /// Load profiles from disk or create defaults.
    pub fn load() -> Self {
        let store = Self::load_store();
        let history = Self::load_history();
        Self { store, history }
    }

    // ── IPC dispatch ───────────────────────────────────────────────

    /// Handle an IPC message. Returns `Some(json)` if the message type is
    /// recognized, `None` otherwise.
    pub fn handle_ipc(&mut self, msg_type: &str, payload: &serde_json::Value) -> Option<String> {
        match msg_type {
            "get_profiles" => Some(self.get_profiles_json()),
            "get_profile_history" => Some(self.get_history_json()),
            "set_profile_setting" => {
                self.set_profile_setting(payload);
                Some(self.get_profiles_json())
            }
            "set_profile" => {
                if let Some(id) = payload.get("profileId").and_then(|v| v.as_str()) {
                    self.switch_profile(id);
                }
                Some(self.get_profiles_json())
            }
            "create_profile" => {
                self.create_profile(payload);
                Some(self.get_profiles_json())
            }
            "import_profile" => {
                // import_profile expects the full profile JSON in the payload
                self.import_profile(payload);
                Some(self.get_profiles_json())
            }
            "export_profile" => {
                let id = payload
                    .get("profileId")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.store.active_id);
                Some(self.export_profile(id))
            }
            "delete_profile" => {
                if let Some(id) = payload.get("profileId").and_then(|v| v.as_str()) {
                    self.delete_profile(id);
                }
                Some(self.get_profiles_json())
            }
            _ => None,
        }
    }

    // ── Queries ────────────────────────────────────────────────────

    fn get_profiles_json(&self) -> String {
        let resp = serde_json::json!({
            "profiles": self.store.profiles,
            "activeId": self.store.active_id,
            "settings": self.store.settings,
        });
        resp.to_string()
    }

    fn get_history_json(&self) -> String {
        serde_json::json!({ "history": self.history }).to_string()
    }

    fn export_profile(&self, id: &str) -> String {
        if let Some(p) = self.store.profiles.iter().find(|p| p.id == id) {
            match serde_json::to_string_pretty(p) {
                Ok(json) => json,
                Err(e) => {
                    tracing::warn!("Failed to serialize profile for export: {}", e);
                    serde_json::json!({"error": e.to_string()}).to_string()
                }
            }
        } else {
            serde_json::json!({"error": "Profile not found"}).to_string()
        }
    }

    // ── Mutations ──────────────────────────────────────────────────

    fn switch_profile(&mut self, id: &str) {
        if self.store.profiles.iter().any(|p| p.id == id) {
            self.store.active_id = id.to_string();

            // Record in history, cap at 100 entries
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            self.history.push(HistoryEntry {
                profile_id: id.to_string(),
                timestamp: now_ms,
            });
            if self.history.len() > 100 {
                self.history.drain(0..self.history.len() - 100);
            }

            self.save_all();
            tracing::info!("Switched to profile: {}", id);
        } else {
            tracing::warn!("Attempted to switch to unknown profile: {}", id);
        }
    }

    fn set_profile_setting(&mut self, payload: &serde_json::Value) {
        let key = match payload.get("key").and_then(|v| v.as_str()) {
            Some(k) => k,
            None => return,
        };
        let value = match payload.get("value") {
            Some(v) => v,
            None => return,
        };

        match key {
            "autoDetect" => {
                if let Some(b) = value.as_bool() {
                    self.store.settings.auto_detect = b;
                }
            }
            "detectionMethod" => {
                if let Some(s) = value.as_str() {
                    self.store.settings.detection_method = s.to_string();
                }
            }
            "switchDelay" => {
                if let Some(n) = value.as_u64() {
                    self.store.settings.switch_delay = n as u32;
                }
            }
            "hotkey" => {
                if let Some(s) = value.as_str() {
                    self.store.settings.hotkey = s.to_string();
                }
            }
            _ => {
                tracing::debug!("Unknown profile setting key: {}", key);
            }
        }

        self.save_all();
    }

    fn create_profile(&mut self, payload: &serde_json::Value) {
        let name = match payload.get("name").and_then(|v| v.as_str()) {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => return,
        };

        // Generate slug id
        let id = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>();

        // Prevent duplicates
        if self.store.profiles.iter().any(|p| p.id == id) {
            tracing::warn!("Profile with id '{}' already exists", id);
            return;
        }

        let priority = payload
            .get("priority")
            .and_then(|v| v.as_str())
            .unwrap_or("normal")
            .to_string();

        let now = chrono::Utc::now().to_rfc3339();

        let profile = Profile {
            id,
            name,
            description: String::new(),
            threshold: default_threshold(),
            interval_secs: default_interval(),
            auto_optimize: true,
            ai_modes: AIModes::default(),
            priority,
            mem_alloc: HashMap::new(),
            created_at: now,
            builtin: false,
        };

        tracing::info!("Created profile: {}", profile.id);
        self.store.profiles.push(profile);
        self.save_all();
    }

    fn import_profile(&mut self, payload: &serde_json::Value) {
        // Try to deserialize a Profile from the payload itself
        match serde_json::from_value::<Profile>(payload.clone()) {
            Ok(mut profile) => {
                profile.builtin = false;
                // Ensure unique id
                if self.store.profiles.iter().any(|p| p.id == profile.id) {
                    profile.id = format!("{}-imported", profile.id);
                }
                tracing::info!("Imported profile: {}", profile.id);
                self.store.profiles.push(profile);
                self.save_all();
            }
            Err(e) => {
                tracing::warn!("Failed to import profile: {}", e);
            }
        }
    }

    fn delete_profile(&mut self, id: &str) {
        // Never delete built-in profiles
        if let Some(p) = self.store.profiles.iter().find(|p| p.id == id) {
            if p.builtin {
                tracing::warn!("Cannot delete built-in profile: {}", id);
                return;
            }
        }

        let before = self.store.profiles.len();
        self.store.profiles.retain(|p| p.id != id);
        if self.store.profiles.len() < before {
            tracing::info!("Deleted profile: {}", id);
            // If the active profile was deleted, switch to first available
            if self.store.active_id == id {
                self.store.active_id = self
                    .store
                    .profiles
                    .first()
                    .map(|p| p.id.clone())
                    .unwrap_or_else(|| "balanced".into());
            }
            self.save_all();
        }
    }

    // ── Persistence ────────────────────────────────────────────────

    fn profiles_path() -> PathBuf {
        config_dir().join("profiles.toml")
    }

    fn history_path() -> PathBuf {
        config_dir().join("profile_history.json")
    }

    fn load_store() -> ProfileStore {
        let path = Self::profiles_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<ProfileStore>(&content) {
                    Ok(store) => {
                        tracing::info!("Loaded {} profiles from {:?}", store.profiles.len(), path);
                        return store;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse profiles file: {}", e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read profiles file: {}", e);
                }
            }
        }

        // Return defaults
        Self::default_store()
    }

    fn load_history() -> Vec<HistoryEntry> {
        let path = Self::history_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<Vec<HistoryEntry>>(&content) {
                    Ok(history) => return history,
                    Err(e) => {
                        tracing::warn!("Failed to parse profile history: {}", e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read profile history: {}", e);
                }
            }
        }
        Vec::new()
    }

    fn save_all(&self) {
        self.save_store();
        self.save_history();
    }

    fn save_store(&self) {
        let path = Self::profiles_path();
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Failed to create profiles directory: {}", e);
                return;
            }
        }
        match toml::to_string_pretty(&self.store) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    tracing::warn!("Failed to write profiles: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize profiles: {}", e);
            }
        }
    }

    fn save_history(&self) {
        let path = Self::history_path();
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Failed to create history directory: {}", e);
                return;
            }
        }
        match serde_json::to_string(&self.history) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    tracing::warn!("Failed to write profile history: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize profile history: {}", e);
            }
        }
    }

    fn default_store() -> ProfileStore {
        let now = chrono::Utc::now().to_rfc3339();

        let balanced = Profile {
            id: "balanced".into(),
            name: "Balanced".into(),
            description: "General-purpose profile with moderate optimization".into(),
            threshold: 75,
            interval_secs: 60,
            auto_optimize: true,
            ai_modes: AIModes::default(),
            priority: "normal".into(),
            mem_alloc: [
                ("primary".into(), 40u32),
                ("secondary".into(), 30),
                ("background".into(), 20),
                ("system".into(), 10),
            ]
            .into_iter()
            .collect(),
            created_at: now.clone(),
            builtin: true,
        };

        let performance = Profile {
            id: "performance".into(),
            name: "Performance".into(),
            description: "Aggressive optimization for maximum speed".into(),
            threshold: 60,
            interval_secs: 30,
            auto_optimize: true,
            ai_modes: AIModes {
                game_mode: true,
                focus_mode: false,
                thermal_prediction: true,
                predictive_preload: true,
            },
            priority: "high".into(),
            mem_alloc: [
                ("primary".into(), 60u32),
                ("secondary".into(), 20),
                ("background".into(), 10),
                ("system".into(), 10),
            ]
            .into_iter()
            .collect(),
            created_at: now.clone(),
            builtin: true,
        };

        let battery = Profile {
            id: "battery-saver".into(),
            name: "Battery Saver".into(),
            description: "Conservative optimization to extend battery life".into(),
            threshold: 85,
            interval_secs: 120,
            auto_optimize: true,
            ai_modes: AIModes {
                game_mode: false,
                focus_mode: true,
                thermal_prediction: true,
                predictive_preload: false,
            },
            priority: "below_normal".into(),
            mem_alloc: [
                ("primary".into(), 35u32),
                ("secondary".into(), 25),
                ("background".into(), 25),
                ("system".into(), 15),
            ]
            .into_iter()
            .collect(),
            created_at: now,
            builtin: true,
        };

        ProfileStore {
            active_id: "balanced".into(),
            settings: ProfileSettings::default(),
            profiles: vec![balanced, performance, battery],
        }
    }
}

// ── Module-level IPC entry point ──────────────────────────────────────

/// Free function called from `control_center.rs` IPC dispatch chain.
/// Loads state from disk, delegates to [`ProfileManager::handle_ipc`],
/// and returns the JSON response (if the message type was recognised).
pub fn handle_ipc(msg_type: &str, payload: &serde_json::Value) -> Option<String> {
    let mut mgr = ProfileManager::load();
    mgr.handle_ipc(msg_type, payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_profiles() {
        let mgr = ProfileManager {
            store: ProfileManager::default_store(),
            history: Vec::new(),
        };
        assert_eq!(mgr.store.profiles.len(), 3);
        assert_eq!(mgr.store.active_id, "balanced");
    }

    #[test]
    fn test_create_and_delete_profile() {
        let mut mgr = ProfileManager {
            store: ProfileManager::default_store(),
            history: Vec::new(),
        };

        let payload = serde_json::json!({"name": "Test Profile", "priority": "high"});
        mgr.create_profile(&payload);
        assert_eq!(mgr.store.profiles.len(), 4);

        mgr.delete_profile("test-profile");
        assert_eq!(mgr.store.profiles.len(), 3);
    }

    #[test]
    fn test_cannot_delete_builtin() {
        let mut mgr = ProfileManager {
            store: ProfileManager::default_store(),
            history: Vec::new(),
        };

        mgr.delete_profile("balanced");
        // Should still be there
        assert!(mgr.store.profiles.iter().any(|p| p.id == "balanced"));
    }

    #[test]
    fn test_switch_records_history() {
        let mut mgr = ProfileManager {
            store: ProfileManager::default_store(),
            history: Vec::new(),
        };

        mgr.switch_profile("performance");
        assert_eq!(mgr.store.active_id, "performance");
        assert_eq!(mgr.history.len(), 1);
        assert_eq!(mgr.history[0].profile_id, "performance");
    }

    #[test]
    fn test_handle_ipc_get_profiles() {
        let mut mgr = ProfileManager {
            store: ProfileManager::default_store(),
            history: Vec::new(),
        };

        let result = mgr.handle_ipc("get_profiles", &serde_json::json!({}));
        assert!(result.is_some());
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed.get("profiles").is_some());
        assert!(parsed.get("activeId").is_some());
    }

    #[test]
    fn test_handle_ipc_unknown_returns_none() {
        let mut mgr = ProfileManager {
            store: ProfileManager::default_store(),
            history: Vec::new(),
        };

        let result = mgr.handle_ipc("unknown_message", &serde_json::json!({}));
        assert!(result.is_none());
    }
}
