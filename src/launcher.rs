//! Silent launcher for RuVector Tray
//!
//! This tiny GUI-only executable launches the tray app with no console flash.
//! It uses CreateProcessW with CREATE_NO_WINDOW flag.
//! Windows-only binary.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
use std::ffi::OsStr;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::ptr::null_mut;

#[cfg(windows)]
fn main() {
    // Get path to tray exe (same directory as this launcher)
    let exe_path = std::env::current_exe().unwrap_or_default();
    let exe_dir = exe_path.parent().unwrap_or(std::path::Path::new("."));
    let tray_path = exe_dir.join("ruvector-memopt-tray.exe");

    // Convert to wide string for Windows API
    let wide_path: Vec<u16> = OsStr::new(&tray_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        use windows::Win32::System::Threading::*;
        use windows::Win32::Foundation::*;
        use windows::core::{PCWSTR, PWSTR};

        let mut startup_info: STARTUPINFOW = std::mem::zeroed();
        startup_info.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        startup_info.dwFlags = STARTF_USESHOWWINDOW;
        startup_info.wShowWindow = 0; // SW_HIDE

        let mut process_info: PROCESS_INFORMATION = std::mem::zeroed();

        // CREATE_NO_WINDOW = 0x08000000
        let result = CreateProcessW(
            PCWSTR(wide_path.as_ptr()),
            PWSTR(null_mut()),
            None,
            None,
            false,
            CREATE_NO_WINDOW,
            None,
            None,
            &startup_info,
            &mut process_info,
        );

        if result.is_ok() {
            // Close handles - we don't need to wait
            let _ = CloseHandle(process_info.hProcess);
            let _ = CloseHandle(process_info.hThread);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows launcher only runs on Windows");
}
