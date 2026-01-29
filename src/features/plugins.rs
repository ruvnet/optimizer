//! ADR-021 — WASM Plugin Marketplace
//!
//! Cross-platform plugin management with Wasmer execution on Windows.
//!
//! IPC messages handled:
//! - `get_installed_plugins`  → list installed plugins
//! - `get_marketplace_plugins` → static catalogue of available plugins
//! - `install_plugin`         → "install" a plugin (create manifest entry)
//! - `uninstall_plugin`       → remove a plugin
//! - `enable_plugin` / `disable_plugin` → toggle plugin state
//! - `configure_plugin`       → store per-plugin config
//! - `get_plugins`            → combined installed + runtime status
//! - `get_marketplace`        → alias for marketplace listing

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub enabled: bool,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub wasm_path: Option<String>,
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub signed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub category: String,
    pub rating: f64,
    pub downloads: u64,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub signed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginManifest {
    #[serde(default)]
    pub plugins: Vec<Plugin>,
}

// ---------------------------------------------------------------------------
// WASM runtime availability
// ---------------------------------------------------------------------------

/// Whether the WASM runtime (Wasmer) is available on this platform.
fn wasm_runtime_available() -> bool {
    cfg!(target_os = "windows")
}

fn wasm_runtime_version() -> &'static str {
    if cfg!(target_os = "windows") {
        "Wasmer 4.3"
    } else {
        "Unavailable (non-Windows)"
    }
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------

struct PluginState {
    manifest: PluginManifest,
}

impl PluginState {
    fn new() -> Self {
        Self {
            manifest: load_manifest(),
        }
    }
}

static STATE: once_cell::sync::Lazy<Mutex<PluginState>> =
    once_cell::sync::Lazy::new(|| Mutex::new(PluginState::new()));

