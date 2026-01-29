//! ADR-019: Predictive Prefetcher
//!
//! Markov-chain based application launch prediction with temporal weights.
//! Tracks which applications are launched and when, builds transition
//! probabilities, and predicts the next likely applications based on
//! the current time and most recently launched app.
//!
//! Cross-platform using chrono for time handling.

use chrono::{Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Handle IPC messages for the predictive prefetcher page.
///
/// Recognised message types:
/// - `get_prefetch_status`      – overall status, hit/miss rates, model info
/// - `get_prefetch_predictions` – top predicted apps with probabilities
/// - `get_predictions`          – alias for get_prefetch_predictions
/// - `train_prefetcher`         – rebuild Markov chain from stored records
/// - `clear_prefetch_cache`     – wipe history and model
/// - `set_prefetch_config`      – update prefetch settings
/// - `get_app_patterns`         – app usage heatmap data
pub fn handle_ipc(msg_type: &str, payload: &Value) -> Option<String> {
    match msg_type {
        "get_prefetch_status" => Some(get_prefetch_status()),
        "get_prefetch_predictions" | "get_predictions" => Some(get_predictions()),
        "train_prefetcher" => Some(train_prefetcher()),
        "clear_prefetch_cache" => Some(clear_prefetch_cache()),
        "set_prefetch_config" => Some(set_prefetch_config(payload)),
        "get_app_patterns" => Some(get_app_patterns()),
        _ => None,
    }
}

// ── Data structures ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LaunchRecord {
    app_name: String,
    timestamp: String,
    hour: u32,
    day_of_week: u32, // 0=Mon .. 6=Sun
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MarkovModel {
    /// Transition counts: from_app -> { to_app -> count }
    transitions: HashMap<String, HashMap<String, u32>>,
    /// Time-based launch counts: app -> { hour -> count }
    hourly_counts: HashMap<String, HashMap<u32, u32>>,
    /// Day-of-week launch counts: app -> { day -> count }
    daily_counts: HashMap<String, HashMap<u32, u32>>,
    /// Total transitions from each app
    transition_totals: HashMap<String, u32>,
    /// Total launches per app
    app_totals: HashMap<String, u32>,
    /// All known apps
    apps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PrefetchConfig {
    enabled: bool,
    max_prefetch_mb: u32,
    confidence_threshold: f64,
}

impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_prefetch_mb: 512,
            confidence_threshold: 0.7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PrefetchState {
    records: Vec<LaunchRecord>,
    model: MarkovModel,
    config: PrefetchConfig,
    hits: u32,
    misses: u32,
    last_app: Option<String>,
}

impl Default for PrefetchState {
    fn default() -> Self {
        Self {
            records: Vec::new(),
            model: MarkovModel::default(),
            config: PrefetchConfig::default(),
            hits: 0,
            misses: 0,
            last_app: None,
        }
    }
}

fn state() -> &'static Mutex<PrefetchState> {
    use std::sync::OnceLock;
    static STATE: OnceLock<Mutex<PrefetchState>> = OnceLock::new();
    STATE.get_or_init(|| {
        let loaded = load_prefetch_state().unwrap_or_default();
        Mutex::new(loaded)
    })
}

// ── IPC handlers ───────────────────────────────────────────────────

fn get_prefetch_status() -> String {
    let s = match state().lock() {
        Ok(s) => s,
        Err(_) => return json!({ "error": "State lock poisoned" }).to_string(),
    };

    let total_predictions = s.hits + s.misses;
    let hit_rate = if total_predictions > 0 {
        (s.hits as f64 / total_predictions as f64) * 100.0
    } else {
        0.0
    };
    let miss_rate = 100.0 - hit_rate;

    let states_count = s.model.apps.len();
    let transitions_count: u32 = s.model.transition_totals.values().sum();

    // Overall score: weighted combination of hit rate and model coverage
    let coverage = (states_count as f64 / 20.0).min(1.0);
    let overall_score = hit_rate * 0.7 + coverage * 30.0;

    json!({
        "enabled": s.config.enabled,
        "hit_rate": round1(hit_rate),
        "miss_rate": round1(miss_rate),
        "overall_score": round1(overall_score.min(100.0)),
        "training_samples": s.records.len(),
        "model_accuracy": round1(hit_rate),
        "last_training_time": if s.records.is_empty() { "Never".to_string() } else { "Available".to_string() },
        "states": states_count,
        "transitions": transitions_count,
        "max_prefetch_mb": s.config.max_prefetch_mb,
        "confidence_threshold": s.config.confidence_threshold,
    })
    .to_string()
}

fn get_predictions() -> String {
    let s = match state().lock() {
        Ok(s) => s,
        Err(_) => return json!({ "predictions": [] }).to_string(),
    };

    let now = Local::now();
    let current_hour = now.hour();
    let current_day = now.weekday().num_days_from_monday();

    let predictions = predict_next_apps(&s.model, s.last_app.as_deref(), current_hour, current_day);

    let pred_values: Vec<Value> = predictions
        .iter()
        .map(|(app, confidence)| {
            let status = if *confidence >= 0.8 {
                "prefetched"
            } else if *confidence >= 0.5 {
                "waiting"
            } else {
                "missed"
            };
            json!({
                "app": app,
                "confidence": round0(*confidence * 100.0),
                "time": format!("{:02}:{:02}", current_hour, now.minute()),
                "status": status,
                "sizeMb": estimate_app_size(app),
            })
        })
        .collect();

    json!({ "predictions": pred_values }).to_string()
}

fn train_prefetcher() -> String {
    let mut s = match state().lock() {
        Ok(s) => s,
        Err(_) => return json!({ "success": false, "error": "Lock poisoned" }).to_string(),
    };

    // Rebuild Markov model from records
    s.model = build_markov_model(&s.records);

    // Save state
    if let Err(e) = save_prefetch_state_inner(&s) {
        tracing::error!("Failed to save prefetch state: {}", e);
    }

    let accuracy = if s.hits + s.misses > 0 {
        (s.hits as f64 / (s.hits + s.misses) as f64) * 100.0
    } else {
        0.0
    };

    json!({
        "success": true,
        "accuracy": round1(accuracy),
        "states": s.model.apps.len(),
        "transitions": s.model.transition_totals.values().sum::<u32>(),
        "samples": s.records.len(),
    })
    .to_string()
}

fn clear_prefetch_cache() -> String {
    let mut s = match state().lock() {
        Ok(s) => s,
        Err(_) => return json!({ "success": false, "error": "Lock poisoned" }).to_string(),
    };

    s.records.clear();
    s.model = MarkovModel::default();
    s.hits = 0;
    s.misses = 0;
    s.last_app = None;

    // Delete persisted files
    let dir = super::config_dir();
    let _ = std::fs::remove_file(dir.join("prefetch_state.json"));

    json!({
        "success": true,
        "message": "Prefetch history and model cleared.",
    })
    .to_string()
}

fn set_prefetch_config(payload: &Value) -> String {
    let mut s = match state().lock() {
        Ok(s) => s,
        Err(_) => return json!({ "success": false, "error": "Lock poisoned" }).to_string(),
    };

    if let Some(enabled) = payload.get("enabled").and_then(|v| v.as_bool()) {
        s.config.enabled = enabled;
    }
    if let Some(max_mb) = payload.get("max_prefetch_mb").and_then(|v| v.as_u64()) {
        s.config.max_prefetch_mb = max_mb as u32;
    }
    if let Some(threshold) = payload.get("confidence_threshold").and_then(|v| v.as_f64()) {
        s.config.confidence_threshold = threshold;
    }

    if let Err(e) = save_prefetch_state_inner(&s) {
        tracing::error!("Failed to save prefetch config: {}", e);
    }

    json!({ "success": true, "message": "Prefetch configuration updated." }).to_string()
}

fn get_app_patterns() -> String {
    let s = match state().lock() {
        Ok(s) => s,
        Err(_) => return json!({ "app_patterns": [] }).to_string(),
    };

    let mut patterns: Vec<Value> = Vec::new();

    for (app_idx, app) in s.model.apps.iter().enumerate() {
        let hourly = s.model.hourly_counts.get(app);
        let daily = s.model.daily_counts.get(app);
        let total = s.model.app_totals.get(app).copied().unwrap_or(1) as f64;

        for day in 0..7u32 {
            for hour in 0..24u32 {
                let h_count = hourly
                    .and_then(|h| h.get(&hour))
                    .copied()
                    .unwrap_or(0) as f64;
                let d_count = daily
                    .and_then(|d| d.get(&day))
                    .copied()
                    .unwrap_or(0) as f64;

                // Intensity: weighted combination of hourly and daily patterns
                let intensity = if total > 0.0 {
                    ((h_count / total) * 0.6 + (d_count / total) * 0.4).min(1.0)
                } else {
                    0.0
                };

                patterns.push(json!({
                    "app": app,
                    "appIndex": app_idx,
                    "day": day,
                    "hour": hour,
                    "intensity": round2(intensity),
                }));
            }
        }
    }

    json!({ "app_patterns": patterns }).to_string()
}

// ── Markov model building ──────────────────────────────────────────

fn build_markov_model(records: &[LaunchRecord]) -> MarkovModel {
    let mut model = MarkovModel::default();
    let mut app_set: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Build transition counts
    for window in records.windows(2) {
        let from = &window[0].app_name;
        let to = &window[1].app_name;

        app_set.insert(from.clone());
        app_set.insert(to.clone());

        *model
            .transitions
            .entry(from.clone())
            .or_default()
            .entry(to.clone())
            .or_insert(0) += 1;

        *model
            .transition_totals
            .entry(from.clone())
            .or_insert(0) += 1;
    }

    // Build hourly and daily counts
    for record in records {
        app_set.insert(record.app_name.clone());

        *model
            .hourly_counts
            .entry(record.app_name.clone())
            .or_default()
            .entry(record.hour)
            .or_insert(0) += 1;

        *model
            .daily_counts
            .entry(record.app_name.clone())
            .or_default()
            .entry(record.day_of_week)
            .or_insert(0) += 1;

        *model
            .app_totals
            .entry(record.app_name.clone())
            .or_insert(0) += 1;
    }

    model.apps = app_set.into_iter().collect();
    model.apps.sort();

    model
}

// ── Prediction ─────────────────────────────────────────────────────

fn predict_next_apps(
    model: &MarkovModel,
    last_app: Option<&str>,
    current_hour: u32,
    current_day: u32,
) -> Vec<(String, f64)> {
    if model.apps.is_empty() {
        return Vec::new();
    }

    let mut scores: HashMap<String, f64> = HashMap::new();
    let total_records: u32 = model.app_totals.values().sum();
    let total_records_f = total_records.max(1) as f64;

    for app in &model.apps {
        let mut score: f64 = 0.0;

        // 1. Markov transition probability (weight: 0.4)
        if let Some(from_app) = last_app {
            if let Some(transitions) = model.transitions.get(from_app) {
                let total = model.transition_totals.get(from_app).copied().unwrap_or(1) as f64;
                let count = transitions.get(app).copied().unwrap_or(0) as f64;
                score += (count / total) * 0.4;
            }
        }

        // 2. Time-of-day probability (weight: 0.35)
        if let Some(hourly) = model.hourly_counts.get(app) {
            let app_total = model.app_totals.get(app).copied().unwrap_or(1) as f64;
            // Check current hour and adjacent hours
            let h_count = hourly.get(&current_hour).copied().unwrap_or(0) as f64;
            let h_prev = hourly.get(&((current_hour + 23) % 24)).copied().unwrap_or(0) as f64;
            let h_next = hourly.get(&((current_hour + 1) % 24)).copied().unwrap_or(0) as f64;
            let weighted_count = h_count * 1.0 + h_prev * 0.3 + h_next * 0.3;
            score += (weighted_count / app_total) * 0.35;
        }

        // 3. Day-of-week probability (weight: 0.15)
        if let Some(daily) = model.daily_counts.get(app) {
            let app_total = model.app_totals.get(app).copied().unwrap_or(1) as f64;
            let d_count = daily.get(&current_day).copied().unwrap_or(0) as f64;
            score += (d_count / app_total) * 0.15;
        }

        // 4. Overall frequency (weight: 0.1) -- base prior
        let freq = model.app_totals.get(app).copied().unwrap_or(0) as f64;
        score += (freq / total_records_f) * 0.1;

        scores.insert(app.clone(), score);
    }

    // Normalize scores to [0, 1]
    let max_score = scores.values().cloned().fold(0.0f64, f64::max);
    if max_score > 0.0 {
        for v in scores.values_mut() {
            *v /= max_score;
        }
    }

    // Sort by score descending, take top 5
    let mut sorted: Vec<(String, f64)> = scores.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(5);

    sorted
}

/// Rough estimate of app memory footprint for display purposes.
fn estimate_app_size(app: &str) -> u32 {
    let lower = app.to_lowercase();
    if lower.contains("code") || lower.contains("vscode") {
        85
    } else if lower.contains("chrome") || lower.contains("firefox") || lower.contains("edge") {
        120
    } else if lower.contains("slack") {
        65
    } else if lower.contains("docker") {
        180
    } else if lower.contains("terminal") || lower.contains("cmd") || lower.contains("powershell") {
        12
    } else if lower.contains("teams") {
        95
    } else if lower.contains("figma") {
        110
    } else if lower.contains("postman") {
        90
    } else {
        50
    }
}

// ── Record a launch (called from external code) ────────────────────

/// Record an application launch event.
/// This can be called by the monitoring subsystem when a new process starts.
#[allow(dead_code)]
pub fn record_launch(app_name: &str) {
    let now = Local::now();
    let record = LaunchRecord {
        app_name: app_name.to_string(),
        timestamp: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        hour: now.hour(),
        day_of_week: now.weekday().num_days_from_monday(),
    };

    if let Ok(mut s) = state().lock() {
        s.last_app = Some(app_name.to_string());
        s.records.push(record);

        // Keep at most 5000 records
        if s.records.len() > 5000 {
            let excess = s.records.len() - 5000;
            s.records.drain(..excess);
        }

        // Auto-rebuild model periodically (every 50 launches)
        if s.records.len() % 50 == 0 {
            s.model = build_markov_model(&s.records);
        }

        let _ = save_prefetch_state_inner(&s);
    }
}

// ── Persistence ────────────────────────────────────────────────────

fn state_path() -> PathBuf {
    let dir = super::config_dir();
    dir.join("prefetch_state.json")
}

fn load_prefetch_state() -> Option<PrefetchState> {
    let path = state_path();
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_prefetch_state_inner(s: &PrefetchState) -> std::io::Result<()> {
    let dir = super::ensure_config_dir(None)?;
    let path = dir.join("prefetch_state.json");
    let json_str = serde_json::to_string(s).unwrap_or_default();
    std::fs::write(&path, json_str)
}

// ── Utilities ──────────────────────────────────────────────────────

fn round0(v: f64) -> f64 {
    v.round()
}

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
    fn test_handle_ipc_status() {
        let result = handle_ipc("get_prefetch_status", &json!({}));
        assert!(result.is_some());
        let v: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(v.get("enabled").is_some());
        assert!(v.get("hit_rate").is_some());
        assert!(v.get("states").is_some());
    }

    #[test]
    fn test_handle_ipc_predictions() {
        let result = handle_ipc("get_predictions", &json!({}));
        assert!(result.is_some());
        let v: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(v["predictions"].is_array());
    }

    #[test]
    fn test_build_markov_model_empty() {
        let model = build_markov_model(&[]);
        assert!(model.apps.is_empty());
        assert!(model.transitions.is_empty());
    }

    #[test]
    fn test_build_markov_model_basic() {
        let records = vec![
            LaunchRecord {
                app_name: "VSCode".into(),
                timestamp: "2025-01-01 09:00:00".into(),
                hour: 9,
                day_of_week: 0,
            },
            LaunchRecord {
                app_name: "Chrome".into(),
                timestamp: "2025-01-01 09:05:00".into(),
                hour: 9,
                day_of_week: 0,
            },
            LaunchRecord {
                app_name: "VSCode".into(),
                timestamp: "2025-01-01 09:10:00".into(),
                hour: 9,
                day_of_week: 0,
            },
        ];

        let model = build_markov_model(&records);
        assert_eq!(model.apps.len(), 2);
        assert!(model.transitions.contains_key("VSCode"));
        assert!(model.transitions.contains_key("Chrome"));

        // VSCode -> Chrome transition should exist
        let vs_transitions = model.transitions.get("VSCode").unwrap();
        assert_eq!(vs_transitions.get("Chrome").copied().unwrap_or(0), 1);
    }

    #[test]
    fn test_predict_next_apps() {
        let records = vec![
            LaunchRecord { app_name: "A".into(), timestamp: "".into(), hour: 9, day_of_week: 0 },
            LaunchRecord { app_name: "B".into(), timestamp: "".into(), hour: 9, day_of_week: 0 },
            LaunchRecord { app_name: "A".into(), timestamp: "".into(), hour: 9, day_of_week: 0 },
            LaunchRecord { app_name: "B".into(), timestamp: "".into(), hour: 9, day_of_week: 0 },
            LaunchRecord { app_name: "C".into(), timestamp: "".into(), hour: 14, day_of_week: 3 },
        ];

        let model = build_markov_model(&records);
        let predictions = predict_next_apps(&model, Some("A"), 9, 0);

        assert!(!predictions.is_empty());
        // B should be the top prediction after A at hour 9
        assert_eq!(predictions[0].0, "B");
    }

    #[test]
    fn test_clear_prefetch_cache() {
        let result = handle_ipc("clear_prefetch_cache", &json!({}));
        assert!(result.is_some());
        let v: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(v["success"].as_bool(), Some(true));
    }

    #[test]
    fn test_estimate_app_size() {
        assert_eq!(estimate_app_size("VS Code"), 85);
        assert_eq!(estimate_app_size("Chrome"), 120);
        assert_eq!(estimate_app_size("Unknown App"), 50);
    }

    #[test]
    fn test_round_functions() {
        assert_eq!(round0(3.7), 4.0);
        assert_eq!(round1(3.456), 3.5);
        assert_eq!(round2(3.456), 3.46);
    }
}
