//! System tray icon and menu with automatic optimization and AI Mode settings

use crate::windows::memory::WindowsMemoryOptimizer;
use crate::accel::CpuCapabilities;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU32, Ordering}};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, CheckMenuItem, Submenu, PredefinedMenuItem},
    TrayIconBuilder, Icon,
};
use winit::event_loop::{ControlFlow, EventLoop};

/// Auto-optimization threshold (optimize when memory usage exceeds this %)
pub const AUTO_OPTIMIZE_THRESHOLD: u32 = 75;
/// Auto-optimization interval in seconds
pub const AUTO_OPTIMIZE_INTERVAL: u64 = 60;

/// GitHub repository URL
const GITHUB_URL: &str = "https://github.com/ruvnet/optimizer";
/// Version string
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct TrayApp {
    running: Arc<AtomicBool>,
}

impl TrayApp {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::new()?;

        // Get initial memory status
        let status_text = get_memory_status_text();

        // Create menu items
        let menu = Menu::new();

        // Status section
        let status_item = MenuItem::new(&status_text, false, None);
        let version_item = MenuItem::new(&format!("v{}", VERSION), false, None);

        // Main actions
        let auto_item = CheckMenuItem::new("Auto-Optimize (60s)", true, true, None);
        let optimize_item = MenuItem::new("Optimize Now", true, None);
        let aggressive_item = MenuItem::new("Deep Clean", true, None);

        // AI Mode submenu
        let ai_menu = Submenu::new("AI Mode", true);
        let game_mode_item = CheckMenuItem::new("Game Mode Auto-Detect", true, true, None);
        let focus_mode_item = CheckMenuItem::new("Focus Mode Auto-Detect", true, true, None);
        let thermal_item = CheckMenuItem::new("Thermal Prediction", true, true, None);
        let preload_item = CheckMenuItem::new("Predictive Preloading", true, true, None);
        ai_menu.append(&game_mode_item)?;
        ai_menu.append(&focus_mode_item)?;
        ai_menu.append(&PredefinedMenuItem::separator())?;
        ai_menu.append(&thermal_item)?;
        ai_menu.append(&preload_item)?;

        // Settings submenu
        let settings_menu = Submenu::new("Settings", true);
        let threshold_75 = CheckMenuItem::new("Threshold: 75%", true, true, None);
        let threshold_80 = CheckMenuItem::new("Threshold: 80%", true, false, None);
        let threshold_85 = CheckMenuItem::new("Threshold: 85%", true, false, None);
        let threshold_90 = CheckMenuItem::new("Threshold: 90%", true, false, None);
        settings_menu.append(&threshold_75)?;
        settings_menu.append(&threshold_80)?;
        settings_menu.append(&threshold_85)?;
        settings_menu.append(&threshold_90)?;

        // Info section
        let cpu_item = MenuItem::new("System Info", true, None);
        let github_item = MenuItem::new("GitHub Repository", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        // Build menu
        menu.append(&status_item)?;
        menu.append(&version_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&auto_item)?;
        menu.append(&optimize_item)?;
        menu.append(&aggressive_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&ai_menu)?;
        menu.append(&settings_menu)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&cpu_item)?;
        menu.append(&github_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&quit_item)?;

        // Get initial memory usage for icon
        let initial_usage = WindowsMemoryOptimizer::get_memory_status()
            .map(|s| s.memory_load_percent)
            .unwrap_or(50);

        // Create tray icon with current usage
        let icon_data = create_icon_with_usage(initial_usage);
        let icon = Icon::from_rgba(icon_data, 32, 32)?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(&format!("RuVector MemOpt v{} - Auto-optimizing", VERSION))
            .with_icon(icon)
            .build()?;

        // Store menu item IDs
        let optimize_id = optimize_item.id().clone();
        let aggressive_id = aggressive_item.id().clone();
        let cpu_id = cpu_item.id().clone();
        let quit_id = quit_item.id().clone();
        let auto_id = auto_item.id().clone();
        let github_id = github_item.id().clone();
        let game_mode_id = game_mode_item.id().clone();
        let focus_mode_id = focus_mode_item.id().clone();
        let thermal_id = thermal_item.id().clone();
        let preload_id = preload_item.id().clone();
        let threshold_75_id = threshold_75.id().clone();
        let threshold_80_id = threshold_80.id().clone();
        let threshold_85_id = threshold_85.id().clone();
        let threshold_90_id = threshold_90.id().clone();

        let running = self.running.clone();
        let mut last_update = std::time::Instant::now();
        let mut last_auto_optimize = std::time::Instant::now();
        let auto_enabled = Arc::new(AtomicBool::new(true));
        let game_mode_enabled = Arc::new(AtomicBool::new(true));
        let focus_mode_enabled = Arc::new(AtomicBool::new(true));
        let thermal_enabled = Arc::new(AtomicBool::new(true));
        let preload_enabled = Arc::new(AtomicBool::new(true));
        let current_threshold = Arc::new(AtomicU32::new(75));
        let last_usage = Arc::new(AtomicU32::new(initial_usage));
        let total_freed = Arc::new(AtomicU32::new(0));

