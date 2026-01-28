//! macOS Menu Bar Tray Application
//!
//! Provides a menu bar icon with memory status and optimization controls
//! similar to the Windows system tray, but using macOS conventions.

use crate::macos::memory::MacMemoryOptimizer;
use crate::accel::CpuCapabilities;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU32, Ordering}};
use std::process::Command;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, CheckMenuItem, Submenu, PredefinedMenuItem},
    TrayIconBuilder, Icon,
};

/// Auto-optimization threshold (optimize when memory usage exceeds this %)
pub const AUTO_OPTIMIZE_THRESHOLD: u32 = 75;
/// Auto-optimization interval in seconds
pub const AUTO_OPTIMIZE_INTERVAL: u64 = 60;

/// GitHub repository URL
const GITHUB_URL: &str = "https://github.com/ruvnet/optimizer";
/// Version string
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Tray settings for persistence
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TraySettings {
    pub threshold: u32,
    pub auto_optimize: bool,
    pub interval_secs: u64,
    pub ai_mode: AIModeSettings,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AIModeSettings {
    pub focus_mode: bool,
    pub thermal_prediction: bool,
    pub predictive_preload: bool,
}

impl Default for TraySettings {
    fn default() -> Self {
        Self {
            threshold: 80,
            auto_optimize: true,
            interval_secs: 60,
            ai_mode: AIModeSettings::default(),
        }
    }
}

impl TraySettings {
    fn config_path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        std::path::PathBuf::from(home)
            .join(".config")
            .join("ruvector-memopt")
            .join("tray-settings.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)
    }
}

pub struct MacTrayApp {
    running: Arc<AtomicBool>,
    settings: Arc<Mutex<TraySettings>>,
}

/// Holds tray icon state for lazy initialization
struct TrayState {
    tray_icon: tray_icon::TrayIcon,
    status_item: MenuItem,
    auto_item: CheckMenuItem,
    optimize_id: tray_icon::menu::MenuId,
    purge_id: tray_icon::menu::MenuId,
    app_id: tray_icon::menu::MenuId,
    cpu_id: tray_icon::menu::MenuId,
    activity_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
    auto_id: tray_icon::menu::MenuId,
    github_id: tray_icon::menu::MenuId,
    threshold_75_id: tray_icon::menu::MenuId,
    threshold_80_id: tray_icon::menu::MenuId,
    threshold_85_id: tray_icon::menu::MenuId,
    threshold_90_id: tray_icon::menu::MenuId,
    threshold_75: CheckMenuItem,
    threshold_80: CheckMenuItem,
    threshold_85: CheckMenuItem,
    threshold_90: CheckMenuItem,
}

impl MacTrayApp {
    pub fn new() -> Self {
        let settings = TraySettings::load();
        tracing::info!(
            "Loaded settings: threshold={}%, auto={}",
            settings.threshold,
            settings.auto_optimize
        );

        Self {
            running: Arc::new(AtomicBool::new(true)),
            settings: Arc::new(Mutex::new(settings)),
        }
    }

    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Note: On macOS, tray icon MUST be created AFTER event loop is running
        // See: https://docs.rs/tray-icon/latest/tray_icon/

        use winit::event_loop::{ControlFlow, EventLoop};
        use std::cell::RefCell;
        use std::rc::Rc;

        let event_loop = EventLoop::new()?;
        let initial_settings = self.settings.lock().unwrap().clone();

        let running = self.running.clone();
        let settings = self.settings.clone();

        // Shared state for lazy initialization
        let tray_state: Rc<RefCell<Option<TrayState>>> = Rc::new(RefCell::new(None));
        let tray_state_clone = tray_state.clone();

        let mut last_update = std::time::Instant::now();
        let mut last_auto_optimize = std::time::Instant::now();
        let auto_enabled = Arc::new(AtomicBool::new(initial_settings.auto_optimize));
        let current_threshold = Arc::new(AtomicU32::new(initial_settings.threshold));
        let initial_usage = MacMemoryOptimizer::get_memory_status()
            .map(|s| s.memory_load_percent)
            .unwrap_or(50);
        let last_usage = Arc::new(AtomicU32::new(initial_usage));
        let total_freed = Arc::new(AtomicU32::new(0));
        let settings_for_loop = initial_settings.clone();
        let mut initialized = false;

        // Run event loop
        #[allow(deprecated)]
        event_loop.run(move |_event, event_loop| {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(100),
            ));

            // Create tray icon on first iteration (AFTER event loop is running)
            if !initialized {
                initialized = true;

                // Get initial memory status
                let status_text = get_memory_status_text();

                // Create menu
                let menu = Menu::new();

                // Status section
                let status_item = MenuItem::new(&status_text, false, None);
                let version_item = MenuItem::new(&format!("v{} (macOS)", VERSION), false, None);
                let arch_item = MenuItem::new(
                    if cfg!(target_arch = "aarch64") {
                        "Apple Silicon"
                    } else {
                        "Intel"
                    },
                    false,
                    None,
                );

                // Main actions
                let auto_item = CheckMenuItem::new(
                    &format!("Auto-Optimize ({}s)", settings_for_loop.interval_secs),
                    true,
                    settings_for_loop.auto_optimize,
                    None,
                );
                let optimize_item = MenuItem::new("Optimize Now", true, None);
                let purge_item = MenuItem::new("Deep Clean (Purge)", true, None);
                let app_item = MenuItem::new("Optimize Apps", true, None);

                // Settings submenu
                let settings_menu = Submenu::new("Settings", true);
                let threshold_75 = CheckMenuItem::new("Threshold: 75%", true, settings_for_loop.threshold == 75, None);
                let threshold_80 = CheckMenuItem::new("Threshold: 80%", true, settings_for_loop.threshold == 80, None);
                let threshold_85 = CheckMenuItem::new("Threshold: 85%", true, settings_for_loop.threshold == 85, None);
                let threshold_90 = CheckMenuItem::new("Threshold: 90%", true, settings_for_loop.threshold == 90, None);
                let _ = settings_menu.append(&threshold_75);
                let _ = settings_menu.append(&threshold_80);
                let _ = settings_menu.append(&threshold_85);
                let _ = settings_menu.append(&threshold_90);

                // Info section
                let cpu_item = MenuItem::new("System Info", true, None);
                let activity_item = MenuItem::new("Open Activity Monitor", true, None);
                let github_item = MenuItem::new("GitHub Repository", true, None);
                let quit_item = MenuItem::new("Quit", true, None);

                // Build menu
                let _ = menu.append(&status_item);
                let _ = menu.append(&version_item);
                let _ = menu.append(&arch_item);
                let _ = menu.append(&PredefinedMenuItem::separator());
                let _ = menu.append(&auto_item);
                let _ = menu.append(&optimize_item);
                let _ = menu.append(&purge_item);
                let _ = menu.append(&app_item);
                let _ = menu.append(&PredefinedMenuItem::separator());
                let _ = menu.append(&settings_menu);
                let _ = menu.append(&PredefinedMenuItem::separator());
                let _ = menu.append(&cpu_item);
                let _ = menu.append(&activity_item);
                let _ = menu.append(&github_item);
                let _ = menu.append(&PredefinedMenuItem::separator());
                let _ = menu.append(&quit_item);

                // Clone IDs before moving items
                let optimize_id = optimize_item.id().clone();
                let purge_id = purge_item.id().clone();
                let app_id = app_item.id().clone();
                let cpu_id = cpu_item.id().clone();
                let activity_id = activity_item.id().clone();
                let quit_id = quit_item.id().clone();
                let auto_id = auto_item.id().clone();
                let github_id = github_item.id().clone();
                let threshold_75_id = threshold_75.id().clone();
                let threshold_80_id = threshold_80.id().clone();
                let threshold_85_id = threshold_85.id().clone();
                let threshold_90_id = threshold_90.id().clone();

                // Create tray icon (MUST be after event loop starts on macOS)
                let icon_data = create_icon_with_usage(initial_usage);
                if let Ok(icon) = Icon::from_rgba(icon_data, 32, 32) {
                    match TrayIconBuilder::new()
                        .with_menu(Box::new(menu))
                        .with_tooltip(&format!("RuVector MemOpt v{}", VERSION))
                        .with_icon(icon)
                        .build()
                    {
                        Ok(tray_icon) => {
                            let state = TrayState {
                                tray_icon,
                                status_item,
                                auto_item,
                                optimize_id,
                                purge_id,
                                app_id,
                                cpu_id,
                                activity_id,
                                quit_id,
                                auto_id,
                                github_id,
                                threshold_75_id,
                                threshold_80_id,
                                threshold_85_id,
                                threshold_90_id,
                                threshold_75,
                                threshold_80,
                                threshold_85,
                                threshold_90,
                            };
                            *tray_state_clone.borrow_mut() = Some(state);
                            tracing::info!("Tray icon created successfully");
                        }
                        Err(e) => {
                            tracing::error!("Failed to create tray icon: {}", e);
                        }
                    }
                }

                return;
            }

            // Get tray state reference
            let state_ref = tray_state.borrow();
            let Some(state) = state_ref.as_ref() else {
                return;
            };

            // Update status every 5 seconds
            if last_update.elapsed() > std::time::Duration::from_secs(5) {
                if let Ok(status) = MacMemoryOptimizer::get_memory_status() {
                    let usage = status.memory_load_percent;
                    last_usage.store(usage, Ordering::SeqCst);

                    // Update status text
                    let freed = total_freed.load(Ordering::SeqCst);
                    let threshold = current_threshold.load(Ordering::SeqCst);
                    let text = if freed > 0 {
                        format!(
                            "Memory: {}% ({:.1}/{:.1} GB) | Freed: {} MB",
                            usage,
                            status.used_physical_mb() / 1024.0,
                            status.total_physical_mb / 1024.0,
                            freed
                        )
                    } else {
                        format!(
                            "Memory: {}% ({:.1}/{:.1} GB)",
                            usage,
                            status.used_physical_mb() / 1024.0,
                            status.total_physical_mb / 1024.0
                        )
                    };
                    let _ = state.status_item.set_text(&text);

                    // Update icon color
                    let icon_data = create_icon_with_usage(usage);
                    if let Ok(new_icon) = Icon::from_rgba(icon_data, 32, 32) {
                        let _ = state.tray_icon.set_icon(Some(new_icon));
                    }

                    // Update tooltip
                    let tooltip = if auto_enabled.load(Ordering::SeqCst) {
                        format!("RuVector v{} - {}% | Auto @{}%", VERSION, usage, threshold)
                    } else {
                        format!("RuVector v{} - {}% | Manual", VERSION, usage)
                    };
                    let _ = state.tray_icon.set_tooltip(Some(tooltip));

                    // Auto-optimize if enabled
                    if auto_enabled.load(Ordering::SeqCst)
                        && usage > threshold
                        && last_auto_optimize.elapsed()
                            > std::time::Duration::from_secs(AUTO_OPTIMIZE_INTERVAL)
                    {
                        let total_freed_clone = total_freed.clone();
                        std::thread::spawn(move || {
                            let optimizer = MacMemoryOptimizer::new();
                            if let Ok(result) = optimizer.optimize(false) {
                                if result.freed_mb > 50.0 {
                                    let current = total_freed_clone.load(Ordering::SeqCst);
                                    total_freed_clone
                                        .store(current + result.freed_mb as u32, Ordering::SeqCst);
                                }
                            }
                        });
                        last_auto_optimize = std::time::Instant::now();
                    }
                }
                last_update = std::time::Instant::now();
            }

            // Handle menu events
            if let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id == state.quit_id {
                    running.store(false, Ordering::SeqCst);
                    event_loop.exit();
                } else if event.id == state.optimize_id {
                    let total_freed_clone = total_freed.clone();
                    run_optimization(false, total_freed_clone);
                } else if event.id == state.purge_id {
                    let total_freed_clone = total_freed.clone();
                    run_optimization(true, total_freed_clone);
                } else if event.id == state.app_id {
                    let total_freed_clone = total_freed.clone();
                    run_app_optimization(total_freed_clone);
                } else if event.id == state.cpu_id {
                    show_system_info();
                } else if event.id == state.activity_id {
                    open_activity_monitor();
                } else if event.id == state.github_id {
                    open_github();
                } else if event.id == state.auto_id {
                    let current = auto_enabled.load(Ordering::SeqCst);
                    let new_val = !current;
                    auto_enabled.store(new_val, Ordering::SeqCst);
                    let _ = state.auto_item.set_checked(new_val);
                    if let Ok(mut s) = settings.lock() {
                        s.auto_optimize = new_val;
                        let _ = s.save();
                    }
                } else if event.id == state.threshold_75_id {
                    current_threshold.store(75, Ordering::SeqCst);
                    let _ = state.threshold_75.set_checked(true);
                    let _ = state.threshold_80.set_checked(false);
                    let _ = state.threshold_85.set_checked(false);
                    let _ = state.threshold_90.set_checked(false);
                    if let Ok(mut s) = settings.lock() {
                        s.threshold = 75;
                        let _ = s.save();
                    }
                } else if event.id == state.threshold_80_id {
                    current_threshold.store(80, Ordering::SeqCst);
                    let _ = state.threshold_75.set_checked(false);
                    let _ = state.threshold_80.set_checked(true);
                    let _ = state.threshold_85.set_checked(false);
                    let _ = state.threshold_90.set_checked(false);
                    if let Ok(mut s) = settings.lock() {
                        s.threshold = 80;
                        let _ = s.save();
                    }
                } else if event.id == state.threshold_85_id {
                    current_threshold.store(85, Ordering::SeqCst);
                    let _ = state.threshold_75.set_checked(false);
                    let _ = state.threshold_80.set_checked(false);
                    let _ = state.threshold_85.set_checked(true);
                    let _ = state.threshold_90.set_checked(false);
                    if let Ok(mut s) = settings.lock() {
                        s.threshold = 85;
                        let _ = s.save();
                    }
                } else if event.id == state.threshold_90_id {
                    current_threshold.store(90, Ordering::SeqCst);
                    let _ = state.threshold_75.set_checked(false);
                    let _ = state.threshold_80.set_checked(false);
                    let _ = state.threshold_85.set_checked(false);
                    let _ = state.threshold_90.set_checked(true);
                    if let Ok(mut s) = settings.lock() {
                        s.threshold = 90;
                        let _ = s.save();
                    }
                }
            }
        })?;

        Ok(())
    }
}

