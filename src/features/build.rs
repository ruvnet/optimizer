//! ADR-017: Build Environment Optimizer
//!
//! Detects active build tools on the system, tracks build history,
//! manages caches, and provides optimization recommendations.
//!
//! Cross-platform: works on Windows, macOS, and Linux.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Handle IPC messages for the build environment page.
///
/// Recognised message types:
/// - `get_build_tools`   – detect installed build tools with versions and cache sizes
/// - `get_build_history` – return recent build history
/// - `set_build_config`  – configure parallel jobs, memory limits, etc.
/// - `optimize_builds`   – apply optimization recommendations
/// - `clear_cache`       – clear cache for a specific tool
pub fn handle_ipc(msg_type: &str, payload: &Value) -> Option<String> {
    match msg_type {
        "get_build_tools" => Some(get_build_tools()),
        "get_build_history" => Some(get_build_history()),
        "set_build_config" => Some(set_build_config(payload)),
        "optimize_builds" => Some(optimize_builds()),
        "clear_cache" => Some(clear_cache(payload)),
        _ => None,
    }
}

// ── Build tool descriptor ──────────────────────────────────────────

struct ToolDef {
    name: &'static str,
    icon: &'static str,
    binary: &'static str,
    version_arg: &'static str,
    color: &'static str,
    cache_dirs: &'static [&'static str],
}

const TOOL_DEFS: &[ToolDef] = &[
    ToolDef {
        name: "Cargo (Rust)",
        icon: "\u{1F980}",
        binary: "cargo",
        version_arg: "--version",
        color: "var(--accent-amber)",
        cache_dirs: &["target"],
    },
    ToolDef {
        name: "npm / Node.js",
        icon: "\u{1F4E6}",
        binary: "npm",
        version_arg: "--version",
        color: "var(--accent-green)",
        cache_dirs: &["node_modules/.cache"],
    },
    ToolDef {
        name: "yarn",
        icon: "\u{1F9F6}",
        binary: "yarn",
        version_arg: "--version",
        color: "var(--accent-cyan)",
        cache_dirs: &[],
    },
    ToolDef {
        name: "pnpm",
        icon: "\u{1F4E6}",
        binary: "pnpm",
        version_arg: "--version",
        color: "var(--accent-purple)",
        cache_dirs: &[],
    },
    ToolDef {
        name: "pip (Python)",
        icon: "\u{1F40D}",
        binary: "pip",
        version_arg: "--version",
        color: "var(--accent-amber)",
        cache_dirs: &["__pycache__"],
    },
    ToolDef {
        name: "Go",
        icon: "\u{1F439}",
        binary: "go",
        version_arg: "version",
        color: "var(--accent-cyan)",
        cache_dirs: &[],
    },
    ToolDef {
        name: "dotnet",
        icon: "\u{1F7E3}",
        binary: "dotnet",
        version_arg: "--version",
        color: "var(--accent-purple)",
        cache_dirs: &[],
    },
    ToolDef {
        name: "gradle",
        icon: "\u{1F418}",
        binary: "gradle",
        version_arg: "--version",
        color: "var(--accent-green)",
        cache_dirs: &[".gradle"],
    },
    ToolDef {
        name: "maven",
        icon: "\u{1F426}",
        binary: "mvn",
        version_arg: "--version",
        color: "var(--accent-red)",
        cache_dirs: &[],
    },
    ToolDef {
        name: "cmake",
        icon: "\u{1F527}",
        binary: "cmake",
        version_arg: "--version",
        color: "var(--accent-cyan)",
        cache_dirs: &["build"],
    },
    ToolDef {
        name: "make",
        icon: "\u{1F6E0}",
        binary: "make",
        version_arg: "--version",
        color: "var(--accent-amber)",
        cache_dirs: &[],
    },
    ToolDef {
        name: "Docker",
        icon: "\u{1F433}",
        binary: "docker",
        version_arg: "--version",
        color: "var(--accent-cyan)",
        cache_dirs: &[],
    },
];

// ── IPC handlers ───────────────────────────────────────────────────

fn get_build_tools() -> String {
    let cpu_count = num_cpus::get();
    let mut tools: Vec<Value> = Vec::new();

    for def in TOOL_DEFS {
        let version = detect_tool_version(def.binary, def.version_arg);
        let status = if version.is_some() { "detected" } else { "unknown" };

        let (cache_size_str, cache_bytes) = if version.is_some() {
            estimate_cache_size(def)
        } else {
            ("--".to_string(), 0u64)
        };

        tools.push(json!({
            "name": def.name,
            "icon": def.icon,
            "version": version.as_deref().unwrap_or("--"),
            "cacheSize": cache_size_str,
            "cacheBytes": cache_bytes,
            "status": status,
            "color": def.color,
        }));
    }

    json!({
        "tools": tools,
        "max_threads": cpu_count,
        "active_build": Value::Null,
        "recommendations": default_recommendations(&tools),
    })
    .to_string()
}

