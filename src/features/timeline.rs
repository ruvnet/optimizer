//! ADR-024: Time-Travel System State
//!
//! Captures system-state checkpoints (memory, CPU, processes, disk) and
//! allows the user to compare snapshots, diagnose degradation, and
//! optionally "rollback" by killing new processes.
//!
//! Checkpoints are stored as individual JSON files under
//! `<config_dir>/timeline/`.  A maximum of 50 checkpoints are kept;
//! the oldest are pruned automatically.
//!
//! IPC messages handled:
//! - `get_checkpoints` / `get_snapshots` – list stored checkpoints
//! - `create_checkpoint` – snapshot current system state
//! - `rollback_checkpoint` / `rollback_to` – show diff & kill new procs
//! - `get_diff` / `compare_snapshots` – diff two checkpoints
//! - `get_diagnosis` – analyse current vs best checkpoint

use serde::{Deserialize, Serialize};
use serde_json::json;
use sysinfo::System;

const MAX_CHECKPOINTS: usize = 50;
const TOP_PROCESSES: usize = 20;

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub name: String,
    pub timestamp: i64,
    pub state: SystemState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemState {
    pub memory_usage_mb: f64,
    pub memory_total_mb: f64,
    pub cpu_load: f32,
    pub process_count: usize,
    pub top_processes: Vec<ProcessSnapshot>,
    pub disk_usage_pct: f64,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSnapshot {
    pub name: String,
    pub mem_mb: f64,
    pub cpu_pct: f32,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    pub processes_added: Vec<ProcessSnapshot>,
    pub processes_removed: Vec<ProcessSnapshot>,
    pub memory_change_mb: f64,
    pub cpu_change: f32,
    pub process_count_change: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisItem {
    pub cause: String,
    pub impact: u32,
    pub category: String,
}

// ── Helpers ────────────────────────────────────────────────────────

fn timeline_dir() -> std::path::PathBuf {
    let mut p = super::config_dir();
    p.push("timeline");
    p
}

fn ensure_timeline_dir() -> std::io::Result<std::path::PathBuf> {
    super::ensure_config_dir(Some("timeline"))
}

fn checkpoint_path(id: &str) -> std::path::PathBuf {
    let mut p = timeline_dir();
    p.push(format!("{}.json", id));
    p
}

fn generate_id() -> String {
    let now = chrono::Utc::now();
    now.format("cp-%Y%m%d-%H%M%S-%3f").to_string()
}

/// Capture a snapshot of the current system state.
fn capture_state() -> SystemState {
    let mut sys = System::new();
    sys.refresh_all();

    let memory_total_mb = sys.total_memory() as f64 / 1024.0 / 1024.0;
    let memory_used_mb = sys.used_memory() as f64 / 1024.0 / 1024.0;

    // Global CPU usage – sysinfo requires two refresh cycles for accuracy,
    // but a single snapshot is good enough for checkpoint comparison.
    let cpu_load: f32 = sys.global_cpu_usage();

    let process_count = sys.processes().len();

    // Collect and sort by memory descending
    let mut procs: Vec<ProcessSnapshot> = sys
        .processes()
        .iter()
        .map(|(pid, p)| ProcessSnapshot {
            name: p.name().to_string_lossy().to_string(),
            mem_mb: p.memory() as f64 / 1024.0 / 1024.0,
            cpu_pct: p.cpu_usage(),
            pid: pid.as_u32(),
        })
        .collect();
    procs.sort_by(|a, b| b.mem_mb.partial_cmp(&a.mem_mb).unwrap_or(std::cmp::Ordering::Equal));
    procs.truncate(TOP_PROCESSES);

    // Disk usage (first available disk)
    let disk_pct = {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        disks
            .list()
            .first()
            .map(|d| {
                let total = d.total_space() as f64;
                let avail = d.available_space() as f64;
                if total > 0.0 {
                    ((total - avail) / total) * 100.0
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0)
    };

    SystemState {
        memory_usage_mb: memory_used_mb,
        memory_total_mb,
        cpu_load,
        process_count,
        top_processes: procs,
        disk_usage_pct: disk_pct,
        uptime_secs: sysinfo::System::uptime(),
    }
}

/// Compute a 0-100 "health score" from a system state.
fn health_score(state: &SystemState) -> u32 {
    let mem_ratio = if state.memory_total_mb > 0.0 {
        state.memory_usage_mb / state.memory_total_mb
    } else {
        0.5
    };
    let mem_penalty = (mem_ratio * 40.0) as u32;
    let cpu_penalty = (state.cpu_load as u32).min(30);
    let disk_penalty = ((state.disk_usage_pct / 100.0) * 15.0) as u32;
    let proc_penalty = (state.process_count as u32 / 50).min(15);
    100u32.saturating_sub(mem_penalty + cpu_penalty + disk_penalty + proc_penalty)
}

// ── Checkpoint CRUD ────────────────────────────────────────────────

fn list_checkpoints() -> Vec<Checkpoint> {
    let dir = timeline_dir();
    let mut cps: Vec<Checkpoint> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                match std::fs::read_to_string(&path) {
                    Ok(data) => match serde_json::from_str::<Checkpoint>(&data) {
                        Ok(cp) => cps.push(cp),
                        Err(e) => tracing::warn!("timeline: bad checkpoint {:?}: {}", path, e),
                    },
                    Err(e) => tracing::warn!("timeline: cannot read {:?}: {}", path, e),
                }
            }
        }
    }

    // Sort most recent first
    cps.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    cps
}

fn save_checkpoint(cp: &Checkpoint) -> Result<(), String> {
    ensure_timeline_dir().map_err(|e| format!("Cannot create timeline dir: {}", e))?;

    let json = serde_json::to_string_pretty(cp)
        .map_err(|e| format!("Serialise error: {}", e))?;
    std::fs::write(checkpoint_path(&cp.id), json)
        .map_err(|e| format!("Write error: {}", e))?;

    // Prune if over limit
    let all = list_checkpoints();
    if all.len() > MAX_CHECKPOINTS {
        for old in &all[MAX_CHECKPOINTS..] {
            let _ = std::fs::remove_file(checkpoint_path(&old.id));
            tracing::debug!("timeline: pruned old checkpoint {}", old.id);
        }
    }

    Ok(())
}

fn find_checkpoint(id: &str) -> Option<Checkpoint> {
    let path = checkpoint_path(id);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|data| serde_json::from_str(&data).ok())
}