fn get_memory_status_text() -> String {
    if let Ok(status) = MacMemoryOptimizer::get_memory_status() {
        format!(
            "Memory: {}% ({:.1}/{:.1} GB)",
            status.memory_load_percent,
            status.used_physical_mb() / 1024.0,
            status.total_physical_mb / 1024.0
        )
    } else {
        "Memory: Unknown".to_string()
    }
}

fn run_optimization(aggressive: bool, total_freed: Arc<AtomicU32>) {
    std::thread::spawn(move || {
        let optimizer = MacMemoryOptimizer::new();
        match optimizer.optimize(aggressive) {
            Ok(result) => {
                let current = total_freed.load(Ordering::SeqCst);
                total_freed.store(current + result.freed_mb as u32, Ordering::SeqCst);

                let msg = format!(
                    "Optimization Complete!\n\nMethod: {:?}\nFreed: {:.1} MB\nProcesses: {}\nTime: {} ms",
                    result.method, result.freed_mb, result.processes_affected, result.duration_ms
                );
                show_notification("RuVector Optimizer", &msg);
            }
            Err(e) => {
                show_notification("RuVector Optimizer", &format!("Error: {}", e));
            }
        }
    });
}

fn run_app_optimization(total_freed: Arc<AtomicU32>) {
    std::thread::spawn(move || {
        // macOS app patterns
        const APP_PATTERNS: &[(&str, &[&str])] = &[
            ("Safari", &["safari"]),
            ("Chrome", &["google chrome", "chrome"]),
            ("Firefox", &["firefox"]),
            ("Arc", &["arc"]),
            ("Brave", &["brave browser"]),
            ("VSCode", &["code", "code helper"]),
            ("Electron", &["electron"]),
            ("Discord", &["discord"]),
            ("Slack", &["slack"]),
            ("Spotify", &["spotify"]),
            ("Teams", &["microsoft teams"]),
            ("Zoom", &["zoom.us"]),
        ];

        use sysinfo::{System, ProcessesToUpdate, UpdateKind};
        let mut system = System::new_all();
        system.refresh_processes(ProcessesToUpdate::All, true);

        let mut apps_found = Vec::new();
        let mut total_mem = 0.0f64;

        for (pid, process) in system.processes() {
            let name = process.name().to_string_lossy().to_lowercase();
            for (app_name, patterns) in APP_PATTERNS {
                if patterns.iter().any(|p| name.contains(p)) {
                    let mem_mb = process.memory() as f64 / 1024.0 / 1024.0;
                    apps_found.push((*app_name, pid.as_u32(), mem_mb));
                    total_mem += mem_mb;
                    break;
                }
            }
        }

        if apps_found.is_empty() {
            show_notification(
                "RuVector - App Optimizer",
                "No browsers or Electron apps detected.",
            );
            return;
        }

        // On macOS, we can't directly trim other process memory
        // Instead, we can suggest using purge or show the user what's consuming memory
        let mut msg = format!(
            "Detected {} app processes using {:.0} MB:\n\n",
            apps_found.len(),
            total_mem
        );

        // Group by app name
        use std::collections::HashMap;
        let mut by_app: HashMap<&str, (usize, f64)> = HashMap::new();
        for (app, _pid, mem) in &apps_found {
            let entry = by_app.entry(*app).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += mem;
        }

        for (app, (count, mem)) in by_app {
            msg.push_str(&format!("  {} - {:.0} MB ({} proc)\n", app, mem, count));
        }

        msg.push_str("\nTip: Use 'Deep Clean' to run purge command.");

        // Attempt optimization via purge
        let optimizer = MacMemoryOptimizer::new();
        if optimizer.has_sudo_privileges() {
            if let Ok(result) = optimizer.optimize(true) {
                let current = total_freed.load(Ordering::SeqCst);
                total_freed.store(current + result.freed_mb as u32, Ordering::SeqCst);
                msg.push_str(&format!("\n\nFreed: {:.0} MB", result.freed_mb));
            }
        }

        show_notification("RuVector - App Optimizer", &msg);
    });
}