fn get_build_history() -> String {
    let history = load_build_history();
    json!({ "history": history }).to_string()
}

fn set_build_config(payload: &Value) -> String {
    // Persist build configuration
    let config = json!({
        "threads": payload.get("threads").and_then(|v| v.as_u64()).unwrap_or(4),
        "memory_limit_mb": payload.get("memory_limit_mb").and_then(|v| v.as_u64()).unwrap_or(8192),
        "cpu_priority": payload.get("cpu_priority").and_then(|v| v.as_str()).unwrap_or("normal"),
        "io_priority": payload.get("io_priority").and_then(|v| v.as_str()).unwrap_or("normal"),
        "tmpfs_enabled": payload.get("tmpfs_enabled").and_then(|v| v.as_bool()).unwrap_or(false),
    });

    if let Err(e) = save_build_config(&config) {
        tracing::error!("Failed to save build config: {}", e);
        return json!({ "success": false, "error": format!("{}", e) }).to_string();
    }

    json!({ "success": true, "message": "Build configuration saved." }).to_string()
}

fn optimize_builds() -> String {
    let cpu_count = num_cpus::get();

    // Generate optimisation suggestions based on detected tools
    let config = json!({
        "threads": cpu_count,
        "memory_limit_mb": estimate_optimal_memory_limit(),
        "cpu_priority": "high",
        "io_priority": "normal",
        "tmpfs_enabled": false,
        "incremental": true,
    });

    if let Err(e) = save_build_config(&config) {
        tracing::error!("Failed to save optimized config: {}", e);
    }

    json!({
        "success": true,
        "message": format!("Optimized for {} parallel threads", cpu_count),
        "config": config,
    })
    .to_string()
}

fn clear_cache(payload: &Value) -> String {
    let tool_name = match payload.get("tool").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => {
            return json!({ "success": false, "error": "No tool specified" }).to_string();
        }
    };

    // Find the tool definition
    let def = TOOL_DEFS.iter().find(|d| d.name == tool_name);
    let def = match def {
        Some(d) => d,
        None => {
            return json!({ "success": false, "error": format!("Unknown tool: {}", tool_name) })
                .to_string();
        }
    };

    if def.cache_dirs.is_empty() {
        return json!({
            "success": true,
            "freed_mb": 0.0,
            "message": format!("No known cache directory for {}", tool_name),
        })
        .to_string();
    }

    let mut total_freed: u64 = 0;
    let cwd = std::env::current_dir().unwrap_or_default();

    for cache_dir in def.cache_dirs {
        let path = cwd.join(cache_dir);
        if path.exists() && path.is_dir() {
            let size = dir_size_bytes(&path);
            if let Err(e) = std::fs::remove_dir_all(&path) {
                tracing::warn!("Failed to remove {}: {}", path.display(), e);
            } else {
                total_freed += size;
                tracing::info!("Cleared cache: {} ({} MB)", path.display(), size / (1024 * 1024));
            }
        }
    }

    let freed_mb = total_freed as f64 / (1024.0 * 1024.0);
    json!({
        "success": true,
        "freed_mb": (freed_mb * 10.0).round() / 10.0,
        "tool": tool_name,
    })
    .to_string()
}

// ── Tool detection ─────────────────────────────────────────────────

fn detect_tool_version(binary: &str, version_arg: &str) -> Option<String> {
    let output = std::process::Command::new(binary)
        .arg(version_arg)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Extract first line and trim to a reasonable length
    let first_line = stdout.lines().next().unwrap_or("").trim();

    // Try to extract just the version number
    let version = extract_version_number(first_line);
    Some(version)
}

fn extract_version_number(line: &str) -> String {
    // Look for patterns like "1.2.3" in the output
    for word in line.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
        if cleaned.contains('.') && cleaned.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            return cleaned.to_string();
        }
    }
    // Fallback: return first 30 chars of line
    if line.len() > 30 {
        format!("{}...", &line[..30])
    } else {
        line.to_string()
    }
}

fn estimate_cache_size(def: &ToolDef) -> (String, u64) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut total: u64 = 0;

    for cache_dir in def.cache_dirs {
        let path = cwd.join(cache_dir);
        if path.exists() && path.is_dir() {
            total += dir_size_bytes(&path);
        }
    }

    if total == 0 {
        return ("0 MB".to_string(), 0);
    }

    let mb = total as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        (format!("{:.1} GB", mb / 1024.0), total)
    } else {
        (format!("{:.0} MB", mb), total)
    }
}

