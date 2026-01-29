//! ADR-016: WSL2 Memory Governor
//!
//! Bridges Windows and Linux memory subsystems via vmmem monitoring.
//! Applies governor policies to prevent WSL2 from consuming all host memory.
//!
//! On non-Windows platforms, all IPC handlers return a "not available" response.

use serde_json::{json, Value};

/// Handle IPC messages for the WSL2 governor page.
///
/// Recognised message types:
/// - `get_wsl2_status`     – detection info, memory usage, pressure, config
/// - `get_wsl2_processes`  – list of WSL2-related processes
/// - `set_wsl2_config`     – write .wslconfig and/or auto-governor flag
/// - `reclaim_wsl2_memory` – reclaim memory via `wsl --shutdown`
pub fn handle_ipc(msg_type: &str, payload: &Value) -> Option<String> {
    match msg_type {
        "get_wsl2_status" => Some(get_wsl2_status()),
        "get_wsl2_processes" => Some(get_wsl2_processes()),
        "set_wsl2_config" => Some(set_wsl2_config(payload)),
        "reclaim_wsl2_memory" => Some(reclaim_wsl2_memory()),
        _ => None,
    }
}

// ── Windows implementation ─────────────────────────────────────────

#[cfg(target_os = "windows")]
fn get_wsl2_status() -> String {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let host_ram_gb = sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);

    // Detect WSL2 by looking for the vmmem process
    let vmmem = find_vmmem_process(&sys);
    let detected = vmmem.is_some();

    let (vm_memory_gb, current_usage_gb) = match &vmmem {
        Some(info) => {
            let mem_gb = info.memory_mb / 1024.0;
            (mem_gb, mem_gb)
        }
        None => (0.0, 0.0),
    };

    // Read .wslconfig
    let wsl_config = read_wslconfig();
    let max_memory_gb = wsl_config.memory_gb.unwrap_or(host_ram_gb.min(16.0));

    // Determine pressure level
    let usage_ratio = if max_memory_gb > 0.0 {
        current_usage_gb / max_memory_gb
    } else {
        0.0
    };
    let pressure = if usage_ratio > 0.9 {
        "critical"
    } else if usage_ratio > 0.75 {
        "high"
    } else if usage_ratio > 0.5 {
        "medium"
    } else {
        "low"
    };

    // Detect distro info
    let distro_info = detect_distro_info();

    let config_obj = json!({
        "memory": wsl_config.memory_str.clone().unwrap_or_default(),
        "swap": wsl_config.swap_str.clone().unwrap_or_default(),
        "processors": wsl_config.processors.unwrap_or(num_cpus::get() as u32),
        "localhostForwarding": wsl_config.localhost_forwarding.unwrap_or(true),
    });

    json!({
        "detected": detected,
        "distro": distro_info.distro,
        "kernel": distro_info.kernel,
        "vm_memory_gb": round2(vm_memory_gb),
        "current_usage_gb": round2(current_usage_gb),
        "max_memory_gb": round2(max_memory_gb),
        "host_ram_gb": round2(host_ram_gb),
        "pressure": pressure,
        "auto_governor": read_auto_governor(),
        "swap_size_gb": wsl_config.swap_gb.unwrap_or(2.0),
        "config": config_obj,
    })
    .to_string()
}

#[cfg(target_os = "windows")]
fn get_wsl2_processes() -> String {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    // Collect WSL2-related processes: vmmem, wsl.exe, wslhost.exe, etc.
    let wsl_names: &[&str] = &["vmmem", "wsl", "wslhost", "wslservice", "plan9"];
    let mut procs: Vec<Value> = Vec::new();

    for (pid, proc_info) in sys.processes() {
        let name = proc_info.name().to_string_lossy().to_lowercase();
        let name_no_ext = name.trim_end_matches(".exe");
        if wsl_names.iter().any(|n| name_no_ext == *n) {
            let mem_mb = proc_info.memory() as f64 / (1024.0 * 1024.0);
            let cpu_pct = proc_info.cpu_usage();
            procs.push(json!({
                "name": proc_info.name().to_string_lossy(),
                "pid": pid.as_u32(),
                "memory_mb": round2(mem_mb),
                "cpu_pct": round1(cpu_pct as f64),
            }));
        }
    }

    // Sort by memory descending
    procs.sort_by(|a, b| {
        let ma = a["memory_mb"].as_f64().unwrap_or(0.0);
        let mb = b["memory_mb"].as_f64().unwrap_or(0.0);
        mb.partial_cmp(&ma).unwrap_or(std::cmp::Ordering::Equal)
    });

    json!({ "processes": procs }).to_string()
}

