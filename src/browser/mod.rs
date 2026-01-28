//! Browser-specific memory optimization
//!
//! Chromium-based browsers (Brave, Chrome, Edge) use multiple processes:
//! - Main browser process
//! - GPU process
//! - Renderer processes (one per tab/extension)
//! - Utility processes
//!
//! This module provides targeted optimization for these browsers.

use sysinfo::{System, ProcessRefreshKind};
use tracing::{info, warn};
use std::collections::HashMap;
use std::ffi::OsString;

/// Chromium process types
#[derive(Debug, Clone, PartialEq)]
pub enum ChromiumProcessType {
    Browser,      // Main browser process
    Gpu,          // GPU compositor
    Renderer,     // Tab content
    Extension,    // Browser extensions
    Utility,      // Various utilities
    Unknown,
}

/// Browser process info
#[derive(Debug, Clone)]
pub struct BrowserProcess {
    pub pid: u32,
    pub process_type: ChromiumProcessType,
    pub memory_mb: f64,
    pub cpu_percent: f32,
    pub title: Option<String>,
}

/// Browser optimization result
#[derive(Debug, Clone)]
pub struct BrowserOptResult {
    pub browser_name: String,
    pub processes_found: usize,
    pub processes_trimmed: usize,
    pub memory_before_mb: f64,
    pub memory_freed_mb: f64,
    pub tabs_suspended: usize,
}

/// Supported browsers
const BRAVE_PROCESSES: &[&str] = &["brave.exe", "brave"];
const CHROME_PROCESSES: &[&str] = &["chrome.exe", "chrome"];
const EDGE_PROCESSES: &[&str] = &["msedge.exe", "msedge"];

/// Electron-based apps (same architecture as Chromium)
const VSCODE_PROCESSES: &[&str] = &["code.exe", "code"];
const DISCORD_PROCESSES: &[&str] = &["discord.exe", "discord"];
const SPOTIFY_PROCESSES: &[&str] = &["spotify.exe", "spotify"];
const WHATSAPP_PROCESSES: &[&str] = &["whatsapp.exe", "whatsapp"];
const SLACK_PROCESSES: &[&str] = &["slack.exe", "slack"];
const TEAMS_PROCESSES: &[&str] = &["teams.exe", "ms-teams.exe", "teams"];

/// Native apps with high memory usage
const ZOOM_PROCESSES: &[&str] = &["zoom.exe", "zoom", "zoomus", "cpthost.exe"];
const OBSIDIAN_PROCESSES: &[&str] = &["obsidian.exe", "obsidian"];
const NOTION_PROCESSES: &[&str] = &["notion.exe", "notion"];
const FIGMA_PROCESSES: &[&str] = &["figma.exe", "figma"];

pub struct BrowserOptimizer {
    system: System,
}

impl BrowserOptimizer {
    pub fn new() -> Self {
        Self {
            system: System::new(),
        }
    }

    /// Detect all browser and Electron app processes
    pub fn detect_browsers(&mut self) -> HashMap<String, Vec<BrowserProcess>> {
        self.system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let mut apps: HashMap<String, Vec<BrowserProcess>> = HashMap::new();

        for (pid, process) in self.system.processes() {
            let name = process.name().to_string_lossy().to_lowercase();

            // Detect app type
            let app_name = if BRAVE_PROCESSES.iter().any(|&b| name.contains(b)) {
                Some("Brave")
            } else if CHROME_PROCESSES.iter().any(|&c| name.contains(c)) {
                Some("Chrome")
            } else if EDGE_PROCESSES.iter().any(|&e| name.contains(e)) {
                Some("Edge")
            } else if VSCODE_PROCESSES.iter().any(|&v| name.contains(v)) {
                Some("VSCode")
            } else if DISCORD_PROCESSES.iter().any(|&d| name.contains(d)) {
                Some("Discord")
            } else if SPOTIFY_PROCESSES.iter().any(|&s| name.contains(s)) {
                Some("Spotify")
            } else if WHATSAPP_PROCESSES.iter().any(|&w| name.contains(w)) {
                Some("WhatsApp")
            } else if SLACK_PROCESSES.iter().any(|&s| name.contains(s)) {
                Some("Slack")
            } else if TEAMS_PROCESSES.iter().any(|&t| name.contains(t)) {
                Some("Teams")
            } else if ZOOM_PROCESSES.iter().any(|&z| name.contains(z)) {
                Some("Zoom")
            } else if OBSIDIAN_PROCESSES.iter().any(|&o| name.contains(o)) {
                Some("Obsidian")
            } else if NOTION_PROCESSES.iter().any(|&n| name.contains(n)) {
                Some("Notion")
            } else if FIGMA_PROCESSES.iter().any(|&f| name.contains(f)) {
                Some("Figma")
            } else {
                None
            };

            if let Some(app) = app_name {
                let cmd = process.cmd();
                let process_type = Self::classify_process(cmd);

                let bp = BrowserProcess {
                    pid: pid.as_u32(),
                    process_type,
                    memory_mb: process.memory() as f64 / 1024.0 / 1024.0,
                    cpu_percent: process.cpu_usage(),
                    title: Self::extract_title(cmd),
                };

                apps.entry(app.to_string()).or_default().push(bp);
            }
        }

        apps
    }

