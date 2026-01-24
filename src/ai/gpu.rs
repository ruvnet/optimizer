//! GPU Monitoring and VRAM Management
//!
//! Provides GPU detection, VRAM monitoring, and memory management for AI workloads.

use serde::{Deserialize, Serialize};

/// GPU vendor types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuVendor {
    Nvidia,
    AMD,
    Intel,
    Unknown,
}

/// GPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU index
    pub index: u32,
    /// GPU name
    pub name: String,
    /// Vendor
    pub vendor: GpuVendor,
    /// Total VRAM in MB
    pub total_vram_mb: u64,
    /// Driver version
    pub driver_version: String,
    /// Compute capability (NVIDIA)
    pub compute_capability: Option<String>,
}

/// VRAM status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VramStatus {
    /// GPU index
    pub gpu_index: u32,
    /// Total VRAM in bytes
    pub total: u64,
    /// Used VRAM in bytes
    pub used: u64,
    /// Free VRAM in bytes
    pub free: u64,
    /// GPU temperature in Celsius
    pub temperature: Option<u32>,
    /// GPU utilization percentage
    pub utilization: Option<u32>,
    /// Power usage in watts
    pub power_watts: Option<u32>,
}

impl VramStatus {
    /// Get usage percentage
    pub fn usage_percent(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.used as f64 / self.total as f64) * 100.0
    }

    /// Get free percentage
    pub fn free_percent(&self) -> f64 {
        100.0 - self.usage_percent()
    }

    /// Get used in MB
    pub fn used_mb(&self) -> u64 {
        self.used / (1024 * 1024)
    }

    /// Get free in MB
    pub fn free_mb(&self) -> u64 {
        self.free / (1024 * 1024)
    }

    /// Get total in MB
    pub fn total_mb(&self) -> u64 {
        self.total / (1024 * 1024)
    }
}

/// GPU monitor for tracking VRAM and GPU metrics
pub struct GpuMonitor {
    gpus: Vec<GpuInfo>,
    #[cfg(feature = "nvml")]
    nvml: Option<nvml_wrapper::Nvml>,
}

impl GpuMonitor {
    /// Create a new GPU monitor
    pub fn new() -> Result<Self, String> {
        #[allow(unused_mut)]
        let mut gpus = Vec::new();

        // Try to detect GPUs
        #[cfg(windows)]
        {
            // Try NVIDIA first via NVML
            #[cfg(feature = "nvml")]
            {
                if let Ok(nvml) = nvml_wrapper::Nvml::init() {
                    if let Ok(count) = nvml.device_count() {
                        for i in 0..count {
                            if let Ok(device) = nvml.device_by_index(i) {
                                let name = device.name().unwrap_or_default();
                                let memory = device.memory_info().ok();
                                let total_vram = memory.map(|m| m.total / (1024 * 1024)).unwrap_or(0);

                                gpus.push(GpuInfo {
                                    index: i,
                                    name,
                                    vendor: GpuVendor::Nvidia,
                                    total_vram_mb: total_vram,
                                    driver_version: nvml.sys_driver_version().unwrap_or_default(),
                                    compute_capability: device.cuda_compute_capability()
                                        .ok()
                                        .map(|cc| format!("{}.{}", cc.major, cc.minor)),
                                });
                            }
                        }
                    }

                    return Ok(Self {
                        gpus,
                        nvml: Some(nvml),
                    });
                }
            }

            // Fallback: Use DirectX/DXGI to detect GPUs
            gpus = Self::detect_via_dxgi()?;
        }


        Ok(Self {
            gpus,
            #[cfg(feature = "nvml")]
            nvml: None,
        })
    }