mod once_cell {
    pub mod sync {
        pub struct Lazy<T> {
            inner: std::sync::OnceLock<T>,
            init: fn() -> T,
        }
        impl<T> Lazy<T> {
            pub const fn new(f: fn() -> T) -> Self {
                Self {
                    inner: std::sync::OnceLock::new(),
                    init: f,
                }
            }
        }
        impl<T> std::ops::Deref for Lazy<T> {
            type Target = T;
            fn deref(&self) -> &T {
                self.inner.get_or_init(self.init)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// IPC entry point
// ---------------------------------------------------------------------------

pub fn handle_ipc(msg_type: &str, payload: &serde_json::Value) -> Option<String> {
    match msg_type {
        "get_installed_plugins" | "get_plugins" => Some(get_installed_plugins()),
        "get_marketplace_plugins" | "get_marketplace" => Some(get_marketplace_plugins()),
        "install_plugin" => {
            let id = payload.get("plugin_id").and_then(|v| v.as_str()).unwrap_or("");
            Some(install_plugin(id))
        }
        "uninstall_plugin" => {
            let id = payload.get("plugin_id").and_then(|v| v.as_str()).unwrap_or("");
            Some(uninstall_plugin(id))
        }
        "enable_plugin" | "toggle_plugin" => {
            let id = payload.get("plugin_id").and_then(|v| v.as_str()).unwrap_or("");
            let enabled = payload.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
            Some(set_plugin_enabled(id, enabled))
        }
        "disable_plugin" => {
            let id = payload.get("plugin_id").and_then(|v| v.as_str()).unwrap_or("");
            Some(set_plugin_enabled(id, false))
        }
        "configure_plugin" | "get_plugin_config" => {
            let id = payload.get("plugin_id").and_then(|v| v.as_str()).unwrap_or("");
            let config = payload.get("config");
            Some(configure_plugin(id, config))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn get_installed_plugins() -> String {
    let plugins: Vec<Plugin> = STATE
        .lock()
        .ok()
        .map(|s| s.manifest.plugins.clone())
        .unwrap_or_default();

    let active_modules: usize = plugins.iter().filter(|p| p.enabled).count();
    let total_mem: f64 = if wasm_runtime_available() {
        plugins
            .iter()
            .filter(|p| p.enabled)
            .count() as f64
            * 3.5 // approximate per-module memory in MB
    } else {
        0.0
    };

    serde_json::json!({
        "installed": plugins,
        "wasm_runtime": {
            "version": wasm_runtime_version(),
            "modules": active_modules,
            "total_memory_mb": total_mem,
            "available": wasm_runtime_available(),
        }
    })
    .to_string()
}

fn get_marketplace_plugins() -> String {
    let catalogue = marketplace_catalogue();
    let featured: Vec<&MarketplaceEntry> = catalogue.iter().take(3).collect();

    serde_json::json!({
        "marketplace": catalogue,
        "featured": featured,
    })
    .to_string()
}

fn install_plugin(id: &str) -> String {
    if id.is_empty() {
        return serde_json::json!({"ok": false, "error": "missing plugin_id"}).to_string();
    }

    // Find in marketplace catalogue
    let catalogue = marketplace_catalogue();
    let entry = catalogue.iter().find(|e| e.id == id);

    let plugin = match entry {
        Some(e) => Plugin {
            id: e.id.clone(),
            name: e.name.clone(),
            version: e.version.clone(),
            description: e.description.clone(),
            author: e.author.clone(),
            enabled: true,
            category: e.category.clone(),
            wasm_path: None, // no actual .wasm yet
            config: HashMap::new(),
            capabilities: e.capabilities.clone(),
            verified: e.verified,
            signed: e.signed,
        },
        None => Plugin {
            id: id.to_string(),
            name: id.to_string(),
            version: "0.0.1".into(),
            description: "Unknown plugin".into(),
            author: "Unknown".into(),
            enabled: true,
            category: "System".into(),
            wasm_path: None,
            config: HashMap::new(),
            capabilities: Vec::new(),
            verified: false,
            signed: false,
        },
    };

    if let Ok(mut st) = STATE.lock() {
        // Avoid duplicates
        if !st.manifest.plugins.iter().any(|p| p.id == id) {
            st.manifest.plugins.push(plugin.clone());
            save_manifest(&st.manifest);

            // Ensure plugins directory exists
            let _ = super::ensure_config_dir(Some("plugins"));
        }
    }

    serde_json::json!({
        "ok": true,
        "plugin": plugin,
    })
    .to_string()
}

fn uninstall_plugin(id: &str) -> String {
    if let Ok(mut st) = STATE.lock() {
        let before = st.manifest.plugins.len();
        st.manifest.plugins.retain(|p| p.id != id);
        if st.manifest.plugins.len() < before {
            save_manifest(&st.manifest);

            // Remove wasm file if present
            let mut wasm_path = super::config_dir();
            wasm_path.push("plugins");
            wasm_path.push(format!("{}.wasm", id));
            let _ = std::fs::remove_file(&wasm_path);
        }
    }
    serde_json::json!({"ok": true, "plugin_id": id}).to_string()
}

fn set_plugin_enabled(id: &str, enabled: bool) -> String {
    if let Ok(mut st) = STATE.lock() {
        if let Some(p) = st.manifest.plugins.iter_mut().find(|p| p.id == id) {
            p.enabled = enabled;
            save_manifest(&st.manifest);
        }
    }
    serde_json::json!({"ok": true, "plugin_id": id, "enabled": enabled}).to_string()
}

fn configure_plugin(id: &str, config: Option<&serde_json::Value>) -> String {
    if let Ok(mut st) = STATE.lock() {
        let mut found = false;
        let mut result_config = HashMap::new();

        if let Some(p) = st.manifest.plugins.iter_mut().find(|p| p.id == id) {
            if let Some(cfg) = config {
                if let Some(obj) = cfg.as_object() {
                    for (k, v) in obj {
                        p.config.insert(k.clone(), v.clone());
                    }
                }
            }
            result_config = p.config.clone();
            found = true;
        }

        if found {
            let manifest_clone = st.manifest.clone();
            save_manifest(&manifest_clone);
            return serde_json::json!({
                "ok": true,
                "plugin_id": id,
                "config": result_config,
            })
            .to_string();
        }
    }
    serde_json::json!({"ok": false, "error": "plugin not found"}).to_string()
}

// ---------------------------------------------------------------------------
// WASM execution (Windows-only via Wasmer)
// ---------------------------------------------------------------------------

/// Execute a WASM plugin's entry function. Only available on Windows.
#[cfg(target_os = "windows")]
pub fn execute_plugin_wasm(wasm_bytes: &[u8], func_name: &str) -> Result<String, String> {
    use wasmer::{imports, Instance, Module, Store};
    use wasmer_compiler_singlepass::Singlepass;

    let compiler = Singlepass::default();
    let mut store = Store::new(compiler);
    let module = Module::new(&store, wasm_bytes).map_err(|e| format!("compile error: {}", e))?;
    let import_object = imports! {};
    let instance =
        Instance::new(&mut store, &module, &import_object).map_err(|e| format!("instantiation error: {}", e))?;

    let entry = instance
        .exports
        .get_function(func_name)
        .map_err(|e| format!("function '{}' not found: {}", func_name, e))?;

    let result = entry
        .call(&mut store, &[])
        .map_err(|e| format!("execution error: {}", e))?;

    Ok(format!("{:?}", result))
}

/// Stub for non-Windows platforms.
#[cfg(not(target_os = "windows"))]
pub fn execute_plugin_wasm(_wasm_bytes: &[u8], _func_name: &str) -> Result<String, String> {
    Err("WASM execution is only available on Windows (Wasmer runtime)".into())
}

// ---------------------------------------------------------------------------
// Marketplace catalogue (hardcoded)
// ---------------------------------------------------------------------------

fn marketplace_catalogue() -> Vec<MarketplaceEntry> {
    vec![
        MarketplaceEntry {
            id: "memory-guardian".into(),
            name: "Memory Guardian".into(),
            version: "1.0.0".into(),
            description: "Auto memory cleanup based on usage patterns and time-of-day heuristics."
                .into(),
            author: "RuVector Team".into(),
            category: "Automation".into(),
            rating: 4.8,
            downloads: 12400,
            capabilities: vec!["memory_read".into(), "memory_write".into()],
            verified: true,
            signed: true,
        },
        MarketplaceEntry {
            id: "process-limiter".into(),
            name: "Process Limiter".into(),
            version: "2.1.0".into(),
            description: "Set CPU and memory limits for specific processes to prevent resource hogging."
                .into(),
            author: "SysOpt Inc".into(),
            category: "System".into(),
            rating: 4.6,
            downloads: 8900,
            capabilities: vec!["process_control".into(), "memory_read".into()],
            verified: true,
            signed: true,
        },
        MarketplaceEntry {
            id: "network-optimizer".into(),
            name: "Network Buffer Optimizer".into(),
            version: "1.3.0".into(),
            description: "Tunes network buffer sizes and socket settings for lower latency."
                .into(),
            author: "NetTune Labs".into(),
            category: "System".into(),
            rating: 4.4,
            downloads: 5600,
            capabilities: vec!["network_config".into()],
            verified: true,
            signed: true,
        },
        MarketplaceEntry {
            id: "disk-cleaner".into(),
            name: "Disk Cleaner".into(),
            version: "3.0.2".into(),
            description:
                "Automated temp file, browser cache, and build artifact cleanup on schedule."
                    .into(),
            author: "CleanSys".into(),
            category: "Automation".into(),
            rating: 4.7,
            downloads: 15600,
            capabilities: vec!["disk_read".into(), "disk_write".into()],
            verified: true,
            signed: true,
        },
        MarketplaceEntry {
            id: "startup-tuner".into(),
            name: "Startup Tuner".into(),
            version: "1.5.0".into(),
            description: "Additional startup optimization rules beyond the built-in scheduler."
                .into(),
            author: "SpeedTools".into(),
            category: "System".into(),
            rating: 4.5,
            downloads: 7800,
            capabilities: vec!["startup_config".into(), "process_control".into()],
            verified: true,
            signed: true,
        },
        MarketplaceEntry {
            id: "security-scan".into(),
            name: "Memory Security Scanner".into(),
            version: "2.4.1".into(),
            description: "Scans for memory-resident malware and suspicious access patterns."
                .into(),
            author: "SecureOps".into(),
            category: "Security".into(),
            rating: 4.9,
            downloads: 9200,
            capabilities: vec!["memory_read".into(), "security_scan".into()],
            verified: true,
            signed: true,
        },
        MarketplaceEntry {
            id: "game-boost".into(),
            name: "Game Memory Booster".into(),
            version: "2.0.1".into(),
            description:
                "Optimizes memory for gaming by pre-loading assets and adjusting priorities."
                    .into(),
            author: "GameDev Labs".into(),
            category: "Gaming".into(),
            rating: 4.3,
            downloads: 11200,
            capabilities: vec!["memory_write".into(), "priority_boost".into()],
            verified: true,
            signed: true,
        },
        MarketplaceEntry {
            id: "thermal-ai".into(),
            name: "Thermal AI Optimizer".into(),
            version: "0.8.0".into(),
            description: "ML-based thermal throttling prediction and prevention.".into(),
            author: "ThermoTech".into(),
            category: "Thermal".into(),
            rating: 4.1,
            downloads: 1200,
            capabilities: vec!["thermal_read".into(), "process_control".into()],
            verified: false,
            signed: false,
        },
    ]
}

// ---------------------------------------------------------------------------
// Manifest persistence
// ---------------------------------------------------------------------------

fn manifest_path() -> PathBuf {
    let mut p = super::config_dir();
    p.push("plugins.toml");
    p
}

fn load_manifest() -> PluginManifest {
    let path = manifest_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => PluginManifest::default(),
    }
}

fn save_manifest(manifest: &PluginManifest) {
    if let Err(e) = super::ensure_config_dir(None) {
        tracing::warn!("Failed to create config dir: {}", e);
        return;
    }
    match toml::to_string_pretty(manifest) {
        Ok(content) => {
            if let Err(e) = std::fs::write(manifest_path(), content) {
                tracing::warn!("Failed to write plugin manifest: {}", e);
            }
        }
        Err(e) => {
            tracing::warn!("Failed to serialize plugin manifest: {}", e);
        }
    }
}
