//! ADR-018: Spectral Leak Detector
//!
//! Monitors process memory over time and detects leak suspects using
//! simple linear regression on memory samples. Processes with a
//! statistically significant upward trend are flagged as leak suspects.
//!
//! Cross-platform using sysinfo.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

/// Handle IPC messages for the leak detector page.
///
/// Recognised message types:
/// - `get_leak_suspects`  – return currently tracked leak suspects
/// - `get_leak_history`   – return historical detections
/// - `analyze_leaks`      – run a 30-second sampling burst
/// - `kill_leak_suspect`  – terminate a process by PID
/// - `dismiss_leak`       – remove a suspect from tracking
pub fn handle_ipc(msg_type: &str, payload: &Value) -> Option<String> {
    match msg_type {
        "get_leak_suspects" => Some(get_leak_suspects()),
        "get_leak_history" => Some(get_leak_history()),
        "analyze_leaks" => Some(analyze_leaks()),
        "kill_leak_suspect" => Some(kill_leak_suspect(payload)),
        "dismiss_leak" => Some(dismiss_leak(payload)),
        _ => None,
    }
}

// ── In-memory process tracker ──────────────────────────────────────

struct ProcessTracker {
    /// PID -> list of (elapsed_secs, memory_mb) samples
    samples: HashMap<u32, Vec<(f64, f64)>>,
    /// PIDs that have been dismissed by the user
    dismissed: Vec<u32>,
    /// When tracking started
    start_time: Instant,
}

impl ProcessTracker {
    fn new() -> Self {
        Self {
            samples: HashMap::new(),
            dismissed: Vec::new(),
            start_time: Instant::now(),
        }
    }

    fn elapsed_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }
}

fn tracker() -> &'static Mutex<ProcessTracker> {
    use std::sync::OnceLock;
    static TRACKER: OnceLock<Mutex<ProcessTracker>> = OnceLock::new();
    TRACKER.get_or_init(|| Mutex::new(ProcessTracker::new()))
}

// ── IPC handlers ───────────────────────────────────────────────────

fn get_leak_suspects() -> String {
    // Take a sample of current process memory
    let snapshot = take_memory_snapshot();

    // Update tracker
    let suspects = {
        let mut t = match tracker().lock() {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("Tracker lock poisoned: {}", e);
                return json!({ "suspects": [] }).to_string();
            }
        };

        let elapsed = t.elapsed_secs();

        for (pid, _name, mem_mb) in &snapshot {
            t.samples
                .entry(*pid)
                .or_insert_with(Vec::new)
                .push((elapsed, *mem_mb));

            // Keep at most 120 samples per process
            if let Some(samples) = t.samples.get_mut(pid) {
                if samples.len() > 120 {
                    let excess = samples.len() - 120;
                    samples.drain(..excess);
                }
            }
        }

        // Prune processes that are no longer running
        let running_pids: std::collections::HashSet<u32> =
            snapshot.iter().map(|(pid, _, _)| *pid).collect();
        t.samples.retain(|pid, _| running_pids.contains(pid));

        // Build suspect list
        let name_map: HashMap<u32, &str> = snapshot
            .iter()
            .map(|(pid, name, _)| (*pid, name.as_str()))
            .collect();

        build_suspect_list(&t, &name_map)
    };

    json!({ "suspects": suspects }).to_string()
}

fn get_leak_history() -> String {
    let history = load_leak_history();
    json!({ "history": history }).to_string()
}

fn analyze_leaks() -> String {
    // Run a 30-second sampling burst: 6 samples at 5-second intervals
    let sample_count = 6;
    let interval = std::time::Duration::from_secs(5);

    for i in 0..sample_count {
        let snapshot = take_memory_snapshot();
        {
            let mut t = match tracker().lock() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let elapsed = t.elapsed_secs();
            for (pid, _name, mem_mb) in &snapshot {
                t.samples
                    .entry(*pid)
                    .or_insert_with(Vec::new)
                    .push((elapsed, *mem_mb));
            }
        }

        if i < sample_count - 1 {
            std::thread::sleep(interval);
        }
    }

    // Now return updated suspects
    let result = get_leak_suspects();

    // Save analysis to history
    if let Ok(v) = serde_json::from_str::<Value>(&result) {
        if let Some(suspects) = v["suspects"].as_array() {
            let high_confidence: Vec<&Value> = suspects
                .iter()
                .filter(|s| s["confidence"].as_f64().unwrap_or(0.0) >= 0.7)
                .collect();
            if !high_confidence.is_empty() {
                append_leak_history(&high_confidence);
            }
        }
    }

    result
}