// ── Diff & diagnosis ───────────────────────────────────────────────

fn diff_states(before: &SystemState, after: &SystemState) -> StateDiff {
    // Processes present in "after" but not "before"
    let before_names: std::collections::HashSet<&str> = before
        .top_processes
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    let after_names: std::collections::HashSet<&str> = after
        .top_processes
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    let added: Vec<ProcessSnapshot> = after
        .top_processes
        .iter()
        .filter(|p| !before_names.contains(p.name.as_str()))
        .cloned()
        .collect();

    let removed: Vec<ProcessSnapshot> = before
        .top_processes
        .iter()
        .filter(|p| !after_names.contains(p.name.as_str()))
        .cloned()
        .collect();

    StateDiff {
        processes_added: added,
        processes_removed: removed,
        memory_change_mb: after.memory_usage_mb - before.memory_usage_mb,
        cpu_change: after.cpu_load - before.cpu_load,
        process_count_change: after.process_count as i64 - before.process_count as i64,
    }
}

fn diagnose(current: &SystemState, best: &SystemState) -> Vec<DiagnosisItem> {
    let mut items = Vec::new();

    // Memory growth
    let mem_growth = current.memory_usage_mb - best.memory_usage_mb;
    if mem_growth > 100.0 {
        items.push(DiagnosisItem {
            cause: format!(
                "Memory usage grew by {:.0} MB since best checkpoint",
                mem_growth
            ),
            impact: (mem_growth / 10.0).min(100.0) as u32,
            category: "Memory".into(),
        });
    }

    // New heavy processes
    let best_names: std::collections::HashSet<&str> = best
        .top_processes
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    for proc in &current.top_processes {
        if !best_names.contains(proc.name.as_str()) && proc.mem_mb > 100.0 {
            items.push(DiagnosisItem {
                cause: format!(
                    "{} consuming {:.0} MB (not present at best checkpoint)",
                    proc.name, proc.mem_mb
                ),
                impact: (proc.mem_mb / 5.0).min(100.0) as u32,
                category: "Process".into(),
            });
        }
    }

    // CPU load increase
    let cpu_delta = current.cpu_load - best.cpu_load;
    if cpu_delta > 10.0 {
        items.push(DiagnosisItem {
            cause: format!("CPU load increased by {:.1}%", cpu_delta),
            impact: (cpu_delta as u32).min(100),
            category: "CPU".into(),
        });
    }

    // Process count increase
    let proc_delta = current.process_count as i64 - best.process_count as i64;
    if proc_delta > 20 {
        items.push(DiagnosisItem {
            cause: format!("{} new processes since best checkpoint", proc_delta),
            impact: (proc_delta as u32).min(80),
            category: "Process".into(),
        });
    }

    items.sort_by(|a, b| b.impact.cmp(&a.impact));
    items
}

// ── IPC handlers ───────────────────────────────────────────────────

fn ipc_get_checkpoints() -> String {
    let cps = list_checkpoints();
    let current_state = capture_state();
    let current_score = health_score(&current_state);
    let best_score = cps
        .iter()
        .map(|cp| health_score(&cp.state))
        .max()
        .unwrap_or(current_score);

    // Build health history from checkpoints for the chart
    let mut history: Vec<serde_json::Value> = cps
        .iter()
        .rev()
        .map(|cp| {
            json!({
                "time": cp.timestamp * 1000, // JS expects milliseconds
                "score": health_score(&cp.state),
            })
        })
        .collect();
    // Append current as latest point
    history.push(json!({
        "time": chrono::Utc::now().timestamp_millis(),
        "score": current_score,
    }));

    let cp_list: Vec<serde_json::Value> = cps
        .iter()
        .map(|cp| {
            json!({
                "id": cp.id,
                "name": cp.name,
                "time": cp.timestamp * 1000,
                "score": health_score(&cp.state),
            })
        })
        .collect();

    json!({
        "checkpoints": cp_list,
        "health_history": history,
        "current_score": current_score,
        "best_today": best_score,
    })
    .to_string()
}

