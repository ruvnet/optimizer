//! ADR-023: Bloatware & Telemetry Silencer
//!
//! Cross-platform bloatware database, scan, removal, telemetry level control,
//! and undo/restore functionality.  All state is persisted to
//! `<config_dir>/bloatware_history.json`.
//!
//! IPC messages handled:
//! - `scan_bloatware` – scan running processes against the known database
//! - `remove_bloatware` – terminate matching processes, record for undo
//! - `set_telemetry_level` – store preferred telemetry level + platform advice
//! - `restore_bloatware` – (informational) retrieve removal history
//! - `get_bloatware_items` – return cached scan results
//! - `get_undo_history` – return undo history
//! - `undo_bloatware` – remove an entry from undo history (UI-driven restore)

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Mutex;
use sysinfo::System;

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloatwareItem {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub category: BloatwareCategory,
    pub process_names: Vec<String>,
    pub impact: Impact,
    pub removable: bool,
    pub description: String,
    /// Runtime: whether the process was found running during the last scan.
    #[serde(default)]
    pub status: ItemStatus,
    /// RAM usage in MB observed at scan time.
    #[serde(default)]
    pub ram_mb: f64,
    /// CPU usage (%) observed at scan time.
    #[serde(default)]
    pub cpu_pct: f32,
    /// Safety rating for the UI.
    #[serde(default)]
    pub safety: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BloatwareCategory {
    Bloatware,
    Telemetry,
    Preinstalled,
    Background,
    Startup,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Impact {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ItemStatus {
    Active,
    Disabled,
    Removed,
}

impl Default for ItemStatus {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub name: String,
    pub action: String,
    pub prev_status: String,
    pub category: String,
    pub time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryAdvice {
    pub level: String,
    pub platform: String,
    pub suggestions: Vec<String>,
}

// ── Persisted state ────────────────────────────────────────────────

#[derive(Debug, Default, Serialize, Deserialize)]
struct BloatwareState {
    scan_results: Vec<BloatwareItem>,
    undo_history: Vec<HistoryEntry>,
    telemetry_level: String,
}

static STATE: Mutex<Option<BloatwareState>> = Mutex::new(None);

fn with_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut BloatwareState) -> R,
{
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(load_state());
    }
    f(guard.as_mut().expect("state initialised"))
}

fn history_path() -> std::path::PathBuf {
    let mut p = super::config_dir();
    p.push("bloatware_history.json");
    p
}

fn load_state() -> BloatwareState {
    let path = history_path();
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(data) => match serde_json::from_str(&data) {
                Ok(s) => return s,
                Err(e) => tracing::warn!("bloatware: failed to parse state: {}", e),
            },
            Err(e) => tracing::warn!("bloatware: failed to read state: {}", e),
        }
    }
    BloatwareState {
        telemetry_level: "minimal".into(),
        ..Default::default()
    }
}

fn save_state(state: &BloatwareState) {
    if let Err(e) = super::ensure_config_dir(None) {
        tracing::warn!("bloatware: cannot create config dir: {}", e);
        return;
    }
    match serde_json::to_string_pretty(state) {
        Ok(json) => {
            if let Err(e) = std::fs::write(history_path(), json) {
                tracing::warn!("bloatware: failed to write state: {}", e);
            }
        }
        Err(e) => tracing::warn!("bloatware: failed to serialise state: {}", e),
    }
}

// ── Built-in bloatware database ────────────────────────────────────

