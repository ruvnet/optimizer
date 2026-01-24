//! System tray icon and menu

use crate::windows::memory::WindowsMemoryOptimizer;
use crate::accel::CpuCapabilities;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder, Icon,
};
use winit::event_loop::{ControlFlow, EventLoop};

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
        let status_item = MenuItem::new(&status_text, false, None);
        let optimize_item = MenuItem::new("Optimize Now", true, None);
        let aggressive_item = MenuItem::new("Aggressive Optimize", true, None);
        let cpu_item = MenuItem::new("CPU Info", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        menu.append(&status_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&optimize_item)?;
        menu.append(&aggressive_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&cpu_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&quit_item)?;

        // Create tray icon
        let icon_data = create_icon_data();
        let icon = Icon::from_rgba(icon_data, 32, 32)?;

        let _tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("RuVector Memory Optimizer")
            .with_icon(icon)
            .build()?;

        let optimize_id = optimize_item.id().clone();
        let aggressive_id = aggressive_item.id().clone();
        let cpu_id = cpu_item.id().clone();
        let quit_id = quit_item.id().clone();

        let running = self.running.clone();
        let mut last_update = std::time::Instant::now();

        // Run event loop
        #[allow(deprecated)]
        event_loop.run(move |_event, event_loop| {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_secs(1)
            ));

            // Update status text periodically
            if last_update.elapsed() > std::time::Duration::from_secs(5) {
                let text = get_memory_status_text();
                let _ = status_item.set_text(&text);
                last_update = std::time::Instant::now();
            }

            if let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id == quit_id {
                    running.store(false, Ordering::SeqCst);
                    event_loop.exit();
                } else if event.id == optimize_id {
                    run_optimization(false);
                } else if event.id == aggressive_id {
                    run_optimization(true);
                } else if event.id == cpu_id {
                    show_cpu_info();
                }
            }
        })?;

        Ok(())
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

fn run_optimization(aggressive: bool) {
    std::thread::spawn(move || {
        let optimizer = WindowsMemoryOptimizer::new();
        match optimizer.optimize(aggressive) {
            Ok(result) => {
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
        "CPU: {}\n\nCores: {}\nAVX2: {}\nAVX-512: {}\nAVX-VNNI: {}\nIntel NPU: {}\n\nEstimated SIMD Speedup: {:.0}x",
        caps.model,
        caps.core_count,
        if caps.has_avx2 { "Yes" } else { "No" },
        if caps.has_avx512 { "Yes" } else { "No" },
        if caps.has_avx_vnni { "Yes" } else { "No" },
        if caps.has_npu { "Yes" } else { "No" },
        caps.estimated_speedup()
    );
    show_message_box("CPU Information", &msg);
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

/// Create a memory-chip shaped icon with color based on usage
/// - Green: < 60% usage (healthy)
/// - Orange: 60-80% usage (moderate)
/// - Red: > 80% usage (critical)
fn create_icon_data() -> Vec<u8> {
    create_icon_with_usage(50) // Default to green
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