fn kill_leak_suspect(payload: &Value) -> String {
    let pid = match payload.get("pid").and_then(|v| v.as_u64()) {
        Some(p) => p as u32,
        None => {
            return json!({ "success": false, "error": "No PID specified" }).to_string();
        }
    };

    let killed = kill_process(pid);

    if killed {
        // Remove from tracker
        if let Ok(mut t) = tracker().lock() {
            t.samples.remove(&pid);
        }
        json!({ "success": true, "pid": pid, "message": "Process terminated." }).to_string()
    } else {
        json!({
            "success": false,
            "pid": pid,
            "error": "Failed to terminate process. It may require elevated privileges.",
        })
        .to_string()
    }
}

fn dismiss_leak(payload: &Value) -> String {
    let pid = match payload.get("pid").and_then(|v| v.as_u64()) {
        Some(p) => p as u32,
        None => {
            return json!({ "success": false, "error": "No PID specified" }).to_string();
        }
    };

    if let Ok(mut t) = tracker().lock() {
        t.dismissed.push(pid);
        t.samples.remove(&pid);
    }

    json!({ "success": true, "pid": pid }).to_string()
}

// ── Memory snapshot ────────────────────────────────────────────────

/// Returns Vec<(pid, name, memory_mb)> for all user-visible processes.
fn take_memory_snapshot() -> Vec<(u32, String, f64)> {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut result = Vec::new();
    for (pid, proc_info) in sys.processes() {
        let mem_bytes = proc_info.memory();
        // Skip very small processes (< 10 MB)
        if mem_bytes < 10 * 1024 * 1024 {
            continue;
        }
        let mem_mb = mem_bytes as f64 / (1024.0 * 1024.0);
        let name = proc_info.name().to_string_lossy().to_string();
        result.push((pid.as_u32(), name, mem_mb));
    }
    result
}

// ── Leak detection via linear regression ───────────────────────────

#[derive(Debug)]
struct LeakSuspect {
    pid: u32,
    name: String,
    current_mb: f64,
    growth_rate_mb_per_min: f64,
    sample_count: usize,
    confidence: f64,
    duration_min: f64,
}

fn build_suspect_list(
    tracker: &ProcessTracker,
    name_map: &HashMap<u32, &str>,
) -> Vec<Value> {
    let min_samples = 4;
    let mut suspects: Vec<LeakSuspect> = Vec::new();

    for (pid, samples) in &tracker.samples {
        if tracker.dismissed.contains(pid) {
            continue;
        }
        if samples.len() < min_samples {
            continue;
        }

        let name = name_map
            .get(pid)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("PID {}", pid));

        let current_mb = samples.last().map(|(_, m)| *m).unwrap_or(0.0);

        // Linear regression: memory_mb = a + b * elapsed_secs
        let (slope, r_squared) = linear_regression(samples);

        // slope is in MB/sec, convert to MB/min
        let growth_rate = slope * 60.0;

        // Duration of tracking in minutes
        let duration_secs = if samples.len() >= 2 {
            samples.last().unwrap().0 - samples.first().unwrap().0
        } else {
            0.0
        };
        let duration_min = duration_secs / 60.0;

        // Confidence based on:
        // - R-squared (how well the line fits)
        // - Positive slope (must be growing)
        // - Sufficient samples
        // - Duration of observation
        if growth_rate <= 0.0 {
            continue; // Not leaking
        }

        let sample_factor = (samples.len() as f64 / 20.0).min(1.0);
        let duration_factor = (duration_min / 5.0).min(1.0);
        let r2_factor = r_squared.max(0.0);
        let size_factor = (current_mb / 500.0).min(1.0);

        let confidence = (r2_factor * 0.5 + sample_factor * 0.2 + duration_factor * 0.2 + size_factor * 0.1)
            .clamp(0.0, 1.0);

        // Only report if confidence is above a minimum threshold
        if confidence >= 0.2 {
            suspects.push(LeakSuspect {
                pid: *pid,
                name,
                current_mb,
                growth_rate_mb_per_min: growth_rate,
                sample_count: samples.len(),
                confidence,
                duration_min,
            });
        }
    }

    // Sort by confidence descending
    suspects.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Take top 20
    suspects
        .iter()
        .take(20)
        .map(|s| {
            json!({
                "pid": s.pid,
                "name": s.name,
                "currentMb": round1(s.current_mb),
                "growthRate": round1(s.growth_rate_mb_per_min * 60.0), // MB/hr for display
                "durationMin": (s.duration_min as u64),
                "confidence": round2(s.confidence),
                "samples": s.sample_count,
                "periodicity": 0, // Simplified: no FFT periodicity in this impl
            })
        })
        .collect()
}