fn builtin_database() -> Vec<BloatwareItem> {
    let mut db = Vec::new();

    // ── Windows ────────────────────────────────────────────────────
    #[cfg(target_os = "windows")]
    {
        let win = vec![
            ("ms-1", "Cortana", "Microsoft", BloatwareCategory::Bloatware, &["cortana.exe", "searchui.exe"][..], Impact::Medium, true, "Voice assistant – rarely used on desktop.", "safe"),
            ("ms-2", "Xbox Game Bar", "Microsoft", BloatwareCategory::Bloatware, &["gamebar.exe", "gamebarpresencewriter.exe"][..], Impact::Low, true, "In-game overlay. Disable if you don't use it.", "safe"),
            ("ms-3", "Microsoft News", "Microsoft", BloatwareCategory::Bloatware, &["microsoftnews.exe"][..], Impact::Low, true, "News feed widget.", "safe"),
            ("ms-4", "Feedback Hub", "Microsoft", BloatwareCategory::Bloatware, &["feedbackhub.exe"][..], Impact::Low, true, "Windows insider feedback tool.", "safe"),
            ("ms-5", "Tips", "Microsoft", BloatwareCategory::Bloatware, &["tips.exe"][..], Impact::Low, true, "Windows tips and suggestions.", "safe"),
            ("ms-6", "Get Help", "Microsoft", BloatwareCategory::Bloatware, &["gethelp.exe"][..], Impact::Low, true, "Online help application.", "safe"),
            ("ms-7", "OneDrive", "Microsoft", BloatwareCategory::Preinstalled, &["onedrive.exe"][..], Impact::Medium, true, "Cloud sync – keep if you use it.", "moderate"),
            ("ms-8", "Microsoft Teams (Personal)", "Microsoft", BloatwareCategory::Preinstalled, &["ms-teams.exe", "teams.exe"][..], Impact::High, true, "Chat application.", "safe"),
            ("ms-9", "Clipchamp", "Microsoft", BloatwareCategory::Bloatware, &["clipchamp.exe"][..], Impact::Low, true, "Video editor.", "safe"),
            ("tel-1", "DiagTrack Service", "Microsoft", BloatwareCategory::Telemetry, &["diagtrack.exe", "utcsvc.exe"][..], Impact::Medium, false, "Diagnostics tracking service.", "moderate"),
            ("tel-2", "Connected User Experiences", "Microsoft", BloatwareCategory::Telemetry, &["sihclient.exe"][..], Impact::Medium, false, "Telemetry data collection.", "moderate"),
            ("tel-3", "Windows Error Reporting", "Microsoft", BloatwareCategory::Telemetry, &["werfault.exe", "wermgr.exe"][..], Impact::Low, false, "Crash data reporting.", "caution"),
            ("tel-4", "Customer Experience Program", "Microsoft", BloatwareCategory::Telemetry, &["ceip.exe"][..], Impact::Low, true, "Anonymous usage data.", "safe"),
            ("bg-1", "Windows Search Indexer", "Microsoft", BloatwareCategory::Background, &["searchindexer.exe"][..], Impact::High, false, "Full-text search index. High I/O usage.", "caution"),
            ("bg-2", "SysMain (Superfetch)", "Microsoft", BloatwareCategory::Background, &["sysmain.exe"][..], Impact::High, false, "Memory pre-caching service.", "expert"),
            ("bg-3", "Windows Update Medic", "Microsoft", BloatwareCategory::Background, &["waasmedicsvc.exe"][..], Impact::Low, false, "Keeps Windows Update running.", "expert"),
            ("bg-4", "Print Spooler", "Microsoft", BloatwareCategory::Background, &["spoolsv.exe"][..], Impact::Low, true, "Disable if you don't print.", "moderate"),
        ];
        for (id, name, pub_, cat, procs, impact, removable, desc, safety) in win {
            db.push(BloatwareItem {
                id: id.into(),
                name: name.into(),
                publisher: pub_.into(),
                category: cat,
                process_names: procs.iter().map(|s| s.to_string()).collect(),
                impact,
                removable,
                description: desc.into(),
                status: ItemStatus::Active,
                ram_mb: 0.0,
                cpu_pct: 0.0,
                safety: safety.into(),
            });
        }
    }

    // ── macOS ──────────────────────────────────────────────────────
    #[cfg(target_os = "macos")]
    {
        let mac = vec![
            ("mac-1", "Siri", "Apple", BloatwareCategory::Bloatware, &["siri", "sirikernelservice"][..], Impact::Medium, true, "Voice assistant.", "safe"),
            ("mac-2", "News", "Apple", BloatwareCategory::Bloatware, &["news"][..], Impact::Low, true, "Apple News app.", "safe"),
            ("mac-3", "Stocks", "Apple", BloatwareCategory::Bloatware, &["stocks"][..], Impact::Low, true, "Stock ticker widget.", "safe"),
            ("mac-4", "Home", "Apple", BloatwareCategory::Bloatware, &["home"][..], Impact::Low, true, "HomeKit controller.", "safe"),
            ("mac-5", "Tips", "Apple", BloatwareCategory::Bloatware, &["tips"][..], Impact::Low, true, "macOS tips.", "safe"),
            ("mac-6", "Spotlight Indexer", "Apple", BloatwareCategory::Background, &["mds", "mds_stores", "mdworker"][..], Impact::High, false, "Full-text search index.", "caution"),
        ];
        for (id, name, pub_, cat, procs, impact, removable, desc, safety) in mac {
            db.push(BloatwareItem {
                id: id.into(),
                name: name.into(),
                publisher: pub_.into(),
                category: cat,
                process_names: procs.iter().map(|s| s.to_string()).collect(),
                impact,
                removable,
                description: desc.into(),
                status: ItemStatus::Active,
                ram_mb: 0.0,
                cpu_pct: 0.0,
                safety: safety.into(),
            });
        }
    }

    // ── Linux ──────────────────────────────────────────────────────
    #[cfg(target_os = "linux")]
    {
        let linux = vec![
            ("lin-1", "Snap Store", "Canonical", BloatwareCategory::Bloatware, &["snap-store"][..], Impact::Medium, true, "Snap package store.", "safe"),
            ("lin-2", "GNOME Software Auto-Updates", "GNOME", BloatwareCategory::Background, &["gnome-software"][..], Impact::Medium, true, "Background update checker.", "moderate"),
            ("lin-3", "Tracker Miner", "GNOME", BloatwareCategory::Background, &["tracker-miner-fs", "tracker-store"][..], Impact::High, true, "File index service.", "caution"),
        ];
        for (id, name, pub_, cat, procs, impact, removable, desc, safety) in linux {
            db.push(BloatwareItem {
                id: id.into(),
                name: name.into(),
                publisher: pub_.into(),
                category: cat,
                process_names: procs.iter().map(|s| s.to_string()).collect(),
                impact,
                removable,
                description: desc.into(),
                status: ItemStatus::Active,
                ram_mb: 0.0,
                cpu_pct: 0.0,
                safety: safety.into(),
            });
        }
    }

    db
}