#[cfg(target_os = "windows")]
fn set_wsl2_config(payload: &Value) -> String {
    // Handle auto-governor toggle
    if let Some(auto_gov) = payload.get("auto_governor").and_then(|v| v.as_bool()) {
        save_auto_governor(auto_gov);
    }

    // Handle .wslconfig changes
    if let Some(config) = payload.get("config") {
        if let Err(e) = write_wslconfig(config) {
            tracing::error!("Failed to write .wslconfig: {}", e);
            return json!({
                "success": false,
                "error": format!("Failed to write .wslconfig: {}", e),
            })
            .to_string();
        }
    }

    json!({
        "success": true,
        "message": "WSL2 configuration saved. Restart WSL2 to apply changes.",
    })
    .to_string()
}

#[cfg(target_os = "windows")]
fn reclaim_wsl2_memory() -> String {
    use sysinfo::System;

    // Capture memory before reclaim
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let before_mb = find_vmmem_process(&sys)
        .map(|p| p.memory_mb)
        .unwrap_or(0.0);

    // Run wsl --shutdown to reclaim memory
    let output = std::process::Command::new("wsl")
        .arg("--shutdown")
        .output();

    match output {
        Ok(o) if o.status.success() => {
            // Wait a moment for memory to be released
            std::thread::sleep(std::time::Duration::from_secs(2));

            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            let after_mb = find_vmmem_process(&sys)
                .map(|p| p.memory_mb)
                .unwrap_or(0.0);
            let freed_mb = (before_mb - after_mb).max(0.0);

            json!({
                "success": true,
                "freed_mb": round1(freed_mb),
                "before_mb": round1(before_mb),
                "after_mb": round1(after_mb),
            })
            .to_string()
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            json!({
                "success": false,
                "error": format!("wsl --shutdown failed: {}", stderr.trim()),
            })
            .to_string()
        }
        Err(e) => json!({
            "success": false,
            "error": format!("Failed to run wsl --shutdown: {}", e),
        })
        .to_string(),
    }
}

// ── Windows helpers ────────────────────────────────────────────────

#[cfg(target_os = "windows")]
struct VmmemInfo {
    memory_mb: f64,
}

#[cfg(target_os = "windows")]
fn find_vmmem_process(sys: &sysinfo::System) -> Option<VmmemInfo> {
    for (_pid, proc_info) in sys.processes() {
        let name = proc_info.name().to_string_lossy().to_lowercase();
        if name == "vmmem" || name == "vmmem.exe" {
            return Some(VmmemInfo {
                memory_mb: proc_info.memory() as f64 / (1024.0 * 1024.0),
            });
        }
    }
    None
}

#[cfg(target_os = "windows")]
struct WslConfig {
    memory_gb: Option<f64>,
    memory_str: Option<String>,
    swap_gb: Option<f64>,
    swap_str: Option<String>,
    processors: Option<u32>,
    localhost_forwarding: Option<bool>,
}

#[cfg(target_os = "windows")]
fn read_wslconfig() -> WslConfig {
    let path = wslconfig_path();
    let mut cfg = WslConfig {
        memory_gb: None,
        memory_str: None,
        swap_gb: None,
        swap_str: None,
        processors: None,
        localhost_forwarding: None,
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return cfg,
    };

    // Parse TOML-like .wslconfig (it is an INI-ish format)
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with('[') || line.is_empty() {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim().to_lowercase();
            let val = val.trim();
            match key.as_str() {
                "memory" => {
                    cfg.memory_str = Some(val.to_string());
                    cfg.memory_gb = parse_size_gb(val);
                }
                "swap" => {
                    cfg.swap_str = Some(val.to_string());
                    cfg.swap_gb = parse_size_gb(val);
                }
                "processors" => {
                    cfg.processors = val.parse::<u32>().ok();
                }
                "localhostforwarding" => {
                    cfg.localhost_forwarding = Some(val.eq_ignore_ascii_case("true"));
                }
                _ => {}
            }
        }
    }

    cfg
}

#[cfg(target_os = "windows")]
fn write_wslconfig(config: &Value) -> std::io::Result<()> {
    let path = wslconfig_path();
    let mut lines = vec!["[wsl2]".to_string()];

    if let Some(mem) = config.get("memory").and_then(|v| v.as_str()) {
        lines.push(format!("memory={}", mem));
    }
    if let Some(swap) = config.get("swap").and_then(|v| v.as_str()) {
        lines.push(format!("swap={}", swap));
    }
    if let Some(procs) = config.get("processors").and_then(|v| v.as_u64()) {
        lines.push(format!("processors={}", procs));
    }
    if let Some(lf) = config.get("localhostForwarding").and_then(|v| v.as_bool()) {
        lines.push(format!("localhostForwarding={}", lf));
    }

    let content = lines.join("\n") + "\n";
    std::fs::write(&path, content)
}

#[cfg(target_os = "windows")]
fn wslconfig_path() -> std::path::PathBuf {
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let mut p = std::path::PathBuf::from(profile);
        p.push(".wslconfig");
        return p;
    }
    std::path::PathBuf::from(r"C:\Users\Default\.wslconfig")
}