    /// Detect GPUs via DXGI (Windows)
    #[cfg(windows)]
    fn detect_via_dxgi() -> Result<Vec<GpuInfo>, String> {
        use windows::Win32::Graphics::Dxgi::{
            CreateDXGIFactory1, IDXGIFactory1,
        };

        let mut gpus = Vec::new();

        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1()
                .map_err(|e| format!("Failed to create DXGI factory: {}", e))?;

            let mut i = 0u32;
            loop {
                match factory.EnumAdapters1(i) {
                    Ok(adapter) => {
                        if let Ok(desc) = adapter.GetDesc1() {
                            let name = String::from_utf16_lossy(
                                &desc.Description[..desc.Description.iter()
                                    .position(|&c| c == 0)
                                    .unwrap_or(desc.Description.len())]
                            );

                            // Determine vendor
                            let vendor = match desc.VendorId {
                                0x10DE => GpuVendor::Nvidia,
                                0x1002 | 0x1022 => GpuVendor::AMD,
                                0x8086 => GpuVendor::Intel,
                                _ => GpuVendor::Unknown,
                            };

                            // Skip software adapters
                            if desc.Flags & 0x2 != 0 { // DXGI_ADAPTER_FLAG_SOFTWARE
                                i += 1;
                                continue;
                            }

                            gpus.push(GpuInfo {
                                index: i,
                                name,
                                vendor,
                                total_vram_mb: desc.DedicatedVideoMemory as u64 / (1024 * 1024),
                                driver_version: String::new(),
                                compute_capability: None,
                            });
                        }
                        i += 1;
                    }
                    Err(_) => break,
                }
            }
        }

        Ok(gpus)
    }

    #[cfg(not(windows))]
    fn detect_via_dxgi() -> Result<Vec<GpuInfo>, String> {
        Ok(Vec::new())
    }

    /// Get list of detected GPUs
    pub fn gpus(&self) -> &[GpuInfo] {
        &self.gpus
    }

    /// Get VRAM status for all GPUs
    pub fn status(&self) -> Vec<VramStatus> {
        let mut statuses = Vec::new();

        #[cfg(feature = "nvml")]
        if let Some(ref nvml) = self.nvml {
            for gpu in &self.gpus {
                if gpu.vendor == GpuVendor::Nvidia {
                    if let Ok(device) = nvml.device_by_index(gpu.index) {
                        let memory = device.memory_info().ok();
                        let temp = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu).ok();
                        let util = device.utilization_rates().ok();
                        let power = device.power_usage().ok();

                        statuses.push(VramStatus {
                            gpu_index: gpu.index,
                            total: memory.map(|m| m.total).unwrap_or(0),
                            used: memory.map(|m| m.used).unwrap_or(0),
                            free: memory.map(|m| m.free).unwrap_or(0),
                            temperature: temp,
                            utilization: util.map(|u| u.gpu),
                            power_watts: power.map(|p| p / 1000), // mW to W
                        });
                    }
                }
            }
        }

        // Fallback for non-NVML systems or AMD/Intel
        if statuses.is_empty() {
            for gpu in &self.gpus {
                statuses.push(VramStatus {
                    gpu_index: gpu.index,
                    total: gpu.total_vram_mb * 1024 * 1024,
                    used: 0, // Can't determine without vendor API
                    free: gpu.total_vram_mb * 1024 * 1024,
                    temperature: None,
                    utilization: None,
                    power_watts: None,
                });
            }
        }

        statuses
    }

    /// Get status for a specific GPU
    pub fn status_for(&self, gpu_index: u32) -> Option<VramStatus> {
        self.status().into_iter().find(|s| s.gpu_index == gpu_index)
    }

    /// Check if any GPU is under memory pressure
    pub fn is_under_pressure(&self, threshold_percent: f64) -> bool {
        self.status().iter().any(|s| s.usage_percent() > threshold_percent)
    }

    /// Get total VRAM across all GPUs
    pub fn total_vram(&self) -> u64 {
        self.gpus.iter().map(|g| g.total_vram_mb).sum::<u64>() * 1024 * 1024
    }

    /// Get total used VRAM across all GPUs
    pub fn total_used_vram(&self) -> u64 {
        self.status().iter().map(|s| s.used).sum()
    }

    /// Get total free VRAM across all GPUs
    pub fn total_free_vram(&self) -> u64 {
        self.status().iter().map(|s| s.free).sum()
    }
}

impl Default for GpuMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            gpus: Vec::new(),
            #[cfg(feature = "nvml")]
            nvml: None,
        })
    }
}

impl std::fmt::Display for VramStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GPU {}: {}/{} MB ({:.1}%)",
            self.gpu_index,
            self.used_mb(),
            self.total_mb(),
            self.usage_percent()
        )?;

        if let Some(temp) = self.temperature {
            write!(f, " | {}°C", temp)?;
        }

        if let Some(util) = self.utilization {
            write!(f, " | {}% util", util)?;
        }

        Ok(())
    }
}
