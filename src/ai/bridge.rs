//! Resource Bridge - CPU/GPU/NPU Unified Resource Management
//!
//! Orchestrates memory and compute resources across different devices
//! for optimal AI workload performance.

use serde::{Deserialize, Serialize};
use super::gpu::{GpuMonitor, VramStatus, GpuVendor};
use std::collections::HashMap;

/// Device types for resource allocation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceType {
    /// CPU with system RAM
    CPU,
    /// GPU with VRAM
    GPU(u32), // GPU index
    /// NPU (Neural Processing Unit)
    NPU,
    /// System RAM (for offloading)
    RAM,
    /// NVMe storage (for model weights)
    Storage,
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceType::CPU => write!(f, "CPU"),
            DeviceType::GPU(idx) => write!(f, "GPU:{}", idx),
            DeviceType::NPU => write!(f, "NPU"),
            DeviceType::RAM => write!(f, "RAM"),
            DeviceType::Storage => write!(f, "Storage"),
        }
    }
}

/// Resource tier for memory hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MemoryTier {
    /// L1/L2 cache - fastest, smallest
    Cache,
    /// VRAM - very fast, limited
    VRAM,
    /// System RAM - fast, large
    RAM,
    /// NVMe/SSD - slower, very large
    Storage,
}

/// Placement strategy for workloads
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementStrategy {
    /// Maximize GPU usage
    GPUFirst,
    /// Balance between CPU and GPU
    Balanced,
    /// Minimize inference latency
    LatencyOptimized,
    /// Minimize power consumption
    PowerEfficient,
    /// Maximize throughput
    ThroughputOptimized,
}

/// Resource allocation for a workload component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    /// Target device
    pub device: DeviceType,
    /// Memory allocated in bytes
    pub memory_bytes: u64,
    /// Compute allocation (0-100%)
    pub compute_percent: u32,
    /// Priority level
    pub priority: u32,
}

/// Placement plan for a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementPlan {
    /// Model identifier
    pub model_id: String,
    /// Layer allocations
    pub layers: Vec<LayerAllocation>,
    /// KV cache allocation
    pub kv_cache: ResourceAllocation,
    /// Estimated total VRAM usage
    pub estimated_vram_mb: u64,
    /// Estimated total RAM usage
    pub estimated_ram_mb: u64,
    /// Estimated inference latency (ms)
    pub estimated_latency_ms: u32,
}

/// Allocation for a model layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerAllocation {
    /// Layer index
    pub layer_index: u32,
    /// Layer name
    pub layer_name: String,
    /// Target device
    pub device: DeviceType,
    /// Memory required in bytes
    pub memory_bytes: u64,
}

/// System resource snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemResources {
    /// Total system RAM in bytes
    pub total_ram: u64,
    /// Available RAM in bytes
    pub available_ram: u64,
    /// Per-GPU VRAM status
    pub gpu_vram: Vec<VramStatus>,
    /// CPU utilization percentage
    pub cpu_utilization: f32,
    /// CPU temperature (if available)
    pub cpu_temp: Option<f32>,
}

/// Resource Bridge for unified resource management
pub struct ResourceBridge {
    gpu_monitor: Option<GpuMonitor>,
    strategy: PlacementStrategy,
    /// Reserved VRAM percentage (keep free)
    vram_reserve_percent: u32,
    /// Reserved RAM in bytes
    ram_reserve_bytes: u64,
    /// Active allocations
    allocations: HashMap<String, PlacementPlan>,
}

impl ResourceBridge {
    /// Create a new resource bridge
    pub fn new(strategy: PlacementStrategy) -> Self {
        Self {
            gpu_monitor: GpuMonitor::new().ok(),
            strategy,
            vram_reserve_percent: 5,
            ram_reserve_bytes: 4 * 1024 * 1024 * 1024, // 4 GB
            allocations: HashMap::new(),
        }
    }

    /// Set VRAM reserve percentage
    pub fn set_vram_reserve(&mut self, percent: u32) {
        self.vram_reserve_percent = percent.min(50);
    }

