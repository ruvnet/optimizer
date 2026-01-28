//! RuVector Memory Optimizer - System Tray Application
//!
//! This is a separate binary for the system tray that hides the console window.
//! Windows-only binary.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod core;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
mod neural;
#[cfg(windows)]
mod bench;
#[cfg(windows)]
mod monitor;
#[cfg(windows)]
mod accel;
#[cfg(windows)]
mod tray;
#[cfg(windows)]
mod algorithms;
#[cfg(windows)]
mod dashboard;

#[cfg(windows)]
fn main() {
    // Immediately hide/detach from any console window
    #[cfg(windows)]
    {
        use ::windows::Win32::System::Console::FreeConsole;
        let _ = unsafe { FreeConsole() };
    }

    // Initialize minimal logging (to file, not console)
    let log_path = std::env::temp_dir().join("ruvector-memopt.log");
    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false)
            .try_init();
    }

    let tray_app = tray::TrayApp::new();
    if let Err(e) = tray_app.run() {
        // Log the error since console is hidden
        tracing::error!("Tray error: {}", e);

        // Also write to a dedicated error file
        let error_path = std::env::temp_dir().join("ruvector-memopt-error.txt");
        let _ = std::fs::write(&error_path, format!("Tray error: {}", e));
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows tray application only runs on Windows");
    eprintln!("On macOS, use: ruvector-memopt tray");
}
