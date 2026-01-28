//! System-level Windows APIs

use sysinfo::System;

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os_name: String,
    pub kernel_version: String,
    pub host_name: String,
    pub cpu_count: usize,
    pub cpu_usage: f32,
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
}

pub fn get_system_info() -> SystemInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    SystemInfo {
        os_name: System::name().unwrap_or_default(),
        kernel_version: System::kernel_version().unwrap_or_default(),
        host_name: System::host_name().unwrap_or_default(),
        cpu_count: sys.cpus().len(),
        cpu_usage: sys.global_cpu_usage(),
        total_memory_mb: sys.total_memory() / 1024 / 1024,
        used_memory_mb: sys.used_memory() / 1024 / 1024,
    }
}
