//! ADR-015: Startup Optimizer
//!
//! Cross-platform startup item enumeration, impact scoring, and
//! enable/disable management with undo support via backup snapshots.
//!
//! IPC messages handled:
//!   get_startup_items, get_boot_estimate, set_startup_item,
//!   optimize_startup, reset_startup

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::config_dir;

// ── Data Structures ────────────────────────────────────────────────────

/// A single startup item discovered on the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupItem {
    /// Unique numeric identifier.
    pub id: u32,
    /// Human-readable name.
    pub name: String,
    /// Full path or registry key.
    pub path: String,
    /// Whether the item is enabled.
    pub enabled: bool,
    /// Impact level: "low", "medium", or "high".
    pub impact: String,
    /// Category: "system", "user", "background", or "updater".
    pub category: String,
    /// Estimated memory usage in MB.
    #[serde(default)]
    pub memory_mb: u32,
    /// PageRank-style importance score (0.0 - 1.0).
    #[serde(default)]
    pub pagerank: f64,
}

/// Boot time estimate returned to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootEstimate {
    /// Current estimated boot time in seconds.
    pub current_secs: u32,
    /// Estimated boot time after optimization.
    pub optimized_secs: u32,
}

// ── StartupManager ─────────────────────────────────────────────────────

/// Cross-platform startup item manager with backup/restore support.
pub struct StartupManager {
    items: Vec<StartupItem>,
    /// A snapshot of the items before the last optimize/toggle for undo.
    backup: Option<Vec<StartupItem>>,
}

impl StartupManager {
    /// Create a new manager, enumerating startup items from the platform.
    pub fn load() -> Self {
        let items = enumerate_startup_items();
        Self {
            items,
            backup: None,
        }
    }

    /// Handle an IPC message. Returns `Some(json)` for recognized types.
    pub fn handle_ipc(&mut self, msg_type: &str, payload: &serde_json::Value) -> Option<String> {
        match msg_type {
            "get_startup_items" => Some(self.get_items_json()),
            "get_boot_estimate" => Some(self.get_boot_estimate_json()),
            "set_startup_item" => {
                self.set_startup_item(payload);
                Some(self.get_items_json())
            }
            "optimize_startup" => {
                self.optimize_startup();
                Some(self.get_items_json())
            }
            "reset_startup" => {
                self.reset_startup();
                Some(self.get_items_json())
            }
            _ => None,
        }
    }

    // ── Queries ────────────────────────────────────────────────────

    fn get_items_json(&self) -> String {
        let estimate = self.compute_boot_estimate();
        serde_json::json!({
            "items": self.items,
            "bootTimeCurrent": estimate.current_secs,
            "bootTimeOptimized": estimate.optimized_secs,
        })
        .to_string()
    }

    fn get_boot_estimate_json(&self) -> String {
        let estimate = self.compute_boot_estimate();
        serde_json::json!(estimate).to_string()
    }

    fn compute_boot_estimate(&self) -> BootEstimate {
        let total_memory: u32 = self.items.iter().map(|i| i.memory_mb).sum();
        let enabled_memory: u32 = self
            .items
            .iter()
            .filter(|i| i.enabled)
            .map(|i| i.memory_mb)
            .sum();
        let system_memory: u32 = self
            .items
            .iter()
            .filter(|i| i.category == "system")
            .map(|i| i.memory_mb)
            .sum();

        let total_f = if total_memory > 0 {
            total_memory as f64
        } else {
            1.0
        };

        let current = 20.0 + (enabled_memory as f64 / total_f) * 30.0;
        let optimized = 15.0 + (system_memory as f64 / total_f) * 15.0;

        BootEstimate {
            current_secs: current.round() as u32,
            optimized_secs: optimized.round() as u32,
        }
    }

    // ── Mutations ──────────────────────────────────────────────────

    fn set_startup_item(&mut self, payload: &serde_json::Value) {
        let item_id = match payload.get("itemId").and_then(|v| v.as_u64()) {
            Some(id) => id as u32,
            None => return,
        };
        let enabled = match payload.get("enabled").and_then(|v| v.as_bool()) {
            Some(e) => e,
            None => return,
        };

        // Save backup before change
        self.backup = Some(self.items.clone());

        if let Some(item) = self.items.iter_mut().find(|i| i.id == item_id) {
            item.enabled = enabled;
            tracing::info!(
                "Startup item '{}' set to {}",
                item.name,
                if enabled { "enabled" } else { "disabled" }
            );

            // Platform-specific enable/disable
            set_startup_enabled(&item.path, enabled, &item.category);
        }

        self.save_state();
    }