fn ipc_create_checkpoint(payload: &serde_json::Value) -> String {
    let name = payload["name"]
        .as_str()
        .unwrap_or("Untitled Checkpoint")
        .to_string();

    let state = capture_state();
    let score = health_score(&state);
    let id = generate_id();

    let cp = Checkpoint {
        id: id.clone(),
        name: name.clone(),
        timestamp: chrono::Utc::now().timestamp(),
        state,
    };

    match save_checkpoint(&cp) {
        Ok(()) => {
            tracing::info!("timeline: created checkpoint '{}' (score {})", name, score);
            json!({
                "success": true,
                "id": id,
                "name": name,
                "score": score,
            })
            .to_string()
        }
        Err(e) => {
            tracing::error!("timeline: failed to save checkpoint: {}", e);
            json!({ "success": false, "error": e }).to_string()
        }
    }
}

fn ipc_rollback(payload: &serde_json::Value) -> String {
    let cp_id = payload["checkpoint"]
        .as_str()
        .or_else(|| payload["id"].as_str())
        .unwrap_or("");

    let cp = match find_checkpoint(cp_id) {
        Some(c) => c,
        None => {
            return json!({ "success": false, "error": "Checkpoint not found" }).to_string();
        }
    };

    let current = capture_state();
    let diff = diff_states(&cp.state, &current);

    // Kill processes that were not running at checkpoint time
    let mut killed: Vec<String> = Vec::new();
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let cp_names: std::collections::HashSet<String> = cp
        .state
        .top_processes
        .iter()
        .map(|p| p.name.to_lowercase())
        .collect();

    for added in &diff.processes_added {
        // Try to find and kill the process
        for (pid, proc) in sys.processes() {
            let pname = proc.name().to_string_lossy().to_lowercase();
            if pname.contains(&added.name.to_lowercase())
                && !cp_names.contains(&pname)
            {
                if proc.kill() {
                    killed.push(format!("{} (PID {})", pname, pid.as_u32()));
                    tracing::info!("timeline: rollback killed {} (PID {})", pname, pid.as_u32());
                }
            }
        }
    }

    let recommendations: Vec<String> = if diff.memory_change_mb > 500.0 {
        vec![
            "Consider running a memory optimisation pass.".into(),
            format!(
                "Memory grew by {:.0} MB since checkpoint.",
                diff.memory_change_mb
            ),
        ]
    } else {
        Vec::new()
    };

    json!({
        "success": true,
        "killed": killed,
        "diff": {
            "processes_added": diff.processes_added,
            "processes_removed": diff.processes_removed,
            "memory_change_mb": diff.memory_change_mb,
        },
        "recommendations": recommendations,
    })
    .to_string()
}

fn ipc_get_diff(payload: &serde_json::Value) -> String {
    let a_id = payload["a"].as_str().unwrap_or("");
    let b_id = payload["b"].as_str().unwrap_or("");

    let cp_a = match find_checkpoint(a_id) {
        Some(c) => c,
        None => return json!({ "error": "Checkpoint A not found" }).to_string(),
    };
    let cp_b = match find_checkpoint(b_id) {
        Some(c) => c,
        None => return json!({ "error": "Checkpoint B not found" }).to_string(),
    };

    let diff = diff_states(&cp_a.state, &cp_b.state);

    json!({
        "diff": {
            "processes_added": diff.processes_added,
            "processes_removed": diff.processes_removed,
            "memory_change_mb": diff.memory_change_mb,
            "cpu_change": diff.cpu_change,
            "process_count_change": diff.process_count_change,
        }
    })
    .to_string()
}

fn ipc_get_diagnosis() -> String {
    let current = capture_state();
    let cps = list_checkpoints();

    // Find the best checkpoint (highest health score)
    let best = cps
        .iter()
        .max_by_key(|cp| health_score(&cp.state));

    let diagnosis = match best {
        Some(best_cp) => diagnose(&current, &best_cp.state),
        None => vec![DiagnosisItem {
            cause: "No checkpoints to compare against. Create a checkpoint first.".into(),
            impact: 0,
            category: "Info".into(),
        }],
    };

    json!({
        "diagnosis": diagnosis,
        "current_score": health_score(&current),
    })
    .to_string()
}

// ── IPC entry point ────────────────────────────────────────────────

pub fn handle_ipc(msg_type: &str, payload: &serde_json::Value) -> Option<String> {
    match msg_type {
        "get_checkpoints" | "get_snapshots" => Some(ipc_get_checkpoints()),
        "create_checkpoint" => Some(ipc_create_checkpoint(payload)),
        "rollback_checkpoint" | "rollback_to" => Some(ipc_rollback(payload)),
        "get_diff" | "compare_snapshots" => Some(ipc_get_diff(payload)),
        "get_diagnosis" => Some(ipc_get_diagnosis()),
        _ => None,
    }
}