// ── Core operations ────────────────────────────────────────────────

fn scan_bloatware() -> String {
    let db = builtin_database();
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut found: Vec<BloatwareItem> = Vec::new();

    for mut item in db {
        let mut matched = false;
        let mut total_mem: u64 = 0;
        let mut total_cpu: f32 = 0.0;

        for (_pid, proc) in sys.processes() {
            let pname = proc.name().to_string_lossy().to_lowercase();
            for known in &item.process_names {
                if pname.contains(known) {
                    matched = true;
                    total_mem += proc.memory();
                    total_cpu += proc.cpu_usage();
                    break;
                }
            }
        }

        if matched {
            item.status = ItemStatus::Active;
            item.ram_mb = total_mem as f64 / 1024.0 / 1024.0;
            item.cpu_pct = total_cpu;
            found.push(item);
        }
    }

    // Categorise into the groups the UI expects
    let categorise = |cat: BloatwareCategory| -> Vec<&BloatwareItem> {
        found.iter().filter(|i| i.category == cat).collect()
    };

    let items = json!({
        "oem": categorise(BloatwareCategory::Preinstalled),
        "microsoft": categorise(BloatwareCategory::Bloatware),
        "telemetry": categorise(BloatwareCategory::Telemetry),
        "background": categorise(BloatwareCategory::Background),
        "startup": categorise(BloatwareCategory::Startup),
    });

    // Cache results
    with_state(|s| {
        s.scan_results = found;
        save_state(s);
    });

    json!({ "items": items }).to_string()
}

fn remove_bloatware(payload: &serde_json::Value) -> String {
    let target_id = payload["id"].as_str().unwrap_or("");
    let action = payload["action"].as_str().unwrap_or("remove");

    let mut killed: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    with_state(|state| {
        let targets: Vec<&BloatwareItem> = if !target_id.is_empty() {
            state.scan_results.iter().filter(|i| i.id == target_id).collect()
        } else if let Some(category) = payload["category"].as_str() {
            state.scan_results.iter().filter(|i| {
                let cat_str = match i.category {
                    BloatwareCategory::Preinstalled => "oem",
                    BloatwareCategory::Bloatware => "microsoft",
                    BloatwareCategory::Telemetry => "telemetry",
                    BloatwareCategory::Background => "background",
                    BloatwareCategory::Startup => "startup",
                };
                cat_str == category
            }).collect()
        } else {
            Vec::new()
        };

        if targets.is_empty() {
            errors.push("No matching bloatware item found".into());
            return;
        }

        // Collect process names to kill
        let proc_names: Vec<String> = targets
            .iter()
            .flat_map(|t| t.process_names.clone())
            .collect();

        // Record history entries
        let now = chrono::Local::now().format("%H:%M:%S").to_string();
        for t in &targets {
            state.undo_history.push(HistoryEntry {
                id: t.id.clone(),
                name: t.name.clone(),
                action: if action == "remove" { "Removed".into() } else { "Disabled".into() },
                prev_status: format!("{:?}", t.status).to_lowercase(),
                category: format!("{:?}", t.category).to_lowercase(),
                time: now.clone(),
            });
        }

        // Attempt to terminate matching processes
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        for (pid, proc) in sys.processes() {
            let pname = proc.name().to_string_lossy().to_lowercase();
            if proc_names.iter().any(|known| pname.contains(known)) {
                if proc.kill() {
                    killed.push(format!("{} (PID {})", pname, pid.as_u32()));
                    tracing::info!("bloatware: killed {} (PID {})", pname, pid.as_u32());
                } else {
                    errors.push(format!("Failed to kill {} (PID {})", pname, pid.as_u32()));
                    tracing::warn!("bloatware: failed to kill {} (PID {})", pname, pid.as_u32());
                }
            }
        }

        // Mark items as removed in cached results
        let target_ids: Vec<String> = targets.iter().map(|t| t.id.clone()).collect();
        for item in &mut state.scan_results {
            if target_ids.iter().any(|tid| *tid == item.id) {
                item.status = ItemStatus::Removed;
            }
        }

        save_state(state);
    });

    json!({
        "success": errors.is_empty(),
        "killed": killed,
        "errors": errors,
    })
    .to_string()
}

