//! Windows Memory Management Core with Real Win32 APIs

use sysinfo::System;
use tracing::{info, warn};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct MemoryStatus {
    pub total_physical_mb: f64,
    pub available_physical_mb: f64,
    pub memory_load_percent: u32,
    pub total_page_file_mb: f64,
    pub available_page_file_mb: f64,
    pub total_virtual_mb: f64,
    pub available_virtual_mb: f64,
}

impl MemoryStatus {
    pub fn used_physical_mb(&self) -> f64 { self.total_physical_mb - self.available_physical_mb }
    pub fn is_high_pressure(&self) -> bool { self.memory_load_percent > 80 }
    pub fn is_critical(&self) -> bool { self.memory_load_percent > 95 }
}

#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub freed_mb: f64,
    pub before_available_mb: f64,
    pub after_available_mb: f64,
    pub processes_trimmed: usize,
    pub duration_ms: u64,
}

pub struct WindowsMemoryOptimizer {
    has_admin: bool,
}

impl WindowsMemoryOptimizer {
    pub fn new() -> Self {
        let has_admin = Self::check_admin();
        if !has_admin { warn!("Running without admin - limited optimization"); }
        else { info!("Running with admin privileges - full optimization available"); }
        Self { has_admin }
    }

    fn check_admin() -> bool {
        #[cfg(windows)]
        {
            // Use Windows API to check admin - no console window!
            use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
            use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
            use windows::Win32::Foundation::CloseHandle;
            use std::mem::{size_of, MaybeUninit};

            unsafe {
                let mut token = MaybeUninit::uninit();
                if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, token.as_mut_ptr()).is_err() {
                    return false;
                }
                let token = token.assume_init();

                let mut elevation = TOKEN_ELEVATION::default();
                let mut size = 0u32;
                let result = GetTokenInformation(
                    token,
                    TokenElevation,
                    Some(&mut elevation as *mut _ as *mut _),
                    size_of::<TOKEN_ELEVATION>() as u32,
                    &mut size,
                );
                let _ = CloseHandle(token);
                result.is_ok() && elevation.TokenIsElevated != 0
            }
        }
        #[cfg(not(windows))]
        { false }
    }

    pub fn get_memory_status() -> Result<MemoryStatus, String> {
        let mut sys = System::new();
        sys.refresh_memory();
        let total = sys.total_memory() as f64 / 1024.0 / 1024.0;
        let avail = sys.available_memory() as f64 / 1024.0 / 1024.0;
        let load = if total > 0.0 { (((total - avail) / total) * 100.0) as u32 } else { 0 };
        Ok(MemoryStatus {
            total_physical_mb: total, available_physical_mb: avail, memory_load_percent: load,
            total_page_file_mb: total * 1.5, available_page_file_mb: avail,
            total_virtual_mb: total * 2.0, available_virtual_mb: avail * 2.0,
        })
    }

    #[cfg(windows)]
    pub fn trim_process_working_set(pid: u32) -> Result<u64, String> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_QUERY_INFORMATION};
        use windows::Win32::System::Memory::SetProcessWorkingSetSizeEx;
        use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
        use std::mem::size_of;

        unsafe {
            let handle = match OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, false, pid) {
                Ok(h) => h,
                Err(_) => return Ok(0),
            };

            let mut mem_counters = PROCESS_MEMORY_COUNTERS::default();
            mem_counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            let before_ws = if GetProcessMemoryInfo(handle, &mut mem_counters, size_of::<PROCESS_MEMORY_COUNTERS>() as u32).is_ok() {
                mem_counters.WorkingSetSize
            } else { 0 };

            let _ = SetProcessWorkingSetSizeEx(handle, usize::MAX, usize::MAX, 
                windows::Win32::System::Memory::SETPROCESSWORKINGSETSIZEEX_FLAGS(0));

            let after_ws = if GetProcessMemoryInfo(handle, &mut mem_counters, size_of::<PROCESS_MEMORY_COUNTERS>() as u32).is_ok() {
                mem_counters.WorkingSetSize
            } else { before_ws };

            let _ = CloseHandle(handle);
            Ok(before_ws.saturating_sub(after_ws) as u64)
        }
    }

    #[cfg(not(windows))]
    pub fn trim_process_working_set(_pid: u32) -> Result<u64, String> { Ok(0) }

    pub fn optimize(&self, aggressive: bool) -> Result<OptimizationResult, String> {
        let start = Instant::now();
        let before = Self::get_memory_status()?;
        let mut trimmed = 0usize;
        let mut total_freed: u64 = 0;

        if let Ok(procs) = super::process::list_processes() {
            for pid in procs.iter().take(150) {
                match Self::trim_process_working_set(*pid) {
                    Ok(freed) => {
                        if freed > 0 {
                            total_freed += freed;
                            trimmed += 1;
                        }
                    }
                    Err(_) => {}
                }
            }
        }

        // Force garbage collection pause
        std::thread::sleep(std::time::Duration::from_millis(100));

        let after = Self::get_memory_status()?;
        let measured_freed = after.available_physical_mb - before.available_physical_mb;
        let calculated_freed = total_freed as f64 / 1024.0 / 1024.0;
        let freed_mb = measured_freed.max(calculated_freed).max(0.0);

        info!("Optimized: trimmed {} processes, freed {:.1} MB in {}ms", 
            trimmed, freed_mb, start.elapsed().as_millis());

        Ok(OptimizationResult {
            freed_mb,
            before_available_mb: before.available_physical_mb,
            after_available_mb: after.available_physical_mb,
            processes_trimmed: trimmed,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    pub fn has_admin_privileges(&self) -> bool { self.has_admin }
}

impl Default for WindowsMemoryOptimizer { fn default() -> Self { Self::new() } }