fn dir_size_bytes(path: &Path) -> u64 {
    let mut total: u64 = 0;
    let walker = walkdir(path);
    for entry in walker {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                total += meta.len();
            }
        }
    }
    total
}

/// Simple recursive directory walk without extra dependencies.
fn walkdir(root: &Path) -> Vec<DirEntry> {
    let mut entries = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let read_dir = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_dir() {
                stack.push(path.clone());
            }
            entries.push(DirEntry { path, meta });
        }
    }
    entries
}

struct DirEntry {
    #[allow(dead_code)]
    path: PathBuf,
    meta: std::fs::Metadata,
}

impl DirEntry {
    fn metadata(&self) -> std::io::Result<&std::fs::Metadata> {
        Ok(&self.meta)
    }
}

// ── Build history ──────────────────────────────────────────────────

fn history_path() -> PathBuf {
    let dir = super::config_dir();
    dir.join("build_history.json")
}

fn load_build_history() -> Vec<Value> {
    let path = history_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<Vec<Value>>(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

#[allow(dead_code)]
fn save_build_history(history: &[Value]) -> std::io::Result<()> {
    if let Ok(dir) = super::ensure_config_dir(None) {
        let path = dir.join("build_history.json");
        // Keep only last 50
        let trimmed: Vec<&Value> = history.iter().rev().take(50).collect();
        let json = serde_json::to_string_pretty(&trimmed).unwrap_or_default();
        std::fs::write(&path, json)?;
    }
    Ok(())
}

// ── Build config ───────────────────────────────────────────────────

fn config_path() -> PathBuf {
    let dir = super::config_dir();
    dir.join("build_config.json")
}

fn save_build_config(config: &Value) -> std::io::Result<()> {
    let dir = super::ensure_config_dir(None)?;
    let path = dir.join("build_config.json");
    let json = serde_json::to_string_pretty(config).unwrap_or_default();
    std::fs::write(&path, json)
}

#[allow(dead_code)]
fn load_build_config() -> Option<Value> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn estimate_optimal_memory_limit() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let total_mb = sys.total_memory() / (1024 * 1024);
    // Use 75% of total for build processes
    (total_mb as f64 * 0.75) as u64
}

// ── Recommendations ────────────────────────────────────────────────

fn default_recommendations(tools: &[Value]) -> Vec<Value> {
    let mut recs = Vec::new();
    let detected: HashMap<&str, bool> = tools
        .iter()
        .map(|t| {
            (
                t["name"].as_str().unwrap_or(""),
                t["status"].as_str() == Some("detected"),
            )
        })
        .collect();

    if detected.get("Cargo (Rust)").copied().unwrap_or(false) {
        recs.push(json!({
            "text": "Enable incremental builds for Cargo",
            "priority": "high",
            "applied": false,
        }));
        recs.push(json!({
            "text": "Use sccache for shared compilation cache",
            "priority": "high",
            "applied": false,
        }));
    }

    if detected.get("npm / Node.js").copied().unwrap_or(false) {
        recs.push(json!({
            "text": "Configure npm cache to local SSD",
            "priority": "medium",
            "applied": false,
        }));
    }

    if detected.get("Docker").copied().unwrap_or(false) {
        recs.push(json!({
            "text": "Set Docker BuildKit for parallel layers",
            "priority": "medium",
            "applied": false,
        }));
    }

    recs.push(json!({
        "text": format!("Set parallel jobs to {} (CPU cores)", num_cpus::get()),
        "priority": "high",
        "applied": false,
    }));

    recs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_ipc_unknown() {
        assert!(handle_ipc("unknown_msg", &json!({})).is_none());
    }

    #[test]
    fn test_handle_ipc_build_tools() {
        let result = handle_ipc("get_build_tools", &json!({}));
        assert!(result.is_some());
        let v: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(v["tools"].is_array());
        assert!(v["max_threads"].is_number());
    }

    #[test]
    fn test_handle_ipc_build_history() {
        let result = handle_ipc("get_build_history", &json!({}));
        assert!(result.is_some());
        let v: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(v["history"].is_array());
    }

    #[test]
    fn test_extract_version_number() {
        assert_eq!(extract_version_number("cargo 1.77.0 (3fe68eaab 2024-02-29)"), "1.77.0");
        assert_eq!(extract_version_number("npm 10.5.0"), "10.5.0");
        assert_eq!(extract_version_number("go version go1.22.1 linux/amd64"), "1.22.1");
    }

    #[test]
    fn test_clear_cache_no_tool() {
        let result = handle_ipc("clear_cache", &json!({}));
        assert!(result.is_some());
        let v: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(v["success"].as_bool(), Some(false));
    }
}