    fn optimize_startup(&mut self) {
        // Save full backup
        self.backup = Some(self.items.clone());

        let mut disabled_count = 0u32;
        for item in &mut self.items {
            // Only disable non-system, high/medium impact items
            if item.category != "system" && item.enabled {
                let should_disable = item.impact == "high"
                    || (item.impact == "medium" && item.pagerank < 0.5);
                if should_disable {
                    item.enabled = false;
                    set_startup_enabled(&item.path, false, &item.category);
                    disabled_count += 1;
                }
            }
        }

        tracing::info!(
            "Startup optimization disabled {} non-essential items",
            disabled_count
        );
        self.save_state();
    }

    fn reset_startup(&mut self) {
        if let Some(backup) = self.backup.take() {
            // Restore from backup
            for backup_item in &backup {
                if let Some(current) = self.items.iter_mut().find(|i| i.id == backup_item.id) {
                    if current.enabled != backup_item.enabled {
                        current.enabled = backup_item.enabled;
                        set_startup_enabled(
                            &current.path,
                            current.enabled,
                            &current.category,
                        );
                    }
                }
            }
            tracing::info!("Startup items restored from backup");
        } else {
            // No backup, enable everything
            for item in &mut self.items {
                if !item.enabled {
                    item.enabled = true;
                    set_startup_enabled(&item.path, true, &item.category);
                }
            }
            tracing::info!("All startup items re-enabled (no backup available)");
        }

        self.save_state();
    }

    // ── Persistence ────────────────────────────────────────────────

    fn state_path() -> PathBuf {
        config_dir().join("startup_state.json")
    }

    fn save_state(&self) {
        let path = Self::state_path();
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Failed to create startup state directory: {}", e);
                return;
            }
        }
        match serde_json::to_string_pretty(&self.items) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    tracing::warn!("Failed to write startup state: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize startup state: {}", e);
            }
        }
    }
}

// ── Platform-specific startup enumeration ──────────────────────────────

/// Enumerate startup items from the current platform.
fn enumerate_startup_items() -> Vec<StartupItem> {
    let mut items = Vec::new();
    let mut next_id = 1u32;

    #[cfg(target_os = "windows")]
    {
        items = enumerate_windows_startup(&mut next_id);
    }

    #[cfg(target_os = "macos")]
    {
        items = enumerate_macos_startup(&mut next_id);
    }

    #[cfg(target_os = "linux")]
    {
        items = enumerate_linux_startup(&mut next_id);
    }

    // If platform enumeration found nothing, provide sensible defaults
    // so the UI is not empty during development.
    if items.is_empty() {
        items = fallback_items();
    }

    // Assign PageRank scores based on category and impact heuristics
    for item in &mut items {
        item.pagerank = compute_pagerank(item);
    }

    items
}

/// Compute a heuristic PageRank-like importance score.
fn compute_pagerank(item: &StartupItem) -> f64 {
    let category_weight = match item.category.as_str() {
        "system" => 0.8,
        "background" => 0.4,
        "user" => 0.3,
        "updater" => 0.2,
        _ => 0.3,
    };
    let impact_weight = match item.impact.as_str() {
        "high" => 0.9,
        "medium" => 0.6,
        "low" => 0.3,
        _ => 0.5,
    };
    // Combine weights, cap at 0.99
    ((category_weight + impact_weight) / 2.0_f64).min(0.99_f64)
}

/// Classify impact based on estimated memory usage.
fn classify_impact(memory_mb: u32) -> &'static str {
    if memory_mb >= 200 {
        "high"
    } else if memory_mb >= 50 {
        "medium"
    } else {
        "low"
    }
}

// ── Windows startup enumeration ────────────────────────────────────────

#[cfg(target_os = "windows")]
fn enumerate_windows_startup(next_id: &mut u32) -> Vec<StartupItem> {
    let mut items = Vec::new();

    // Read HKCU\Software\Microsoft\Windows\CurrentVersion\Run
    let output = std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let line = line.trim();
                // Lines with REG_SZ or REG_EXPAND_SZ contain startup entries
                if line.contains("REG_SZ") || line.contains("REG_EXPAND_SZ") {
                    if let Some(item) = parse_reg_line(line, next_id, "user") {
                        items.push(item);
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to query startup registry: {}", e);
        }
    }

    // Also query HKLM Run (system-level)
    let output_lm = std::process::Command::new("reg")
        .args([
            "query",
            r"HKLM\Software\Microsoft\Windows\CurrentVersion\Run",
        ])
        .output();

    match output_lm {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let line = line.trim();
                if line.contains("REG_SZ") || line.contains("REG_EXPAND_SZ") {
                    if let Some(item) = parse_reg_line(line, next_id, "system") {
                        items.push(item);
                    }
                }
            }
        }
        Err(e) => {
            tracing::debug!("Failed to query HKLM startup registry (may need admin): {}", e);
        }
    }

    items
}

