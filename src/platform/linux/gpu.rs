//! Linux GPU Monitoring
//!
//! Provides GPU detection and monitoring for Linux systems, supporting:
//! - NVIDIA GPUs via NVML (nvidia-smi)
//! - AMD GPUs via sysfs (amdgpu driver)
//! - Intel GPUs via sysfs (i915 driver)
//!
//! # Example
//!
//! ```no_run
//! use ruvector_memopt::platform::linux::gpu::LinuxGpuMonitor;
//!
//! let monitor = LinuxGpuMonitor::new();
//! for gpu in monitor.detect_gpus() {
//!     println!("Found GPU: {} ({:?})", gpu.name, gpu.vendor);
//!     if let Some(stats) = monitor.get_gpu_stats(gpu.index) {
//!         println!("  Utilization: {:.1}%", stats.utilization.unwrap_or(0.0));
//!         println!("  Temperature: {:.1}C", stats.temperature.unwrap_or(0.0));
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// GPU vendor identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuVendor {
    /// NVIDIA Corporation
    Nvidia,
    /// Advanced Micro Devices (AMD)
    Amd,
    /// Intel Corporation
    Intel,
    /// Unknown or unsupported vendor
    Unknown,
}

impl std::fmt::Display for GpuVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuVendor::Nvidia => write!(f, "NVIDIA"),
            GpuVendor::Amd => write!(f, "AMD"),
            GpuVendor::Intel => write!(f, "Intel"),
            GpuVendor::Unknown => write!(f, "Unknown"),
        }
    }
}

/// GPU information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU index (0-based)
    pub index: usize,
    /// GPU name/model
    pub name: String,
    /// GPU vendor
    pub vendor: GpuVendor,
    /// Total VRAM in bytes
    pub vram_total: u64,
    /// Driver version string
    pub driver_version: String,
    /// PCI device path (e.g., "0000:01:00.0")
    pub pci_slot: Option<String>,
    /// DRM card path (e.g., "/sys/class/drm/card0")
    pub drm_path: Option<PathBuf>,
}

/// GPU statistics structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuStats {
    /// GPU utilization percentage (0-100)
    pub utilization: Option<f32>,
    /// Memory used in bytes
    pub memory_used: Option<u64>,
    /// Total memory in bytes
    pub memory_total: Option<u64>,
    /// GPU temperature in Celsius
    pub temperature: Option<f32>,
    /// Power draw in watts
    pub power_draw: Option<f32>,
    /// GPU clock speed in MHz
    pub clock_mhz: Option<u32>,
    /// Memory clock speed in MHz
    pub memory_clock_mhz: Option<u32>,
}

impl GpuStats {
    /// Get memory usage as a percentage
    pub fn memory_usage_percent(&self) -> Option<f32> {
        match (self.memory_used, self.memory_total) {
            (Some(used), Some(total)) if total > 0 => {
                Some((used as f64 / total as f64 * 100.0) as f32)
            }
            _ => None,
        }
    }
}

/// VRAM information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VramInfo {
    /// Total VRAM in bytes
    pub total: u64,
    /// Used VRAM in bytes
    pub used: u64,
    /// Free VRAM in bytes
    pub free: u64,
}

impl VramInfo {
    /// Get usage percentage
    pub fn usage_percent(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.used as f64 / self.total as f64 * 100.0) as f32
    }

    /// Get total in MB
    pub fn total_mb(&self) -> u64 {
        self.total / (1024 * 1024)
    }

    /// Get used in MB
    pub fn used_mb(&self) -> u64 {
        self.used / (1024 * 1024)
    }

    /// Get free in MB
    pub fn free_mb(&self) -> u64 {
        self.free / (1024 * 1024)
    }
}

/// Linux GPU Monitor
///
/// Provides unified GPU monitoring across NVIDIA, AMD, and Intel GPUs on Linux.
pub struct LinuxGpuMonitor {
    /// Cached GPU information
    gpus: Vec<GpuInfo>,
    /// NVML handle for NVIDIA GPUs (when feature enabled)
    #[cfg(feature = "nvml")]
    nvml: Option<nvml_wrapper::Nvml>,
}