    /// Set RAM reserve in GB
    pub fn set_ram_reserve_gb(&mut self, gb: u64) {
        self.ram_reserve_bytes = gb * 1024 * 1024 * 1024;
    }

    /// Get current system resources
    pub fn get_resources(&self) -> SystemResources {
        let gpu_vram = self.gpu_monitor
            .as_ref()
            .map(|m| m.status())
            .unwrap_or_default();

        // Get system memory info
        let (total_ram, available_ram) = self.get_system_memory();

        SystemResources {
            total_ram,
            available_ram,
            gpu_vram,
            cpu_utilization: self.get_cpu_utilization(),
            cpu_temp: None, // Would need WMI or similar
        }
    }

    /// Get system memory info
    #[cfg(windows)]
    fn get_system_memory(&self) -> (u64, u64) {
        use windows::Win32::System::SystemInformation::{
            GlobalMemoryStatusEx, MEMORYSTATUSEX,
        };

        unsafe {
            let mut status = MEMORYSTATUSEX {
                dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
                ..Default::default()
            };

            if GlobalMemoryStatusEx(&mut status).is_ok() {
                return (status.ullTotalPhys, status.ullAvailPhys);
            }
        }

        (0, 0)
    }

    #[cfg(not(windows))]
    fn get_system_memory(&self) -> (u64, u64) {
        (0, 0)
    }

    /// Get CPU utilization
    #[cfg(windows)]
    fn get_cpu_utilization(&self) -> f32 {
        // Simplified - would need proper PDH counters for accuracy
        0.0
    }

    #[cfg(not(windows))]
    fn get_cpu_utilization(&self) -> f32 {
        0.0
    }

    /// Calculate available VRAM considering reserve
    pub fn available_vram(&self) -> u64 {
        let resources = self.get_resources();
        let total_vram: u64 = resources.gpu_vram.iter().map(|v| v.total).sum();
        let used_vram: u64 = resources.gpu_vram.iter().map(|v| v.used).sum();
        let reserved = (total_vram as f64 * self.vram_reserve_percent as f64 / 100.0) as u64;

        total_vram.saturating_sub(used_vram).saturating_sub(reserved)
    }

    /// Calculate available RAM considering reserve
    pub fn available_ram(&self) -> u64 {
        let resources = self.get_resources();
        resources.available_ram.saturating_sub(self.ram_reserve_bytes)
    }

    /// Plan placement for a model
    pub fn plan_placement(
        &self,
        model_id: &str,
        model_size_bytes: u64,
        num_layers: u32,
        kv_cache_bytes: u64,
    ) -> PlacementPlan {
        let available_vram = self.available_vram();
        let _available_ram = self.available_ram();

        let layer_size = model_size_bytes / num_layers as u64;
        let mut layers = Vec::new();
        let mut vram_used: u64 = 0;
        let mut ram_used: u64 = 0;

        // Determine how many layers fit in VRAM
        let vram_for_layers = available_vram.saturating_sub(kv_cache_bytes);
        let max_gpu_layers = (vram_for_layers / layer_size) as u32;

        let gpu_layers = match self.strategy {
            PlacementStrategy::GPUFirst => max_gpu_layers.min(num_layers),
            PlacementStrategy::Balanced => (max_gpu_layers / 2).min(num_layers),
            PlacementStrategy::LatencyOptimized => max_gpu_layers.min(num_layers),
            PlacementStrategy::PowerEfficient => (max_gpu_layers / 3).min(num_layers),
            PlacementStrategy::ThroughputOptimized => max_gpu_layers.min(num_layers),
        };

        // Allocate layers
        for i in 0..num_layers {
            let device = if i < gpu_layers {
                vram_used += layer_size;
                DeviceType::GPU(0)
            } else {
                ram_used += layer_size;
                DeviceType::RAM
            };

            layers.push(LayerAllocation {
                layer_index: i,
                layer_name: format!("layer_{}", i),
                device,
                memory_bytes: layer_size,
            });
        }

        // Allocate KV cache (prefer VRAM)
        let kv_device = if vram_used + kv_cache_bytes <= available_vram {
            vram_used += kv_cache_bytes;
            DeviceType::GPU(0)
        } else {
            ram_used += kv_cache_bytes;
            DeviceType::RAM
        };

        // Estimate latency based on placement
        let gpu_ratio = gpu_layers as f64 / num_layers as f64;
        let base_latency = 50; // ms per token
        let estimated_latency = ((1.0 - gpu_ratio * 0.8) * base_latency as f64) as u32;

        PlacementPlan {
            model_id: model_id.to_string(),
            layers,
            kv_cache: ResourceAllocation {
                device: kv_device,
                memory_bytes: kv_cache_bytes,
                compute_percent: 0,
                priority: 1,
            },
            estimated_vram_mb: vram_used / (1024 * 1024),
            estimated_ram_mb: ram_used / (1024 * 1024),
            estimated_latency_ms: estimated_latency,
        }
    }