#[cfg(target_os = "windows")]
fn parse_reg_line(line: &str, next_id: &mut u32, default_category: &str) -> Option<StartupItem> {
    // Format: "    ValueName    REG_SZ    ValueData"
    let parts: Vec<&str> = line.splitn(3, "    ").collect();
    if parts.len() < 3 {
        // Try splitting on multiple spaces
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            return None;
        }
        let name = parts[0].to_string();
        let path = parts[parts.len() - 1].to_string();
        let id = *next_id;
        *next_id += 1;

        let memory_mb = estimate_memory_from_name(&name);
        let impact = classify_impact(memory_mb).to_string();
        let category = categorize_item(&name, default_category).to_string();

        return Some(StartupItem {
            id,
            name,
            path,
            enabled: true,
            impact,
            category,
            memory_mb,
            pagerank: 0.0,
        });
    }

    let name = parts[0].trim().to_string();
    let path = parts[2].trim().to_string();
    let id = *next_id;
    *next_id += 1;

    let memory_mb = estimate_memory_from_name(&name);
    let impact = classify_impact(memory_mb).to_string();
    let category = categorize_item(&name, default_category).to_string();

    Some(StartupItem {
        id,
        name,
        path,
        enabled: true,
        impact,
        category,
        memory_mb,
        pagerank: 0.0,
    })
}

// ── macOS startup enumeration ──────────────────────────────────────────

#[cfg(target_os = "macos")]
fn enumerate_macos_startup(next_id: &mut u32) -> Vec<StartupItem> {
    let mut items = Vec::new();

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let launch_agents_dir = std::path::PathBuf::from(&home).join("Library/LaunchAgents");

    if launch_agents_dir.exists() {
        match std::fs::read_dir(&launch_agents_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "plist").unwrap_or(false) {
                        let filename = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();

                        // Try to extract a human-readable name from the plist filename
                        // e.g. "com.apple.something" -> "Apple Something"
                        let name = humanize_plist_name(&filename);

                        let id = *next_id;
                        *next_id += 1;

                        let memory_mb = estimate_memory_from_name(&name);
                        let impact = classify_impact(memory_mb).to_string();

                        // Check if the plist has a Disabled key
                        let enabled = !is_plist_disabled(&path);
                        let category = categorize_item(&name, "user").to_string();

                        items.push(StartupItem {
                            id,
                            name,
                            path: path.to_string_lossy().to_string(),
                            enabled,
                            impact,
                            category,
                            memory_mb,
                            pagerank: 0.0,
                        });
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to read LaunchAgents: {}", e);
            }
        }
    }

    items
}

#[cfg(target_os = "macos")]
fn humanize_plist_name(filename: &str) -> String {
    // Convert "com.company.AppName" to "AppName"
    let parts: Vec<&str> = filename.split('.').collect();
    if parts.len() >= 3 {
        parts[2..].join(" ")
    } else if let Some(last) = parts.last() {
        last.to_string()
    } else {
        filename.to_string()
    }
}

#[cfg(target_os = "macos")]
fn is_plist_disabled(path: &std::path::Path) -> bool {
    // Quick heuristic: check if plist file content contains "Disabled.*true"
    match std::fs::read_to_string(path) {
        Ok(content) => {
            content.contains("<key>Disabled</key>")
                && content.contains("<true/>")
        }
        Err(_) => false,
    }
}

// ── Linux startup enumeration ──────────────────────────────────────────

#[cfg(target_os = "linux")]
fn enumerate_linux_startup(next_id: &mut u32) -> Vec<StartupItem> {
    let mut items = Vec::new();

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let autostart_dir = std::path::PathBuf::from(&home).join(".config/autostart");

    if autostart_dir.exists() {
        match std::fs::read_dir(&autostart_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "desktop").unwrap_or(false) {
                        if let Some(item) = parse_desktop_file(&path, next_id) {
                            items.push(item);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to read autostart directory: {}", e);
            }
        }
    }

    items
}