impl LinuxGpuMonitor {
    /// Create a new Linux GPU monitor
    ///
    /// Automatically detects all available GPUs on the system.
    pub fn new() -> Self {
        let mut monitor = Self {
            gpus: Vec::new(),
            #[cfg(feature = "nvml")]
            nvml: None,
        };

        // Initialize NVML if available
        #[cfg(feature = "nvml")]
        {
            monitor.nvml = nvml_wrapper::Nvml::init().ok();
        }

        // Detect all GPUs
        monitor.gpus = monitor.detect_gpus_internal();
        monitor
    }

    /// Detect all GPUs on the system
    ///
    /// Returns a vector of GpuInfo structures for each detected GPU.
    pub fn detect_gpus(&self) -> Vec<GpuInfo> {
        self.gpus.clone()
    }

    /// Internal GPU detection implementation
    fn detect_gpus_internal(&self) -> Vec<GpuInfo> {
        let mut gpus = Vec::new();
        let mut index = 0;

        // Detect NVIDIA GPUs via NVML
        #[cfg(feature = "nvml")]
        if let Some(ref nvml) = self.nvml {
            if let Ok(count) = nvml.device_count() {
                for i in 0..count {
                    if let Ok(device) = nvml.device_by_index(i) {
                        let name = device.name().unwrap_or_else(|_| "Unknown NVIDIA GPU".to_string());
                        let memory = device.memory_info().ok();
                        let vram_total = memory.map(|m| m.total).unwrap_or(0);
                        let driver_version = nvml.sys_driver_version().unwrap_or_default();
                        let pci_info = device.pci_info().ok();

                        gpus.push(GpuInfo {
                            index,
                            name,
                            vendor: GpuVendor::Nvidia,
                            vram_total,
                            driver_version,
                            pci_slot: pci_info.map(|p| p.bus_id),
                            drm_path: None,
                        });
                        index += 1;
                    }
                }
            }
        }

        // Detect GPUs via DRM subsystem (sysfs)
        let drm_cards = self.enumerate_drm_cards();
        for card_path in drm_cards {
            if let Some(gpu_info) = self.detect_drm_gpu(&card_path, index) {
                // Skip if we already detected this GPU via NVML
                let is_duplicate = gpus.iter().any(|g: &GpuInfo| {
                    g.vendor == GpuVendor::Nvidia && gpu_info.vendor == GpuVendor::Nvidia
                });

                if !is_duplicate {
                    gpus.push(gpu_info);
                    index += 1;
                }
            }
        }

        gpus
    }

    /// Enumerate DRM card devices
    fn enumerate_drm_cards(&self) -> Vec<PathBuf> {
        let drm_path = Path::new("/sys/class/drm");
        let mut cards = Vec::new();

        if let Ok(entries) = fs::read_dir(drm_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                // Match card0, card1, etc. (not card0-DP-1, etc.)
                if name_str.starts_with("card") && !name_str.contains('-') {
                    cards.push(entry.path());
                }
            }
        }