/// Simple linear regression returning (slope, r_squared).
fn linear_regression(samples: &[(f64, f64)]) -> (f64, f64) {
    let n = samples.len() as f64;
    if n < 2.0 {
        return (0.0, 0.0);
    }

    let mut sum_x: f64 = 0.0;
    let mut sum_y: f64 = 0.0;
    let mut sum_xy: f64 = 0.0;
    let mut sum_xx: f64 = 0.0;
    let mut sum_yy: f64 = 0.0;

    for (x, y) in samples {
        sum_x += x;
        sum_y += y;
        sum_xy += x * y;
        sum_xx += x * x;
        sum_yy += y * y;
    }

    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < 1e-10 {
        return (0.0, 0.0);
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denom;

    // R-squared
    let y_mean = sum_y / n;
    let ss_tot = sum_yy - n * y_mean * y_mean;
    let _ss_res = sum_yy - slope * sum_xy - (sum_y / n) * sum_y
        + slope * sum_x * sum_y / n;

    // More robust R-squared calculation
    let ss_tot_safe = if ss_tot.abs() < 1e-10 { 1.0 } else { ss_tot };

    // Use standard formula: R^2 = 1 - SS_res / SS_tot
    let predicted_sum_sq: f64 = samples
        .iter()
        .map(|(x, y)| {
            let intercept = (sum_y - slope * sum_x) / n;
            let predicted = intercept + slope * x;
            (y - predicted).powi(2)
        })
        .sum();

    let r_squared = (1.0 - predicted_sum_sq / ss_tot_safe).clamp(0.0, 1.0);

    (slope, r_squared)
}

// ── Process killing ────────────────────────────────────────────────

fn kill_process(pid: u32) -> bool {
    use sysinfo::{Pid, System};

    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let sysinfo_pid = Pid::from_u32(pid);
    if let Some(process) = sys.process(sysinfo_pid) {
        process.kill()
    } else {
        tracing::warn!("Process {} not found", pid);
        false
    }
}

// ── Leak history persistence ───────────────────────────────────────

fn history_path() -> PathBuf {
    let dir = super::config_dir();
    dir.join("leak_history.json")
}

fn load_leak_history() -> Vec<Value> {
    let path = history_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<Vec<Value>>(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn append_leak_history(suspects: &[&Value]) {
    let mut history = load_leak_history();

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for s in suspects {
        history.push(json!({
            "process": s["name"],
            "pid": s["pid"],
            "leakMb": s["currentMb"],
            "growthRate": s["growthRate"],
            "confidence": s["confidence"],
            "detectedAt": now,
            "resolvedAt": null,
            "resolution": null,
        }));
    }

    // Keep last 20 entries
    if history.len() > 20 {
        history = history.split_off(history.len() - 20);
    }

    if let Ok(dir) = super::ensure_config_dir(None) {
        let path = dir.join("leak_history.json");
        let json_str = serde_json::to_string_pretty(&history).unwrap_or_default();
        if let Err(e) = std::fs::write(&path, json_str) {
            tracing::error!("Failed to save leak history: {}", e);
        }
    }
}

// ── Utilities ──────────────────────────────────────────────────────

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_ipc_unknown() {
        assert!(handle_ipc("unknown_msg", &json!({})).is_none());
    }

    #[test]
    fn test_handle_ipc_suspects() {
        let result = handle_ipc("get_leak_suspects", &json!({}));
        assert!(result.is_some());
        let v: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(v["suspects"].is_array());
    }

    #[test]
    fn test_handle_ipc_history() {
        let result = handle_ipc("get_leak_history", &json!({}));
        assert!(result.is_some());
        let v: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(v["history"].is_array());
    }

    #[test]
    fn test_linear_regression_flat() {
        let samples: Vec<(f64, f64)> = (0..10).map(|i| (i as f64, 100.0)).collect();
        let (slope, _r2) = linear_regression(&samples);
        assert!(slope.abs() < 0.001, "Flat data should have ~0 slope, got {}", slope);
    }

    #[test]
    fn test_linear_regression_upward() {
        let samples: Vec<(f64, f64)> = (0..20).map(|i| (i as f64, 100.0 + i as f64 * 2.0)).collect();
        let (slope, r2) = linear_regression(&samples);
        assert!((slope - 2.0).abs() < 0.01, "Expected slope ~2.0, got {}", slope);
        assert!(r2 > 0.99, "Expected high R^2, got {}", r2);
    }

    #[test]
    fn test_dismiss_leak() {
        let result = handle_ipc("dismiss_leak", &json!({ "pid": 99999 }));
        assert!(result.is_some());
        let v: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(v["success"].as_bool(), Some(true));
    }

    #[test]
    fn test_round() {
        assert_eq!(round1(3.456), 3.5);
        assert_eq!(round2(0.789), 0.79);
    }
}