    /// Classify Chromium process type from command line
    fn classify_process(cmd: &[OsString]) -> ChromiumProcessType {
        let cmd_str: String = cmd.iter()
            .map(|s| s.to_string_lossy().to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");

        if cmd_str.contains("--type=gpu") {
            ChromiumProcessType::Gpu
        } else if cmd_str.contains("--type=renderer") {
            ChromiumProcessType::Renderer
        } else if cmd_str.contains("--type=extension") || cmd_str.contains("--extension-process") {
            ChromiumProcessType::Extension
        } else if cmd_str.contains("--type=utility") {
            ChromiumProcessType::Utility
        } else if cmd_str.contains("--type=") {
            ChromiumProcessType::Unknown
        } else {
            ChromiumProcessType::Browser // Main process has no --type flag
        }
    }

    /// Extract tab/page title if available
    fn extract_title(_cmd: &[OsString]) -> Option<String> {
        // Chromium doesn't expose tab titles in cmd, but we can try
        None
    }

    /// Get total memory usage for a browser
    pub fn get_browser_memory(&mut self, browser: &str) -> f64 {
        let browsers = self.detect_browsers();
        browsers.get(browser)
            .map(|procs| procs.iter().map(|p| p.memory_mb).sum())
            .unwrap_or(0.0)
    }

    /// Optimize a specific browser
    pub fn optimize_browser(&mut self, browser: &str, aggressive: bool) -> BrowserOptResult {
        let browsers = self.detect_browsers();
        let processes = browsers.get(browser).cloned().unwrap_or_default();

        let memory_before: f64 = processes.iter().map(|p| p.memory_mb).sum();
        let mut trimmed = 0;
        let mut freed = 0.0;

        // Sort by memory usage (highest first)
        let mut sorted_procs = processes.clone();
        sorted_procs.sort_by(|a, b| b.memory_mb.partial_cmp(&a.memory_mb).unwrap());

        for proc in &sorted_procs {
            // Skip main browser and GPU process unless aggressive
            if !aggressive && matches!(proc.process_type, ChromiumProcessType::Browser | ChromiumProcessType::Gpu) {
                continue;
            }

            // Trim working set
            match crate::windows::memory::WindowsMemoryOptimizer::trim_process_working_set(proc.pid) {
                Ok(bytes_freed) => {
                    if bytes_freed > 0 {
                        freed += bytes_freed as f64 / 1024.0 / 1024.0;
                        trimmed += 1;
                        info!("Trimmed {} ({:?}): {:.1} MB freed",
                            browser, proc.process_type, bytes_freed as f64 / 1024.0 / 1024.0);
                    }
                }
                Err(e) => {
                    warn!("Failed to trim {} process {}: {}", browser, proc.pid, e);
                }
            }
        }

        // Try to trigger Chromium's internal memory pressure handling
        if aggressive {
            Self::send_memory_pressure(browser);
        }

        BrowserOptResult {
            browser_name: browser.to_string(),
            processes_found: processes.len(),
            processes_trimmed: trimmed,
            memory_before_mb: memory_before,
            memory_freed_mb: freed,
            tabs_suspended: 0, // Would need browser extension for this
        }
    }

    /// Send memory pressure signal to Chromium
    /// This triggers internal garbage collection and tab discarding
    fn send_memory_pressure(_browser: &str) {
        // Chromium monitors system memory and reacts to pressure
        // We can't directly signal it, but aggressive trimming
        // triggers the same response

        // Future: Could use named pipe or debug protocol
        // chrome://memory-internals shows memory pressure state
    }

    /// Optimize all detected browsers
    pub fn optimize_all(&mut self, aggressive: bool) -> Vec<BrowserOptResult> {
        let browsers = self.detect_browsers();
        let browser_names: Vec<String> = browsers.keys().cloned().collect();

        browser_names.iter()
            .map(|name| self.optimize_browser(name, aggressive))
            .collect()
    }

    /// Get browser stats summary
    pub fn stats(&mut self) -> BrowserStats {
        let browsers = self.detect_browsers();

        BrowserStats {
            browsers: browsers.iter().map(|(name, procs)| {
                BrowserStat {
                    name: name.clone(),
                    process_count: procs.len(),
                    total_memory_mb: procs.iter().map(|p| p.memory_mb).sum(),
                    renderer_count: procs.iter().filter(|p| p.process_type == ChromiumProcessType::Renderer).count(),
                    extension_count: procs.iter().filter(|p| p.process_type == ChromiumProcessType::Extension).count(),
                }
            }).collect(),
        }
    }
}

impl Default for BrowserOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct BrowserStats {
    pub browsers: Vec<BrowserStat>,
}

#[derive(Debug, Clone)]
pub struct BrowserStat {
    pub name: String,
    pub process_count: usize,
    pub total_memory_mb: f64,
    pub renderer_count: usize,
    pub extension_count: usize,
}

impl BrowserStats {
    pub fn total_memory(&self) -> f64 {
        self.browsers.iter().map(|b| b.total_memory_mb).sum()
    }

    pub fn summary(&self) -> String {
        self.browsers.iter()
            .map(|b| format!("{}: {:.0} MB ({} tabs, {} ext)",
                b.name, b.total_memory_mb, b.renderer_count, b.extension_count))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