        cards.sort();
        cards
    }

    /// Detect GPU information from DRM card path
    fn detect_drm_gpu(&self, card_path: &Path, index: usize) -> Option<GpuInfo> {
        let device_path = card_path.join("device");

        // Read vendor ID
        let vendor_id = self.read_sysfs_hex(&device_path.join("vendor"))?;
        let vendor = match vendor_id {
            0x10DE => GpuVendor::Nvidia,
            0x1002 | 0x1022 => GpuVendor::Amd,
            0x8086 => GpuVendor::Intel,
            _ => GpuVendor::Unknown,
        };

        // Read device name
        let name = self.get_gpu_name(&device_path, vendor);

        // Read VRAM total
        let vram_total = self.get_vram_total(&device_path, vendor);

        // Read driver version
        let driver_version = self.get_driver_version(&device_path);

        // Read PCI slot
        let pci_slot = device_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string());

        Some(GpuInfo {
            index,
            name,
            vendor,
            vram_total,
            driver_version,
            pci_slot,
            drm_path: Some(card_path.to_path_buf()),
        })
    }

    /// Get GPU name from sysfs or use vendor default
    fn get_gpu_name(&self, device_path: &Path, vendor: GpuVendor) -> String {
        // Try to read from uevent first
        if let Ok(uevent) = fs::read_to_string(device_path.join("uevent")) {
            for line in uevent.lines() {
                if line.starts_with("PCI_ID=") {
                    // Try to get a better name from device ID
                    let device_id = line.trim_start_matches("PCI_ID=");
                    return format!("{} GPU ({})", vendor, device_id);
                }
            }
        }

        // Try vendor-specific name sources
        match vendor {
            GpuVendor::Amd => {
                // AMD: Try to read from marketing name
                if let Ok(name) = fs::read_to_string(device_path.join("product_name")) {
                    let name = name.trim().to_string();
                    if !name.is_empty() {
                        return name;
                    }
                }
            }
            GpuVendor::Intel => {
                // Intel: Try to read from device description
                if let Ok(name) = fs::read_to_string(device_path.join("label")) {
                    let name = name.trim().to_string();
                    if !name.is_empty() {
                        return name;
                    }
                }
            }
            _ => {}
        }

        // Fallback to generic name
        format!("{} GPU", vendor)
    }

    /// Get total VRAM from sysfs
    fn get_vram_total(&self, device_path: &Path, vendor: GpuVendor) -> u64 {
        match vendor {
            GpuVendor::Amd => {
                // AMD: Read from mem_info_vram_total
                if let Some(vram) = self.read_sysfs_u64(&device_path.join("mem_info_vram_total")) {
                    return vram;
                }
            }
            GpuVendor::Intel => {
                // Intel: Try GT total memory (integrated GPUs share system RAM)
                let gt_path = device_path.join("drm");
                if let Ok(entries) = fs::read_dir(&gt_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.file_name().map(|n| n.to_string_lossy().starts_with("card")).unwrap_or(false) {
                            // Intel integrated GPUs typically report via i915 debugfs
                            // or through memory regions - here we estimate based on system
                            break;
                        }
                    }
                }
                // Intel integrated GPUs share system memory - return 0 for dedicated
                return 0;
            }
            _ => {}
        }

        // Try generic resource file for dedicated memory
        if let Ok(resource) = fs::read_to_string(device_path.join("resource")) {
            // Parse BAR 0 (typically VRAM) size from resource file
            if let Some(line) = resource.lines().next() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let (Ok(start), Ok(end)) = (
                        u64::from_str_radix(parts[0].trim_start_matches("0x"), 16),
                        u64::from_str_radix(parts[1].trim_start_matches("0x"), 16),
                    ) {
                        if end > start {
                            return end - start + 1;
                        }
                    }
                }
            }
        }

        0
    }

    /// Get driver version
    fn get_driver_version(&self, device_path: &Path) -> String {
        // Try to get driver name and version
        let driver_link = device_path.join("driver");
        if let Ok(link) = fs::read_link(&driver_link) {
            if let Some(driver_name) = link.file_name() {
                let driver = driver_name.to_string_lossy().to_string();

                // Try to get version from module
                let version_path = format!("/sys/module/{}/version", driver);
                if let Ok(version) = fs::read_to_string(&version_path) {
                    return format!("{} {}", driver, version.trim());
                }

                return driver;
            }
        }

        "Unknown".to_string()
    }

    /// Get GPU statistics for a specific GPU
    ///
    /// # Arguments
    ///
    /// * `index` - GPU index (0-based)
    ///
    /// # Returns
    ///
    /// Returns `Some(GpuStats)` if the GPU exists and stats are available,
    /// `None` otherwise.
    pub fn get_gpu_stats(&self, index: usize) -> Option<GpuStats> {
        let gpu = self.gpus.get(index)?;

        match gpu.vendor {
            GpuVendor::Nvidia => self.get_nvidia_stats(index),
            GpuVendor::Amd => self.get_amd_stats(gpu),
            GpuVendor::Intel => self.get_intel_stats(gpu),
            GpuVendor::Unknown => None,
        }
    }

    /// Get NVIDIA GPU stats via NVML
    #[cfg(feature = "nvml")]
    fn get_nvidia_stats(&self, index: usize) -> Option<GpuStats> {
        let nvml = self.nvml.as_ref()?;
        let device = nvml.device_by_index(index as u32).ok()?;

        let memory = device.memory_info().ok();
        let utilization = device.utilization_rates().ok();
        let temperature = device
            .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            .ok();
        let power = device.power_usage().ok();
        let clocks = device
            .clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics)
            .ok();
        let mem_clocks = device
            .clock_info(nvml_wrapper::enum_wrappers::device::Clock::Memory)
            .ok();

        Some(GpuStats {
            utilization: utilization.map(|u| u.gpu as f32),
            memory_used: memory.map(|m| m.used),
            memory_total: memory.map(|m| m.total),
            temperature: temperature.map(|t| t as f32),
            power_draw: power.map(|p| p as f32 / 1000.0), // mW to W
            clock_mhz: clocks,
            memory_clock_mhz: mem_clocks,
        })
    }

    #[cfg(not(feature = "nvml"))]
    fn get_nvidia_stats(&self, _index: usize) -> Option<GpuStats> {
        // Without NVML, we can't get NVIDIA stats
        // Could potentially parse nvidia-smi output as fallback
        None
    }

    /// Get AMD GPU stats via sysfs
    fn get_amd_stats(&self, gpu: &GpuInfo) -> Option<GpuStats> {
        let drm_path = gpu.drm_path.as_ref()?;
        let device_path = drm_path.join("device");

        // Read GPU busy percent
        let utilization = self
            .read_sysfs_u64(&device_path.join("gpu_busy_percent"))
            .map(|v| v as f32);

        // Read VRAM usage
        let memory_total = self.read_sysfs_u64(&device_path.join("mem_info_vram_total"));
        let memory_used = self.read_sysfs_u64(&device_path.join("mem_info_vram_used"));

        // Read temperature from hwmon
        let temperature = self.get_amd_temperature(&device_path);

        // Read power from hwmon
        let power_draw = self.get_amd_power(&device_path);

        // Read GPU clock
        let clock_mhz = self.get_amd_clock(&device_path, "pp_dpm_sclk");
        let memory_clock_mhz = self.get_amd_clock(&device_path, "pp_dpm_mclk");

        Some(GpuStats {
            utilization,
            memory_used,
            memory_total,
            temperature,
            power_draw,
            clock_mhz,
            memory_clock_mhz,
        })
    }

    /// Get AMD GPU temperature from hwmon
    fn get_amd_temperature(&self, device_path: &Path) -> Option<f32> {
        let hwmon_path = device_path.join("hwmon");

        if let Ok(entries) = fs::read_dir(&hwmon_path) {
            for entry in entries.flatten() {
                let temp_path = entry.path().join("temp1_input");
                if let Some(temp) = self.read_sysfs_u64(&temp_path) {
                    // Temperature is in millidegrees Celsius
                    return Some(temp as f32 / 1000.0);
                }
            }
        }

        None
    }

    /// Get AMD GPU power consumption from hwmon
    fn get_amd_power(&self, device_path: &Path) -> Option<f32> {
        let hwmon_path = device_path.join("hwmon");

        if let Ok(entries) = fs::read_dir(&hwmon_path) {
            for entry in entries.flatten() {
                // Try power1_average first (average power)
                let power_path = entry.path().join("power1_average");
                if let Some(power) = self.read_sysfs_u64(&power_path) {
                    // Power is in microwatts
                    return Some(power as f32 / 1_000_000.0);
                }

                // Fall back to power1_input (instantaneous)
                let power_path = entry.path().join("power1_input");
                if let Some(power) = self.read_sysfs_u64(&power_path) {
                    return Some(power as f32 / 1_000_000.0);
                }
            }
        }

        None
    }

    /// Get AMD GPU clock speed from DPM state
    fn get_amd_clock(&self, device_path: &Path, dpm_file: &str) -> Option<u32> {
        let dpm_path = device_path.join(dpm_file);

        if let Ok(content) = fs::read_to_string(&dpm_path) {
            // Find the active state (marked with *)
            for line in content.lines() {
                if line.contains('*') {
                    // Parse clock from line like "0: 300Mhz *"
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let clock_str = parts[1].trim_end_matches("Mhz").trim_end_matches("MHz");
                        if let Ok(clock) = clock_str.parse::<u32>() {
                            return Some(clock);
                        }
                    }
                }
            }
        }

        None
    }

    /// Get Intel GPU stats via sysfs
    fn get_intel_stats(&self, gpu: &GpuInfo) -> Option<GpuStats> {
        let drm_path = gpu.drm_path.as_ref()?;

        // Intel GPU metrics are typically in the gt/ subdirectory or i915 debugfs
        let device_path = drm_path.join("device");

        // Read GPU frequency
        let clock_mhz = self.get_intel_frequency(&drm_path);

        // Intel integrated GPUs share system memory
        // Try to read from debugfs or perf interface
        let utilization = self.get_intel_utilization(&device_path);

        // Temperature from hwmon
        let temperature = self.get_intel_temperature(&device_path);

        // Power from RAPL or hwmon
        let power_draw = self.get_intel_power(&device_path);

        Some(GpuStats {
            utilization,
            memory_used: None, // Intel iGPU shares system memory
            memory_total: None,
            temperature,
            power_draw,
            clock_mhz,
            memory_clock_mhz: None,
        })
    }

    /// Get Intel GPU frequency
    fn get_intel_frequency(&self, drm_path: &Path) -> Option<u32> {
        // Try gt_cur_freq_mhz for current frequency
        let freq_paths = [
            drm_path.join("gt_cur_freq_mhz"),
            drm_path.join("gt/gt0/cur_freq_mhz"),
            drm_path.join("device/drm").join(
                drm_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .as_ref(),
            ),
        ];

        for path in &freq_paths {
            if let Some(freq) = self.read_sysfs_u64(path) {
                return Some(freq as u32);
            }
        }

        None
    }

    /// Get Intel GPU utilization
    fn get_intel_utilization(&self, device_path: &Path) -> Option<f32> {
        // Intel doesn't expose GPU utilization directly in sysfs
        // It's available via i915 PMU (perf events) or intel_gpu_top
        // For now, return None - could implement perf event reading

        // Try to read from debugfs if available (requires root)
        let debugfs_path = Path::new("/sys/kernel/debug/dri/0/i915_engine_info");
        if debugfs_path.exists() {
            // Would need to parse engine info for actual utilization
            let _ = device_path; // Suppress unused warning
        }

        None
    }

    /// Get Intel GPU temperature
    fn get_intel_temperature(&self, device_path: &Path) -> Option<f32> {
        let hwmon_path = device_path.join("hwmon");

        if let Ok(entries) = fs::read_dir(&hwmon_path) {
            for entry in entries.flatten() {
                // Look for temperature input
                let temp_path = entry.path().join("temp1_input");
                if let Some(temp) = self.read_sysfs_u64(&temp_path) {
                    return Some(temp as f32 / 1000.0);
                }
            }
        }

        None
    }

    /// Get Intel GPU power consumption
    fn get_intel_power(&self, device_path: &Path) -> Option<f32> {
        // Try hwmon first
        let hwmon_path = device_path.join("hwmon");

        if let Ok(entries) = fs::read_dir(&hwmon_path) {
            for entry in entries.flatten() {
                let power_path = entry.path().join("power1_average");
                if let Some(power) = self.read_sysfs_u64(&power_path) {
                    return Some(power as f32 / 1_000_000.0);
                }
            }
        }

        // Could also try RAPL interface for package power
        // /sys/class/powercap/intel-rapl/intel-rapl:0/energy_uj

        None
    }

    /// Get VRAM usage for a specific GPU
    ///
    /// # Arguments
    ///
    /// * `index` - GPU index (0-based)
    ///
    /// # Returns
    ///
    /// Returns `Some(VramInfo)` if VRAM information is available,
    /// `None` otherwise.
    pub fn get_vram_usage(&self, index: usize) -> Option<VramInfo> {
        let gpu = self.gpus.get(index)?;

        match gpu.vendor {
            GpuVendor::Nvidia => self.get_nvidia_vram(index),
            GpuVendor::Amd => self.get_amd_vram(gpu),
            GpuVendor::Intel => None, // Intel iGPU shares system memory
            GpuVendor::Unknown => None,
        }
    }

    /// Get NVIDIA VRAM usage via NVML
    #[cfg(feature = "nvml")]
    fn get_nvidia_vram(&self, index: usize) -> Option<VramInfo> {
        let nvml = self.nvml.as_ref()?;
        let device = nvml.device_by_index(index as u32).ok()?;
        let memory = device.memory_info().ok()?;

        Some(VramInfo {
            total: memory.total,
            used: memory.used,
            free: memory.free,
        })
    }

    #[cfg(not(feature = "nvml"))]
    fn get_nvidia_vram(&self, _index: usize) -> Option<VramInfo> {
        None
    }

    /// Get AMD VRAM usage via sysfs
    fn get_amd_vram(&self, gpu: &GpuInfo) -> Option<VramInfo> {
        let drm_path = gpu.drm_path.as_ref()?;
        let device_path = drm_path.join("device");

        let total = self.read_sysfs_u64(&device_path.join("mem_info_vram_total"))?;
        let used = self.read_sysfs_u64(&device_path.join("mem_info_vram_used"))?;
        let free = total.saturating_sub(used);

        Some(VramInfo { total, used, free })
    }

    /// Get GPU utilization percentage
    ///
    /// # Arguments
    ///
    /// * `index` - GPU index (0-based)
    ///
    /// # Returns
    ///
    /// Returns `Some(f32)` with utilization percentage (0-100),
    /// `None` if not available.
    pub fn get_gpu_utilization(&self, index: usize) -> Option<f32> {
        self.get_gpu_stats(index)?.utilization
    }

    /// Get GPU temperature in Celsius
    ///
    /// # Arguments
    ///
    /// * `index` - GPU index (0-based)
    ///
    /// # Returns
    ///
    /// Returns `Some(f32)` with temperature in Celsius,
    /// `None` if not available.
    pub fn get_gpu_temperature(&self, index: usize) -> Option<f32> {
        self.get_gpu_stats(index)?.temperature
    }

    /// Refresh GPU list
    ///
    /// Re-scans the system for GPUs. Useful if GPUs are hot-plugged.
    pub fn refresh(&mut self) {
        self.gpus = self.detect_gpus_internal();
    }

    /// Get number of detected GPUs
    pub fn gpu_count(&self) -> usize {
        self.gpus.len()
    }

    /// Check if any GPUs were detected
    pub fn has_gpus(&self) -> bool {
        !self.gpus.is_empty()
    }

    /// Get GPUs of a specific vendor
    pub fn gpus_by_vendor(&self, vendor: GpuVendor) -> Vec<&GpuInfo> {
        self.gpus.iter().filter(|g| g.vendor == vendor).collect()
    }

    // Helper functions for reading sysfs

    /// Read a hexadecimal value from sysfs
    fn read_sysfs_hex(&self, path: &Path) -> Option<u32> {
        let content = fs::read_to_string(path).ok()?;
        let content = content.trim().trim_start_matches("0x");
        u32::from_str_radix(content, 16).ok()
    }

    /// Read a u64 value from sysfs
    fn read_sysfs_u64(&self, path: &Path) -> Option<u64> {
        let content = fs::read_to_string(path).ok()?;
        content.trim().parse().ok()
    }
}