fn show_system_info() {
    let caps = CpuCapabilities::detect();
    let optimizer = MacMemoryOptimizer::new();

    let arch = if cfg!(target_arch = "aarch64") {
        "Apple Silicon"
    } else {
        "Intel x86_64"
    };

    let msg = format!(
        "RuVector Memory Optimizer v{}\n\n\
        Architecture: {}\n\
        CPU: {}\n\
        Cores: {}\n\n\
        Sudo Access: {}\n\n\
        SIMD: {}\n\
        NEON: {}\n\n\
        GitHub: {}",
        VERSION,
        arch,
        caps.model,
        caps.core_count,
        if optimizer.has_sudo_privileges() { "Yes" } else { "No" },
        if caps.has_avx2 { "AVX2" } else { "Basic" },
        if cfg!(target_arch = "aarch64") { "Yes" } else { "N/A" },
        GITHUB_URL
    );

    show_notification("System Information", &msg);
}

fn open_activity_monitor() {
    let _ = Command::new("open")
        .arg("-a")
        .arg("Activity Monitor")
        .spawn();
}

fn open_github() {
    let _ = Command::new("open").arg(GITHUB_URL).spawn();
}

fn show_notification(title: &str, message: &str) {
    // Use osascript to show a notification
    let script = format!(
        r#"display notification "{}" with title "{}""#,
        message.replace('"', r#"\""#).replace('\n', r#"\n"#),
        title
    );

    let _ = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .spawn();

    // Also print to console for debugging
    println!("\n{}\n{}\n", title, message);
}

/// Create icon with usage percentage color coding
fn create_icon_with_usage(usage_percent: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity(32 * 32 * 4);

    // Color based on memory usage
    let (r, g, b) = if usage_percent < 60 {
        (0x00u8, 0xC8u8, 0x50u8) // Green
    } else if usage_percent < 80 {
        (0xFFu8, 0xA5u8, 0x00u8) // Orange
    } else {
        (0xE0u8, 0x30u8, 0x30u8) // Red
    };

    let (border_r, border_g, border_b) = if usage_percent < 60 {
        (0x00u8, 0x80u8, 0x30u8)
    } else if usage_percent < 80 {
        (0xCCu8, 0x80u8, 0x00u8)
    } else {
        (0xA0u8, 0x20u8, 0x20u8)
    };

    for y in 0..32 {
        for x in 0..32 {
            // Memory chip shape
            let in_body = x >= 4 && x < 28 && y >= 2 && y < 30;
            let in_notch = x >= 12 && x < 20 && y < 4;
            let in_chip = in_body && !in_notch;

            let left_pin = x < 4 && (y == 8 || y == 14 || y == 20 || y == 26);
            let right_pin = x >= 28 && (y == 8 || y == 14 || y == 20 || y == 26);
            let is_pin = left_pin || right_pin;

            let is_border = in_chip
                && (x == 4 || x == 27 || y == 2 || y == 29 || (y == 3 && (x < 12 || x >= 20)));

            let fill_height = 28 - ((usage_percent as i32 * 26) / 100);
            let is_filled = in_chip && !is_border && (y as i32) >= fill_height;

            if is_pin {
                data.extend_from_slice(&[border_r, border_g, border_b, 0xFF]);
            } else if is_border {
                data.extend_from_slice(&[border_r, border_g, border_b, 0xFF]);
            } else if is_filled {
                data.extend_from_slice(&[r, g, b, 0xFF]);
            } else if in_chip {
                data.extend_from_slice(&[r / 3, g / 3, b / 3, 0xFF]);
            } else {
                data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
            }
        }
    }

    data
}

impl Default for MacTrayApp {
    fn default() -> Self {
        Self::new()
    }
}