    /// Check if rebalancing is needed
    pub fn needs_rebalance(&self) -> bool {
        let resources = self.get_resources();

        // Check VRAM pressure
        for vram in &resources.gpu_vram {
            if vram.usage_percent() > 95.0 {
                return true;
            }
        }

        // Check RAM pressure
        let ram_usage = 100.0 - (resources.available_ram as f64 / resources.total_ram as f64 * 100.0);
        if ram_usage > 90.0 {
            return true;
        }

        false
    }

    /// Suggest offloading if under pressure
    pub fn suggest_offload(&self) -> Vec<OffloadSuggestion> {
        let mut suggestions = Vec::new();
        let resources = self.get_resources();

        for (i, vram) in resources.gpu_vram.iter().enumerate() {
            if vram.usage_percent() > 90.0 {
                let to_offload = vram.used.saturating_sub(
                    (vram.total as f64 * 0.8) as u64
                );

                suggestions.push(OffloadSuggestion {
                    from: DeviceType::GPU(i as u32),
                    to: DeviceType::RAM,
                    bytes: to_offload,
                    reason: format!(
                        "GPU {} at {:.1}% VRAM usage",
                        i,
                        vram.usage_percent()
                    ),
                });
            }
        }

        suggestions
    }

    /// Register an active allocation
    pub fn register_allocation(&mut self, plan: PlacementPlan) {
        self.allocations.insert(plan.model_id.clone(), plan);
    }

    /// Remove an allocation
    pub fn remove_allocation(&mut self, model_id: &str) {
        self.allocations.remove(model_id);
    }

    /// Get all active allocations
    pub fn active_allocations(&self) -> &HashMap<String, PlacementPlan> {
        &self.allocations
    }
}

/// Offload suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffloadSuggestion {
    /// Source device
    pub from: DeviceType,
    /// Target device
    pub to: DeviceType,
    /// Bytes to offload
    pub bytes: u64,
    /// Reason for suggestion
    pub reason: String,
}

impl std::fmt::Display for OffloadSuggestion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Offload {} MB from {} to {}: {}",
            self.bytes / (1024 * 1024),
            self.from,
            self.to,
            self.reason
        )
    }
}

impl std::fmt::Display for PlacementPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Placement Plan for {}", self.model_id)?;
        writeln!(f, "  VRAM: {} MB", self.estimated_vram_mb)?;
        writeln!(f, "  RAM:  {} MB", self.estimated_ram_mb)?;
        writeln!(f, "  Est. Latency: {} ms/token", self.estimated_latency_ms)?;

        let gpu_layers = self.layers.iter()
            .filter(|l| matches!(l.device, DeviceType::GPU(_)))
            .count();
        let cpu_layers = self.layers.len() - gpu_layers;

        writeln!(f, "  Layers: {} GPU, {} CPU", gpu_layers, cpu_layers)?;
        writeln!(f, "  KV Cache: {}", self.kv_cache.device)?;

        Ok(())
    }
}

impl Default for ResourceBridge {
    fn default() -> Self {
        Self::new(PlacementStrategy::GPUFirst)
    }
}