        // Run event loop
        #[allow(deprecated)]
        event_loop.run(move |_event, event_loop| {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_secs(1)
            ));

            // Update status and check for auto-optimization every 5 seconds
            if last_update.elapsed() > std::time::Duration::from_secs(5) {
                if let Ok(status) = WindowsMemoryOptimizer::get_memory_status() {
                    let usage = status.memory_load_percent;
                    last_usage.store(usage, Ordering::SeqCst);

                    // Update status text
                    let freed = total_freed.load(Ordering::SeqCst);
                    let threshold = current_threshold.load(Ordering::SeqCst);
                    let text = if freed > 0 {
                        format!(
                            "Memory: {:.0}% ({:.1}/{:.1} GB) | Freed: {} MB",
                            usage,
                            status.used_physical_mb() / 1024.0,
                            status.total_physical_mb / 1024.0,
                            freed
                        )
                    } else {
                        format!(
                            "Memory: {:.0}% ({:.1}/{:.1} GB)",
                            usage,
                            status.used_physical_mb() / 1024.0,
                            status.total_physical_mb / 1024.0
                        )
                    };
                    let _ = status_item.set_text(&text);

                    // Update icon color based on usage
                    let icon_data = create_icon_with_usage(usage);
                    if let Ok(new_icon) = Icon::from_rgba(icon_data, 32, 32) {
                        let _ = tray_icon.set_icon(Some(new_icon));
                    }

                    // Update tooltip with mode info
                    let modes = build_mode_string(
                        game_mode_enabled.load(Ordering::SeqCst),
                        focus_mode_enabled.load(Ordering::SeqCst),
                    );
                    let tooltip = if auto_enabled.load(Ordering::SeqCst) {
                        format!("RuVector v{} - {}% | Auto @{}%{}", VERSION, usage, threshold, modes)
                    } else {
                        format!("RuVector v{} - {}% | Manual{}", VERSION, usage, modes)
                    };
                    let _ = tray_icon.set_tooltip(Some(tooltip));

                    // Auto-optimize if enabled and conditions met
                    if auto_enabled.load(Ordering::SeqCst)
                        && usage > threshold
                        && last_auto_optimize.elapsed() > std::time::Duration::from_secs(AUTO_OPTIMIZE_INTERVAL)
                    {
                        let total_freed_clone = total_freed.clone();
                        std::thread::spawn(move || {
                            let optimizer = WindowsMemoryOptimizer::new();
                            if let Ok(result) = optimizer.optimize(false) {
                                if result.freed_mb > 100.0 {
                                    let current = total_freed_clone.load(Ordering::SeqCst);
                                    total_freed_clone.store(current + result.freed_mb as u32, Ordering::SeqCst);
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
                if event.id == quit_id {
                    running.store(false, Ordering::SeqCst);
                    event_loop.exit();
                } else if event.id == optimize_id {
                    let total_freed_clone = total_freed.clone();
                    run_optimization(false, total_freed_clone);
                } else if event.id == aggressive_id {
                    let total_freed_clone = total_freed.clone();
                    run_optimization(true, total_freed_clone);
                } else if event.id == cpu_id {
                    show_cpu_info();
                } else if event.id == github_id {
                    open_github();
                } else if event.id == auto_id {
                    let current = auto_enabled.load(Ordering::SeqCst);
                    auto_enabled.store(!current, Ordering::SeqCst);
                    let _ = auto_item.set_checked(!current);
                } else if event.id == game_mode_id {
                    let current = game_mode_enabled.load(Ordering::SeqCst);
                    game_mode_enabled.store(!current, Ordering::SeqCst);
                    let _ = game_mode_item.set_checked(!current);
                } else if event.id == focus_mode_id {
                    let current = focus_mode_enabled.load(Ordering::SeqCst);
                    focus_mode_enabled.store(!current, Ordering::SeqCst);
                    let _ = focus_mode_item.set_checked(!current);
                } else if event.id == thermal_id {
                    let current = thermal_enabled.load(Ordering::SeqCst);
                    thermal_enabled.store(!current, Ordering::SeqCst);
                    let _ = thermal_item.set_checked(!current);
                } else if event.id == preload_id {
                    let current = preload_enabled.load(Ordering::SeqCst);
                    preload_enabled.store(!current, Ordering::SeqCst);
                    let _ = preload_item.set_checked(!current);
                } else if event.id == threshold_75_id {
                    current_threshold.store(75, Ordering::SeqCst);
                    let _ = threshold_75.set_checked(true);
                    let _ = threshold_80.set_checked(false);
                    let _ = threshold_85.set_checked(false);
                    let _ = threshold_90.set_checked(false);
                } else if event.id == threshold_80_id {
                    current_threshold.store(80, Ordering::SeqCst);
                    let _ = threshold_75.set_checked(false);
                    let _ = threshold_80.set_checked(true);
                    let _ = threshold_85.set_checked(false);
                    let _ = threshold_90.set_checked(false);
                } else if event.id == threshold_85_id {
                    current_threshold.store(85, Ordering::SeqCst);
                    let _ = threshold_75.set_checked(false);
                    let _ = threshold_80.set_checked(false);
                    let _ = threshold_85.set_checked(true);
                    let _ = threshold_90.set_checked(false);
                } else if event.id == threshold_90_id {
                    current_threshold.store(90, Ordering::SeqCst);
                    let _ = threshold_75.set_checked(false);
                    let _ = threshold_80.set_checked(false);
                    let _ = threshold_85.set_checked(false);
                    let _ = threshold_90.set_checked(true);
                }
            }
        })?;

        Ok(())
    }
}

fn build_mode_string(game: bool, focus: bool) -> String {
    let mut modes = Vec::new();
    if game { modes.push("Game"); }
    if focus { modes.push("Focus"); }
    if modes.is_empty() {
        String::new()
    } else {
        format!(" | {}", modes.join("+"))
    }
}

fn get_memory_status_text() -> String {
    if let Ok(status) = WindowsMemoryOptimizer::get_memory_status() {
        format!(
            "Memory: {:.0}% ({:.1}/{:.1} GB)",
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
        let optimizer = WindowsMemoryOptimizer::new();
        match optimizer.optimize(aggressive) {
            Ok(result) => {
                let current = total_freed.load(Ordering::SeqCst);
                total_freed.store(current + result.freed_mb as u32, Ordering::SeqCst);

                let msg = format!(
                    "Optimization Complete!\n\nFreed: {:.1} MB\nProcesses: {}\nTime: {} ms",
                    result.freed_mb, result.processes_trimmed, result.duration_ms
                );
                show_message_box("RuVector Optimizer", &msg);
            }
            Err(e) => {
                show_message_box("RuVector Optimizer", &format!("Error: {}", e));
            }
        }
    });
}

fn show_cpu_info() {
    let caps = CpuCapabilities::detect();
    let msg = format!(
        "RuVector Memory Optimizer v{}\n\n\
        CPU: {}\n\n\
        Cores: {}\n\
        AVX2: {}\n\
        AVX-512: {}\n\
        AVX-VNNI: {}\n\
        Intel NPU: {}\n\n\
        Estimated SIMD Speedup: {:.1}x\n\n\
        GitHub: {}",
        VERSION,
        caps.model,
        caps.core_count,
        if caps.has_avx2 { "Yes" } else { "No" },
        if caps.has_avx512 { "Yes" } else { "No" },
        if caps.has_avx_vnni { "Yes" } else { "No" },
        if caps.has_npu { "Yes" } else { "No" },
        caps.estimated_speedup(),
        GITHUB_URL
    );
    show_message_box("System Information", &msg);
}

fn open_github() {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", GITHUB_URL])
            .spawn();
    }
}

fn show_message_box(title: &str, message: &str) {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;

        fn to_wide(s: &str) -> Vec<u16> {
            OsStr::new(s).encode_wide().chain(Some(0)).collect()
        }

        let title = to_wide(title);
        let message = to_wide(message);

        unsafe {
            windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                windows::Win32::Foundation::HWND(ptr::null_mut()),
                windows::core::PCWSTR(message.as_ptr()),
                windows::core::PCWSTR(title.as_ptr()),
                windows::Win32::UI::WindowsAndMessaging::MB_OK |
                windows::Win32::UI::WindowsAndMessaging::MB_ICONINFORMATION,
            );
        }
    }
}

