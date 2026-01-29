//! ADR-020 — Thermal-Aware Scheduler
//!
//! Cross-platform thermal monitoring with graceful fallbacks.
//!
//! IPC messages handled:
//! - `get_thermal_status`  → current temperatures, fan speed, throttle state
//! - `get_thermal_history` → last N readings with timestamps
//! - `set_thermal_config`  → persist warning/critical thresholds, silent mode
//! - `enable_silent_mode`  → shortcut to toggle silent mode

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalStatus {
    pub cpu_temp_c: f64,
    pub gpu_temp_c: f64,
    pub fan_speed_rpm: u32,
    pub throttle_active: bool,
    pub thermal_zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalReading {
    pub ts: u64, // millis since epoch
    pub cpu_temp: f64,
    pub gpu_temp: f64,
    pub fan_speed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalConfig {
    pub warning_threshold: f64,
    pub critical_threshold: f64,
    pub silent_mode: bool,
    #[serde(default = "default_power_plan")]
    pub power_plan: String,
    #[serde(default = "default_true")]
    pub core_migration: bool,
    #[serde(default = "default_true")]
    pub thermal_prediction: bool,
}

fn default_power_plan() -> String {
    "balanced".into()
}
fn default_true() -> bool {
    true
}

impl Default for ThermalConfig {
    fn default() -> Self {
        Self {
            warning_threshold: 80.0,
            critical_threshold: 95.0,
            silent_mode: false,
            power_plan: default_power_plan(),
            core_migration: true,
            thermal_prediction: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Global state (lazy)
// ---------------------------------------------------------------------------

const MAX_HISTORY: usize = 200;

struct ThermalState {
    history: Vec<ThermalReading>,
    config: ThermalConfig,
}

impl ThermalState {
    fn new() -> Self {
        Self {
            history: Vec::with_capacity(MAX_HISTORY),
            config: load_config(),
        }
    }
}

static STATE: once_cell::sync::Lazy<Mutex<ThermalState>> =
    once_cell::sync::Lazy::new(|| Mutex::new(ThermalState::new()));

// We embed a tiny once_cell inline since the crate is not in deps.
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
        "get_thermal_status" => Some(get_thermal_status()),
        "get_thermal_history" => Some(get_thermal_history()),
        "set_thermal_config" => Some(set_thermal_config(payload)),
        "enable_silent_mode" => {
            let enable = payload.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
            Some(toggle_silent_mode(enable))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn get_thermal_status() -> String {
    let status = read_thermal_status();
    // Push the reading into history
    {
        let now = epoch_millis();
        if let Ok(mut st) = STATE.lock() {
            st.history.push(ThermalReading {
                ts: now,
                cpu_temp: status.cpu_temp_c,
                gpu_temp: status.gpu_temp_c,
                fan_speed: status.fan_speed_rpm,
            });
            if st.history.len() > MAX_HISTORY {
                st.history.remove(0);
            }
        }
    }

    let config = STATE.lock().ok().map(|s| s.config.clone()).unwrap_or_default();

    // Compute thermal zones percentages
    let zones = compute_zones(status.cpu_temp_c, &config);

    // Predict minutes until throttle (simple linear extrapolation)
    let prediction_minutes = predict_throttle_minutes(&config);

    let mut map = serde_json::Map::new();
    map.insert("cpu_temp".into(), serde_json::json!(status.cpu_temp_c));
    map.insert("gpu_temp".into(), serde_json::json!(status.gpu_temp_c));
    map.insert("fan_speed".into(), serde_json::json!(status.fan_speed_rpm));
    map.insert("max_fan_speed".into(), serde_json::json!(3000));
    map.insert(
        "throttle_state".into(),
        serde_json::json!(if status.throttle_active { "thermal" } else { "none" }),
    );
    map.insert(
        "thresholds".into(),
        serde_json::json!({
            "warning": config.warning_threshold,
            "throttle": config.warning_threshold + 5.0,
            "critical": config.critical_threshold,
        }),
    );
    map.insert("zones".into(), serde_json::json!(zones));
    map.insert("power_plan".into(), serde_json::json!(config.power_plan));
    map.insert(
        "prediction_minutes".into(),
        serde_json::json!(prediction_minutes),
    );

    serde_json::to_string(&map).unwrap_or_else(|_| "{}".into())
}

fn get_thermal_history() -> String {
    let history: Vec<ThermalReading> = STATE
        .lock()
        .ok()
        .map(|s| s.history.clone())
        .unwrap_or_default();

    let items: Vec<serde_json::Value> = history
        .iter()
        .map(|r| {
            serde_json::json!({
                "ts": r.ts,
                "cpuTemp": r.cpu_temp,
                "gpuTemp": r.gpu_temp,
                "fanSpeed": r.fan_speed,
            })
        })
        .collect();

    serde_json::json!({ "history": items }).to_string()
}

fn set_thermal_config(payload: &serde_json::Value) -> String {
    if let Ok(mut st) = STATE.lock() {
        if let Some(w) = payload.get("warning_threshold").or(
            payload
                .get("thresholds")
                .and_then(|t| t.get("warning")),
        ) {
            if let Some(v) = w.as_f64() {
                st.config.warning_threshold = v;
            }
        }
        if let Some(c) = payload.get("critical_threshold").or(
            payload
                .get("thresholds")
                .and_then(|t| t.get("critical")),
        ) {
            if let Some(v) = c.as_f64() {
                st.config.critical_threshold = v;
            }
        }
        if let Some(v) = payload.get("silent_mode").and_then(|v| v.as_bool()) {
            st.config.silent_mode = v;
        }
        if let Some(v) = payload.get("power_plan").and_then(|v| v.as_str()) {
            st.config.power_plan = v.to_string();
        }
        if let Some(v) = payload.get("core_migration").and_then(|v| v.as_bool()) {
            st.config.core_migration = v;
        }
        if let Some(v) = payload.get("thermal_prediction").and_then(|v| v.as_bool()) {
            st.config.thermal_prediction = v;
        }
        save_config(&st.config);
    }
    serde_json::json!({"ok": true}).to_string()
}

fn toggle_silent_mode(enable: bool) -> String {
    if let Ok(mut st) = STATE.lock() {
        st.config.silent_mode = enable;
        save_config(&st.config);
    }
    serde_json::json!({"silent_mode": enable}).to_string()
}

// ---------------------------------------------------------------------------
// Thermal reading — cross-platform
// ---------------------------------------------------------------------------

fn read_thermal_status() -> ThermalStatus {
    let (cpu_temp, gpu_temp, fan_rpm, throttle) = read_platform_thermal();

    let zone = if cpu_temp >= 95.0 {
        "critical"
    } else if cpu_temp >= 80.0 {
        "warning"
    } else {
        "normal"
    };

    ThermalStatus {
        cpu_temp_c: cpu_temp,
        gpu_temp_c: gpu_temp,
        fan_speed_rpm: fan_rpm,
        throttle_active: throttle,
        thermal_zone: zone.into(),
    }
}

/// Platform-specific thermal data retrieval.
/// Returns (cpu_temp_c, gpu_temp_c, fan_speed_rpm, throttle_active).
fn read_platform_thermal() -> (f64, f64, u32, bool) {
    // Attempt native reading first; fall back to CPU-load estimation.
    #[cfg(target_os = "windows")]
    {
        if let Some(vals) = read_thermal_windows() {
            return vals;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(vals) = read_thermal_linux() {
            return vals;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(vals) = read_thermal_macos() {
            return vals;
        }
    }

    // ── Fallback: estimate temperature from CPU load ──────────────
    estimate_from_cpu_load()
}

/// Estimate temperature from CPU load (graceful fallback for all platforms).
fn estimate_from_cpu_load() -> (f64, f64, u32, bool) {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_cpu_all();

    // Brief sleep so sysinfo can compute deltas
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_all();

    let cpu_count = sys.cpus().len().max(1) as f64;
    let total_load: f64 = sys.cpus().iter().map(|c| c.cpu_usage() as f64).sum();
    let avg_load = total_load / cpu_count; // 0..100

    // Map load (0-100) to temperature estimate (35-100 C)
    let cpu_temp = 35.0 + avg_load * 0.65;
    let gpu_temp = 30.0 + avg_load * 0.40; // GPUs tend cooler without direct load
    let fan_rpm = (600.0 + avg_load * 24.0) as u32; // 600-3000 range
    let throttle = cpu_temp >= 90.0;

    (cpu_temp, gpu_temp, fan_rpm, throttle)
}

// ---------------------------------------------------------------------------
// Windows: try WMI Win32_TemperatureProbe, fall back to estimation
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn read_thermal_windows() -> Option<(f64, f64, u32, bool)> {
    // WMI COM calls are complex; we attempt a lightweight approach via
    // the `sysinfo` crate's component temperatures (it reads WMI internally
    // on Windows for components that expose thermal data).
    use sysinfo::Components;

    let components = Components::new_with_refreshed_list();
    let mut cpu_temp: Option<f64> = None;
    let mut gpu_temp: Option<f64> = None;

    for comp in &components {
        let label = comp.label().to_lowercase();
        let temp: f64 = match comp.temperature() {
            Some(t) => t as f64,
            None => continue,
        };
        if temp <= 0.0 {
            continue;
        }
        if label.contains("cpu") || label.contains("core") || label.contains("package") {
            cpu_temp = Some(cpu_temp.map_or(temp, |prev: f64| prev.max(temp)));
        }
        if label.contains("gpu") || label.contains("nvidia") || label.contains("radeon") {
            gpu_temp = Some(gpu_temp.map_or(temp, |prev: f64| prev.max(temp)));
        }
    }

    // If we got at least CPU temp, consider it a success
    if let Some(ct) = cpu_temp {
        let gt = gpu_temp.unwrap_or(ct * 0.85);

        // Estimate fan from temperature range
        let fan = estimate_fan_rpm(ct);
        let throttle = ct >= 95.0;
        Some((ct, gt, fan, throttle))
    } else {
        // No thermal data available via WMI/sysinfo — caller will use fallback
        None
    }
}

// ---------------------------------------------------------------------------
// Linux: /sys/class/thermal/thermal_zone*/temp
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn read_thermal_linux() -> Option<(f64, f64, u32, bool)> {
    let mut max_temp: Option<f64> = None;

    // Iterate thermal zones
    let thermal_dir = std::path::Path::new("/sys/class/thermal");
    if let Ok(entries) = std::fs::read_dir(thermal_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with("thermal_zone") {
                continue;
            }
            let temp_path = entry.path().join("temp");
            if let Ok(content) = std::fs::read_to_string(&temp_path) {
                if let Ok(millideg) = content.trim().parse::<i64>() {
                    let celsius = millideg as f64 / 1000.0;
                    max_temp = Some(max_temp.map_or(celsius, |prev: f64| prev.max(celsius)));
                }
            }
        }
    }

    if let Some(ct) = max_temp {
        let gt = ct * 0.85;
        let fan = estimate_fan_rpm(ct);
        let throttle = ct >= 95.0;
        Some((ct, gt, fan, throttle))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// macOS: stub (IOKit / powermetrics require elevated privileges)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn read_thermal_macos() -> Option<(f64, f64, u32, bool)> {
    // On macOS, temperature data requires IOKit SMC access or running
    // `sudo powermetrics`. Both require elevated privileges. We return
    // None to trigger the CPU-load estimation fallback.
    //
    // In a future iteration this could shell out to a helper binary
    // that reads the SMC.
    None
}

/// Estimate fan RPM from temperature.
fn estimate_fan_rpm(temp_c: f64) -> u32 {
    if temp_c < 40.0 {
        600
    } else if temp_c > 90.0 {
        3000
    } else {
        // Linear interpolation 600..3000 over 40..90 C
        let ratio = (temp_c - 40.0) / 50.0;
        (600.0 + ratio * 2400.0) as u32
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn compute_zones(cpu_temp: f64, cfg: &ThermalConfig) -> serde_json::Value {
    let (green, yellow, red) = if cpu_temp < cfg.warning_threshold {
        (100, 0, 0)
    } else if cpu_temp < cfg.critical_threshold {
        let ratio =
            (cpu_temp - cfg.warning_threshold) / (cfg.critical_threshold - cfg.warning_threshold);
        let yellow_pct = (ratio * 100.0).min(100.0) as u32;
        (100u32.saturating_sub(yellow_pct), yellow_pct, 0)
    } else {
        (0, 0, 100)
    };

    serde_json::json!({
        "green": green,
        "yellow": yellow,
        "red": red,
    })
}

fn predict_throttle_minutes(cfg: &ThermalConfig) -> serde_json::Value {
    let history: Vec<ThermalReading> = STATE
        .lock()
        .ok()
        .map(|s| s.history.clone())
        .unwrap_or_default();

    if history.len() < 5 {
        return serde_json::Value::Null;
    }

    // Simple linear regression on the last 10 readings
    let window: Vec<&ThermalReading> = history.iter().rev().take(10).collect();
    let n = window.len() as f64;
    if n < 2.0 {
        return serde_json::Value::Null;
    }

    let first_ts = window.last().map(|r| r.ts).unwrap_or(0) as f64;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xy = 0.0;
    let mut sum_xx = 0.0;

    for r in &window {
        let x = (r.ts as f64 - first_ts) / 60_000.0; // minutes
        let y = r.cpu_temp;
        sum_x += x;
        sum_y += y;
        sum_xy += x * y;
        sum_xx += x * x;
    }

    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < 1e-9 {
        return serde_json::Value::Null;
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y - slope * sum_x) / n;

    if slope <= 0.0 {
        // Temperature not rising
        return serde_json::Value::Null;
    }

    let current_temp = intercept + slope * (sum_x / n);
    let target = cfg.critical_threshold;
    if current_temp >= target {
        return serde_json::json!(0);
    }

    let minutes_to_throttle = (target - current_temp) / slope;
    serde_json::json!(minutes_to_throttle.max(0.0).min(999.0))
}

fn epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Config persistence
// ---------------------------------------------------------------------------

fn config_path() -> std::path::PathBuf {
    let mut p = super::config_dir();
    p.push("thermal_config.toml");
    p
}

fn load_config() -> ThermalConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => ThermalConfig::default(),
    }
}

fn save_config(cfg: &ThermalConfig) {
    if let Err(e) = super::ensure_config_dir(None) {
        tracing::warn!("Failed to create config dir: {}", e);
        return;
    }
    let path = config_path();
    match toml::to_string_pretty(cfg) {
        Ok(content) => {
            if let Err(e) = std::fs::write(&path, content) {
                tracing::warn!("Failed to write thermal config: {}", e);
            }
        }
        Err(e) => {
            tracing::warn!("Failed to serialize thermal config: {}", e);
        }
    }
}
