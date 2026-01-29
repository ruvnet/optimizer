//! Feature backends for Control Center pages (ADR-013 through ADR-025)
//!
//! Each sub-module exposes a single entry point:
//!
//! ```ignore
//! pub fn handle_ipc(msg_type: &str, payload: &serde_json::Value) -> Option<String>
//! ```
//!
//! Returning `Some(json_string)` when the message type is recognised, or
//! `None` to let the next module in the chain try.

use std::path::PathBuf;

// ADR-013 through ADR-015: full implementations
pub mod profiles;
pub mod health;
pub mod startup;

// ADR-016 through ADR-022: file-based modules (full or stub implementations)
pub mod wsl2;
pub mod build;
pub mod thermal;
pub mod plugins;

// ADR-018 through ADR-019: file-based stubs
pub mod leaks;

// ADR-022: GPU Memory Optimizer (file-based)
pub mod gpu;

// ADR-019: Predictive Prefetcher (full implementation)
pub mod prefetch;

// ADR-023, ADR-024, ADR-025 – full implementations
pub mod bloatware;
pub mod timeline;
pub mod agent;

// ── Cross-platform config directory ────────────────────────────────

/// Return the application config directory.
///
/// - **Windows**: `%APPDATA%\RuVector`
/// - **macOS**: `~/Library/Application Support/RuVector`
/// - **Linux**: `~/.config/ruvector`
pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let mut p = PathBuf::from(appdata);
            p.push("RuVector");
            return p;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let mut p = PathBuf::from(home);
            p.push("Library");
            p.push("Application Support");
            p.push("RuVector");
            return p;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            let mut p = PathBuf::from(xdg);
            p.push("ruvector");
            return p;
        }
        if let Some(home) = std::env::var_os("HOME") {
            let mut p = PathBuf::from(home);
            p.push(".config");
            p.push("ruvector");
            return p;
        }
    }

    // Fallback: current directory
    PathBuf::from(".")
}

/// Ensure the config directory (and an optional sub-directory) exists.
/// Returns the path to the requested directory.
pub fn ensure_config_dir(sub: Option<&str>) -> std::io::Result<PathBuf> {
    let mut dir = config_dir();
    if let Some(s) = sub {
        dir.push(s);
    }
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