/// Create icon with specific memory usage percentage for color coding
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
            // Memory chip shape: rounded rectangle with notch
            let in_body = x >= 4 && x < 28 && y >= 2 && y < 30;
            let in_notch = x >= 12 && x < 20 && y < 4;
            let in_chip = in_body && !in_notch;

            // Pin markers on sides
            let left_pin = x < 4 && (y == 8 || y == 14 || y == 20 || y == 26);
            let right_pin = x >= 28 && (y == 8 || y == 14 || y == 20 || y == 26);
            let is_pin = left_pin || right_pin;

            // Border detection
            let is_border = in_chip && (x == 4 || x == 27 || y == 2 || y == 29 ||
                                       (y == 3 && (x < 12 || x >= 20)));

            // Fill level indicator (shows usage as filled portion)
            let fill_height = 28 - ((usage_percent as i32 * 26) / 100);
            let is_filled = in_chip && !is_border && (y as i32) >= fill_height;

            if is_pin {
                // Pins in border color
                data.push(border_r);
                data.push(border_g);
                data.push(border_b);
                data.push(0xFF);
            } else if is_border {
                // Border
                data.push(border_r);
                data.push(border_g);
                data.push(border_b);
                data.push(0xFF);
            } else if is_filled {
                // Filled portion (based on usage)
                data.push(r);
                data.push(g);
                data.push(b);
                data.push(0xFF);
            } else if in_chip {
                // Empty portion (darker)
                data.push(r / 3);
                data.push(g / 3);
                data.push(b / 3);
                data.push(0xFF);
            } else {
                // Transparent
                data.push(0x00);
                data.push(0x00);
                data.push(0x00);
                data.push(0x00);
            }
        }
    }

    data
}

impl Default for TrayApp {
    fn default() -> Self {
        Self::new()
    }
}