fn set_telemetry_level(payload: &serde_json::Value) -> String {
    let level = payload["level"]
        .as_str()
        .unwrap_or("minimal")
        .to_string();

    let suggestions = telemetry_suggestions(&level);

    with_state(|s| {
        s.telemetry_level = level.clone();
        save_state(s);
    });

    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };

    json!(TelemetryAdvice {
        level,
        platform: platform.into(),
        suggestions,
    })
    .to_string()
}

fn telemetry_suggestions(level: &str) -> Vec<String> {
    let mut out = Vec::new();

    #[cfg(target_os = "windows")]
    {
        match level {
            "minimal" => {
                out.push("Disable non-essential diagnostics: Settings > Privacy > Diagnostics".into());
                out.push("Registry: HKLM\\SOFTWARE\\Policies\\Microsoft\\Windows\\DataCollection -> AllowTelemetry = 1".into());
            }
            "aggressive" => {
                out.push("Disable all optional telemetry endpoints.".into());
                out.push("Disable Cortana: HKLM\\SOFTWARE\\Policies\\Microsoft\\Windows\\Windows Search -> AllowCortana = 0".into());
                out.push("Disable ad ID: HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AdvertisingInfo -> Enabled = 0".into());
                out.push("Registry: AllowTelemetry = 0".into());
            }
            "paranoid" => {
                out.push("Block ALL outbound telemetry including error reporting.".into());
                out.push("Disable Connected User Experiences service (DiagTrack).".into());
                out.push("Block telemetry hosts via Windows Firewall or hosts file.".into());
                out.push("WARNING: May break some Windows Update features.".into());
            }
            _ => {}
        }
    }

    #[cfg(target_os = "macos")]
    {
        match level {
            "minimal" => {
                out.push("Disable Analytics: System Settings > Privacy > Analytics & Improvements".into());
            }
            "aggressive" | "paranoid" => {
                out.push("Disable all analytics sharing in System Settings.".into());
                out.push("Disable Siri data sharing.".into());
                out.push("Use Little Snitch or LuLu to block telemetry endpoints.".into());
            }
            _ => {}
        }
    }

    #[cfg(target_os = "linux")]
    {
        match level {
            "minimal" => {
                out.push("Disable apt/snap telemetry: sudo apt remove ubuntu-report".into());
            }
            "aggressive" | "paranoid" => {
                out.push("Remove apport (crash reporting): sudo apt remove apport".into());
                out.push("Disable whoopsie: sudo systemctl disable whoopsie".into());
                out.push("Block telemetry hosts in /etc/hosts.".into());
            }
            _ => {}
        }
    }

    out
}

fn get_cached_items() -> String {
    with_state(|s| {
        if s.scan_results.is_empty() {
            return json!({ "items": {} }).to_string();
        }

        let categorise = |cat: BloatwareCategory| -> Vec<&BloatwareItem> {
            s.scan_results.iter().filter(|i| i.category == cat).collect()
        };
        json!({
            "items": {
                "oem": categorise(BloatwareCategory::Preinstalled),
                "microsoft": categorise(BloatwareCategory::Bloatware),
                "telemetry": categorise(BloatwareCategory::Telemetry),
                "background": categorise(BloatwareCategory::Background),
                "startup": categorise(BloatwareCategory::Startup),
            },
            "telemetry_level": s.telemetry_level,
        })
        .to_string()
    })
}

fn get_undo_history() -> String {
    with_state(|s| {
        json!({ "undo_history": s.undo_history }).to_string()
    })
}

fn undo_item(payload: &serde_json::Value) -> String {
    let id = payload["id"].as_str().unwrap_or("");
    with_state(|s| {
        s.undo_history.retain(|e| e.id != id);
        // Mark item back to active if it was removed
        for item in &mut s.scan_results {
            if item.id == id {
                item.status = ItemStatus::Active;
            }
        }
        save_state(s);
        json!({ "success": true }).to_string()
    })
}

// ── IPC entry point ────────────────────────────────────────────────

pub fn handle_ipc(msg_type: &str, payload: &serde_json::Value) -> Option<String> {
    match msg_type {
        "scan_bloatware" => Some(scan_bloatware()),
        "remove_bloatware" => Some(remove_bloatware(payload)),
        "set_telemetry_level" => Some(set_telemetry_level(payload)),
        "restore_bloatware" | "get_bloatware_items" => Some(get_cached_items()),
        "get_undo_history" => Some(get_undo_history()),
        "undo_bloatware" => Some(undo_item(payload)),
        _ => None,
    }
}