#[cfg(target_os = "windows")]
fn parse_size_gb(s: &str) -> Option<f64> {
    let s = s.trim().to_uppercase();
    if let Some(num) = s.strip_suffix("GB") {
        return num.trim().parse::<f64>().ok();
    }
    if let Some(num) = s.strip_suffix("MB") {
        return num.trim().parse::<f64>().ok().map(|v| v / 1024.0);
    }
    if let Some(num) = s.strip_suffix("TB") {
        return num.trim().parse::<f64>().ok().map(|v| v * 1024.0);
    }
    s.parse::<f64>().ok()
}

#[cfg(target_os = "windows")]
struct DistroInfo {
    distro: String,
    kernel: String,
}

#[cfg(target_os = "windows")]
fn detect_distro_info() -> DistroInfo {
    let mut info = DistroInfo {
        distro: "--".to_string(),
        kernel: "--".to_string(),
    };

    // Try `wsl --list --verbose`
    if let Ok(output) = std::process::Command::new("wsl")
        .args(["--list", "--verbose"])
        .output()
    {
        if output.status.success() {
            // wsl --list outputs UTF-16 on Windows
            let stdout = String::from_utf16_lossy(
                &output
                    .stdout
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect::<Vec<u16>>(),
            );
            // Parse lines: NAME  STATE  VERSION
            for line in stdout.lines().skip(1) {
                let line = line.trim().replace('\0', "");
                if line.is_empty() {
                    continue;
                }
                // Default distro is marked with *
                let is_default = line.starts_with('*');
                let cleaned = line.trim_start_matches('*').trim();
                let parts: Vec<&str> = cleaned.split_whitespace().collect();
                if parts.len() >= 3 && is_default {
                    info.distro = parts[0].to_string();
                    break;
                } else if parts.len() >= 3 && info.distro == "--" {
                    info.distro = parts[0].to_string();
                }
            }
        }
    }

    // Try to get kernel version
    if let Ok(output) = std::process::Command::new("wsl")
        .args(["--", "uname", "-r"])
        .output()
    {
        if output.status.success() {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !ver.is_empty() {
                info.kernel = ver;
            }
        }
    }

    info
}

/// Read auto-governor flag from config.
#[cfg(target_os = "windows")]
fn read_auto_governor() -> bool {
    let dir = super::config_dir();
    let path = dir.join("wsl2_governor.json");
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<Value>(&content)
            .ok()
            .and_then(|v| v["auto_governor"].as_bool())
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Save auto-governor flag to config.
#[cfg(target_os = "windows")]
fn save_auto_governor(enabled: bool) {
    if let Ok(dir) = super::ensure_config_dir(None) {
        let path = dir.join("wsl2_governor.json");
        let val = json!({ "auto_governor": enabled });
        if let Err(e) = std::fs::write(&path, val.to_string()) {
            tracing::error!("Failed to save auto-governor state: {}", e);
        }
    }
}

// ── Non-Windows stubs ──────────────────────────────────────────────

#[cfg(not(target_os = "windows"))]
fn get_wsl2_status() -> String {
    json!({
        "detected": false,
        "available": false,
        "reason": "WSL2 is Windows-only",
        "distro": "--",
        "kernel": "--",
        "vm_memory_gb": 0.0,
        "current_usage_gb": 0.0,
        "max_memory_gb": 0.0,
        "host_ram_gb": 0.0,
        "pressure": "low",
        "auto_governor": false,
    })
    .to_string()
}

#[cfg(not(target_os = "windows"))]
fn get_wsl2_processes() -> String {
    json!({ "processes": [] }).to_string()
}

#[cfg(not(target_os = "windows"))]
fn set_wsl2_config(_payload: &Value) -> String {
    json!({
        "success": false,
        "error": "WSL2 is Windows-only. Configuration not available on this platform.",
    })
    .to_string()
}

#[cfg(not(target_os = "windows"))]
fn reclaim_wsl2_memory() -> String {
    json!({
        "success": false,
        "error": "WSL2 is Windows-only. Memory reclaim not available on this platform.",
    })
    .to_string()
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
    fn test_handle_ipc_status() {
        let result = handle_ipc("get_wsl2_status", &json!({}));
        assert!(result.is_some());
        let v: Value = serde_json::from_str(&result.unwrap()).unwrap();
        // On non-Windows or Windows without WSL2, detected may be false
        assert!(v.get("detected").is_some());
    }

    #[test]
    fn test_handle_ipc_processes() {
        let result = handle_ipc("get_wsl2_processes", &json!({}));
        assert!(result.is_some());
        let v: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(v["processes"].is_array());
    }

    #[test]
    fn test_round() {
        assert_eq!(round1(3.456), 3.5);
        assert_eq!(round2(3.456), 3.46);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_parse_size_gb() {
        assert_eq!(parse_size_gb("8GB"), Some(8.0));
        assert_eq!(parse_size_gb("512MB"), Some(0.5));
        assert_eq!(parse_size_gb("1TB"), Some(1024.0));
    }
}