impl Default for LinuxGpuMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for LinuxGpuMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinuxGpuMonitor")
            .field("gpu_count", &self.gpus.len())
            .field("gpus", &self.gpus)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vram_info_calculations() {
        let vram = VramInfo {
            total: 8 * 1024 * 1024 * 1024, // 8 GB
            used: 4 * 1024 * 1024 * 1024,  // 4 GB
            free: 4 * 1024 * 1024 * 1024,  // 4 GB
        };

        assert_eq!(vram.total_mb(), 8192);
        assert_eq!(vram.used_mb(), 4096);
        assert_eq!(vram.free_mb(), 4096);
        assert!((vram.usage_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_gpu_stats_memory_percent() {
        let stats = GpuStats {
            utilization: Some(75.0),
            memory_used: Some(4 * 1024 * 1024 * 1024),
            memory_total: Some(8 * 1024 * 1024 * 1024),
            temperature: Some(65.0),
            power_draw: Some(150.0),
            clock_mhz: Some(1500),
            memory_clock_mhz: Some(7000),
        };

        let percent = stats.memory_usage_percent().unwrap();
        assert!((percent - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_gpu_vendor_display() {
        assert_eq!(format!("{}", GpuVendor::Nvidia), "NVIDIA");
        assert_eq!(format!("{}", GpuVendor::Amd), "AMD");
        assert_eq!(format!("{}", GpuVendor::Intel), "Intel");
        assert_eq!(format!("{}", GpuVendor::Unknown), "Unknown");
    }

    #[test]
    fn test_monitor_creation() {
        // This test will pass even without GPUs
        let monitor = LinuxGpuMonitor::new();
        // Just verify it doesn't panic
        let _ = monitor.gpu_count();
        let _ = monitor.has_gpus();
    }

    #[test]
    fn test_empty_vram_usage_percent() {
        let vram = VramInfo {
            total: 0,
            used: 0,
            free: 0,
        };
        assert_eq!(vram.usage_percent(), 0.0);
    }
}