#[cfg(target_os = "linux")]
fn parse_desktop_file(path: &std::path::Path, next_id: &mut u32) -> Option<StartupItem> {
    let content = std::fs::read_to_string(path).ok()?;

    let mut name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut enabled = true;

    for line in content.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("Name=") {
            name = val.to_string();
        }
        if line.eq_ignore_ascii_case("Hidden=true") || line.eq_ignore_ascii_case("X-GNOME-Autostart-enabled=false") {
            enabled = false;
        }
    }

    let id = *next_id;
    *next_id += 1;

    let memory_mb = estimate_memory_from_name(&name);
    let impact = classify_impact(memory_mb).to_string();
    let category = categorize_item(&name, "user").to_string();

    Some(StartupItem {
        id,
        name,
        path: path.to_string_lossy().to_string(),
        enabled,
        impact,
        category,
        memory_mb,
        pagerank: 0.0,
    })
}

// ── Platform-specific enable/disable ───────────────────────────────────

/// Enable or disable a startup item on the current platform.
fn set_startup_enabled(path: &str, enabled: bool, _category: &str) {
    #[cfg(target_os = "windows")]
    {
        // For registry-based items we would need to add/remove the registry value.
        // This is a simplified version that logs the intent; full implementation
        // would use the `windows` crate registry APIs.
        tracing::info!(
            "Windows startup: {} -> {} (path: {})",
            if enabled { "enabling" } else { "disabling" },
            path,
            _category
        );
    }

    #[cfg(target_os = "macos")]
    {
        // For LaunchAgents, we can add/remove a Disabled key via `launchctl`
        // or by modifying the plist XML. For safety, we use launchctl.
        let action = if enabled { "load" } else { "unload" };
        match std::process::Command::new("launchctl")
            .args([action, path])
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::warn!("launchctl {} failed for {}: {}", action, path, stderr);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to run launchctl: {}", e);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // For .desktop files, toggle the Hidden= or X-GNOME-Autostart-enabled= key
        if let Ok(content) = std::fs::read_to_string(path) {
            let mut new_content = String::new();
            let mut found_hidden = false;

            for line in content.lines() {
                if line.starts_with("Hidden=") {
                    new_content.push_str(&format!("Hidden={}\n", !enabled));
                    found_hidden = true;
                } else if line.starts_with("X-GNOME-Autostart-enabled=") {
                    new_content.push_str(&format!(
                        "X-GNOME-Autostart-enabled={}\n",
                        enabled
                    ));
                    found_hidden = true;
                } else {
                    new_content.push_str(line);
                    new_content.push('\n');
                }
            }

            if !found_hidden {
                new_content.push_str(&format!("Hidden={}\n", !enabled));
            }

            if let Err(e) = std::fs::write(path, new_content) {
                tracing::warn!("Failed to update desktop file {}: {}", path, e);
            }
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Estimate memory usage of a startup item from its name (heuristic).
fn estimate_memory_from_name(name: &str) -> u32 {
    let lower = name.to_lowercase();

    // Well-known heavy hitters
    if lower.contains("teams") || lower.contains("adobe") {
        return 300;
    }
    if lower.contains("chrome")
        || lower.contains("brave")
        || lower.contains("edge")
        || lower.contains("discord")
        || lower.contains("slack")
    {
        return 200;
    }
    if lower.contains("spotify")
        || lower.contains("steam")
        || lower.contains("onedrive")
        || lower.contains("dropbox")
    {
        return 150;
    }
    if lower.contains("defender") || lower.contains("nvidia") || lower.contains("amd") {
        return 100;
    }
    if lower.contains("update") || lower.contains("updater") {
        return 40;
    }
    if lower.contains("bluetooth") || lower.contains("audio") {
        return 25;
    }

    // Default: medium
    60
}

/// Categorize a startup item based on its name.
fn categorize_item<'a>(name: &str, default: &'a str) -> &'a str {
    let lower = name.to_lowercase();

    if lower.contains("defender")
        || lower.contains("security")
        || lower.contains("audio")
        || lower.contains("bluetooth")
        || lower.contains("nvidia")
        || lower.contains("amd")
        || lower.contains("network")
    {
        return "system";
    }
    if lower.contains("update") || lower.contains("updater") {
        return "updater";
    }
    if lower.contains("onedrive")
        || lower.contains("dropbox")
        || lower.contains("google drive")
        || lower.contains("adobe")
        || lower.contains("cortana")
    {
        return "background";
    }

    default
}

/// Provide fallback items when platform enumeration fails or finds nothing.
fn fallback_items() -> Vec<StartupItem> {
    vec![
        StartupItem {
            id: 1,
            name: "System Security".into(),
            path: "(system)".into(),
            enabled: true,
            impact: "high".into(),
            category: "system".into(),
            memory_mb: 180,
            pagerank: 0.0,
        },
        StartupItem {
            id: 2,
            name: "Audio Service".into(),
            path: "(system)".into(),
            enabled: true,
            impact: "medium".into(),
            category: "system".into(),
            memory_mb: 45,
            pagerank: 0.0,
        },
        StartupItem {
            id: 3,
            name: "Network Manager".into(),
            path: "(system)".into(),
            enabled: true,
            impact: "high".into(),
            category: "system".into(),
            memory_mb: 65,
            pagerank: 0.0,
        },
    ]
}

// ── Module-level IPC entry point ──────────────────────────────────────

/// Free function called from `control_center.rs` IPC dispatch chain.
/// Enumerates startup items from the platform, delegates to
/// [`StartupManager::handle_ipc`], and returns the JSON response
/// (if the message type was recognised).
pub fn handle_ipc(msg_type: &str, payload: &serde_json::Value) -> Option<String> {
    let mut mgr = StartupManager::load();
    mgr.handle_ipc(msg_type, payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_impact() {
        assert_eq!(classify_impact(300), "high");
        assert_eq!(classify_impact(100), "medium");
        assert_eq!(classify_impact(20), "low");
    }

    #[test]
    fn test_compute_pagerank() {
        let system_high = StartupItem {
            id: 1,
            name: "test".into(),
            path: "".into(),
            enabled: true,
            impact: "high".into(),
            category: "system".into(),
            memory_mb: 200,
            pagerank: 0.0,
        };
        let pr = compute_pagerank(&system_high);
        assert!(pr > 0.8, "system+high should have high pagerank: {}", pr);

        let updater_low = StartupItem {
            id: 2,
            name: "test".into(),
            path: "".into(),
            enabled: true,
            impact: "low".into(),
            category: "updater".into(),
            memory_mb: 30,
            pagerank: 0.0,
        };
        let pr2 = compute_pagerank(&updater_low);
        assert!(pr2 < 0.4, "updater+low should have low pagerank: {}", pr2);
    }

    #[test]
    fn test_estimate_memory() {
        assert!(estimate_memory_from_name("Microsoft Teams") >= 200);
        // "Chrome Updater" matches "chrome" first, which is heavy (200MB)
        assert!(estimate_memory_from_name("Chrome Updater") >= 100);
        // A pure updater without a heavy-app keyword should be small
        assert!(estimate_memory_from_name("Java Updater") < 100);
        assert!(estimate_memory_from_name("Bluetooth Support") < 50);
    }

    #[test]
    fn test_categorize_item() {
        assert_eq!(categorize_item("Windows Defender", "user"), "system");
        assert_eq!(categorize_item("Chrome Updater", "user"), "updater");
        assert_eq!(categorize_item("OneDrive Sync", "user"), "background");
        assert_eq!(categorize_item("My App", "user"), "user");
    }

    #[test]
    fn test_boot_estimate() {
        let mgr = StartupManager {
            items: vec![
                StartupItem {
                    id: 1,
                    name: "System".into(),
                    path: "".into(),
                    enabled: true,
                    impact: "high".into(),
                    category: "system".into(),
                    memory_mb: 100,
                    pagerank: 0.9,
                },
                StartupItem {
                    id: 2,
                    name: "App".into(),
                    path: "".into(),
                    enabled: true,
                    impact: "medium".into(),
                    category: "user".into(),
                    memory_mb: 200,
                    pagerank: 0.3,
                },
            ],
            backup: None,
        };

        let est = mgr.compute_boot_estimate();
        assert!(est.current_secs > est.optimized_secs);
    }

    #[test]
    fn test_optimize_disables_non_system() {
        let mut mgr = StartupManager {
            items: vec![
                StartupItem {
                    id: 1,
                    name: "System".into(),
                    path: "".into(),
                    enabled: true,
                    impact: "high".into(),
                    category: "system".into(),
                    memory_mb: 100,
                    pagerank: 0.9,
                },
                StartupItem {
                    id: 2,
                    name: "Heavy App".into(),
                    path: "".into(),
                    enabled: true,
                    impact: "high".into(),
                    category: "user".into(),
                    memory_mb: 300,
                    pagerank: 0.3,
                },
            ],
            backup: None,
        };

        mgr.optimize_startup();

        // System item should still be enabled
        assert!(mgr.items[0].enabled);
        // User high-impact item should be disabled
        assert!(!mgr.items[1].enabled);
        // Backup should exist
        assert!(mgr.backup.is_some());
    }

    #[test]
    fn test_handle_ipc_unknown_returns_none() {
        let mut mgr = StartupManager {
            items: Vec::new(),
            backup: None,
        };
        assert!(mgr
            .handle_ipc("unknown_msg", &serde_json::json!({}))
            .is_none());
    }
}
