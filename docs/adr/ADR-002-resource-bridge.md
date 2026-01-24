# ADR-002: Unified Resource Bridge - CPU/GPU/NPU Optimization Layer

## Status
**Proposed** | Date: 2024-01-24

## Context

Modern AI workloads span multiple compute resources:
- **CPU**: General computation, model loading, preprocessing
- **GPU**: Tensor operations, inference, training
- **NPU**: Dedicated AI acceleration (Intel, Qualcomm, Apple)
- **RAM**: Model weights, KV cache overflow, batch buffers
- **VRAM**: Active model layers, attention caches

Current tools optimize these in isolation. RuVector should provide a **unified bridge** that orchestrates resources holistically.

## Decision

Implement a **Resource Bridge** abstraction that:
1. Provides unified view of all compute/memory resources
2. Enables intelligent workload placement across devices
3. Supports heterogeneous computing (CPU+GPU+NPU)
4. Optimizes data movement between memory tiers

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        RuVector Resource Bridge                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Unified Resource View                           │   │
│  │  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────────────┐ │   │
│  │  │    CPU    │  │    GPU    │  │    NPU    │  │      Memory       │ │   │
│  │  │  Cores    │  │  CUDA/ROCm│  │  Intel/QC │  │   RAM + VRAM      │ │   │
│  │  │  Threads  │  │  Tensor   │  │  Tensor   │  │   + Swap          │ │   │
│  │  │  Cache    │  │  Cores    │  │  Cores    │  │   + NVMe Cache    │ │   │
│  │  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └─────────┬─────────┘ │   │
│  │        └──────────────┴──────────────┴───────────────────┘          │   │
│  └─────────────────────────────────┬───────────────────────────────────┘   │
│                                    │                                        │
│  ┌─────────────────────────────────▼───────────────────────────────────┐   │
│  │                    Workload Placement Engine                         │   │
│  │                                                                      │   │
│  │  ┌─────────────────┐  ┌──────────────────┐  ┌────────────────────┐ │   │
│  │  │   Cost Model    │  │  Latency Model   │  │  Thermal Model     │ │   │
│  │  │  - Compute cost │  │  - Data transfer │  │  - Power budget    │ │   │
│  │  │  - Memory BW    │  │  - Kernel launch │  │  - Temp limits     │ │   │
│  │  │  - Energy       │  │  - Queue depth   │  │  - Throttle pred   │ │   │
│  │  └─────────────────┘  └──────────────────┘  └────────────────────┘ │   │
│  │                                                                      │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │                  Placement Strategies                        │   │   │
│  │  │  • GPU-First: Maximize GPU utilization                      │   │   │
│  │  │  • Balanced: Distribute across devices                      │   │   │
│  │  │  • Latency: Minimize end-to-end latency                     │   │   │
│  │  │  • Efficiency: Minimize energy consumption                  │   │   │
│  │  │  • Hybrid: Dynamic based on workload                        │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    Data Movement Optimizer                           │   │
│  │                                                                      │   │
│  │  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────┐  │   │
│  │  │   Prefetch   │    │   Pipeline   │    │    Zero-Copy         │  │   │
│  │  │   Engine     │    │   Scheduler  │    │    Transfers         │  │   │
│  │  └──────────────┘    └──────────────┘    └──────────────────────┘  │   │
│  │                                                                      │   │
│  │  Memory Hierarchy:                                                   │   │
│  │  ┌─────────┐ ──► ┌─────────┐ ──► ┌─────────┐ ──► ┌─────────────┐  │   │
│  │  │ L1/L2   │     │   RAM   │     │  VRAM   │     │  NVMe/SSD   │  │   │
│  │  │ Cache   │     │  DDR5   │     │  GDDR6  │     │  Offload    │  │   │
│  │  │ ~1ns    │     │ ~100ns  │     │ ~300ns  │     │  ~10µs      │  │   │
│  │  └─────────┘     └─────────┘     └─────────┘     └─────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Unified Device Abstraction

```rust
/// Unified compute device abstraction
pub trait ComputeDevice: Send + Sync {
    fn device_type(&self) -> DeviceType;
    fn name(&self) -> &str;
    fn total_memory(&self) -> u64;
    fn available_memory(&self) -> u64;
    fn compute_capability(&self) -> ComputeCapability;
    fn power_state(&self) -> PowerState;
    fn temperature(&self) -> Option<f32>;
    fn utilization(&self) -> f32;
}

pub enum DeviceType {
    CPU,
    GPU(GpuVendor),
    NPU(NpuVendor),
    APU,  // Integrated CPU+GPU
}

pub enum GpuVendor {
    Nvidia { cuda_version: String, compute_capability: (u32, u32) },
    AMD { rocm_version: Option<String> },
    Intel { arc_generation: Option<String> },
}

pub enum NpuVendor {
    Intel { generation: String },      // Intel AI Boost
    Qualcomm { hexagon_version: u32 }, // Snapdragon
    Apple { neural_engine: bool },     // M-series
    Microsoft { dml_version: String }, // DirectML
}
```

### 2. Resource Monitor

```rust
pub struct ResourceMonitor {
    devices: Vec<Box<dyn ComputeDevice>>,
    memory_tracker: MemoryTracker,
    bandwidth_monitor: BandwidthMonitor,
    thermal_monitor: ThermalMonitor,
}

impl ResourceMonitor {
    /// Get unified system view
    pub fn get_system_state(&self) -> SystemState;

    /// Monitor memory pressure across all devices
    pub fn memory_pressure(&self) -> MemoryPressure;

    /// Get data transfer bandwidth between devices
    pub fn transfer_bandwidth(&self, from: DeviceId, to: DeviceId) -> Bandwidth;

    /// Predict thermal throttling
    pub fn thermal_headroom(&self) -> ThermalHeadroom;
}

pub struct SystemState {
    pub cpu: CpuState,
    pub gpus: Vec<GpuState>,
    pub npus: Vec<NpuState>,
    pub memory: MemoryState,
    pub power: PowerState,
}
```

### 3. Workload Classifier

```rust
pub struct WorkloadClassifier {
    patterns: PatternIndex,
    model: WorkloadModel,
}

pub enum WorkloadType {
    /// LLM text generation
    LLMInference {
        model_size: ModelSize,
        context_length: usize,
        batch_size: usize,
    },
    /// Image generation (Stable Diffusion, etc)
    ImageGeneration {
        resolution: (u32, u32),
        steps: u32,
    },
    /// Speech recognition/synthesis
    AudioProcessing {
        sample_rate: u32,
        duration_secs: f32,
    },
    /// Video processing
    VideoProcessing {
        resolution: (u32, u32),
        fps: u32,
    },
    /// Training workload
    Training {
        model_size: ModelSize,
        batch_size: usize,
    },
    /// General compute
    GeneralCompute,
}

impl WorkloadClassifier {
    /// Classify active workload from process analysis
    pub fn classify(&self, processes: &[ProcessInfo]) -> Vec<ClassifiedWorkload>;

    /// Predict resource requirements
    pub fn predict_requirements(&self, workload: &WorkloadType) -> ResourceRequirements;

    /// Suggest optimal device placement
    pub fn suggest_placement(&self, workload: &WorkloadType) -> PlacementSuggestion;
}
```

### 4. Placement Engine

```rust
pub struct PlacementEngine {
    devices: Vec<Box<dyn ComputeDevice>>,
    cost_model: CostModel,
    strategy: PlacementStrategy,
}

pub enum PlacementStrategy {
    /// Maximize GPU utilization
    GpuFirst,
    /// Balance across all devices
    Balanced,
    /// Minimize latency
    LatencyOptimized,
    /// Minimize power consumption
    PowerEfficient,
    /// Auto-select based on workload
    Adaptive,
}

pub struct PlacementPlan {
    pub assignments: Vec<(WorkloadComponent, DeviceId)>,
    pub memory_allocations: Vec<MemoryAllocation>,
    pub data_transfers: Vec<DataTransfer>,
    pub estimated_latency: Duration,
    pub estimated_power: f32,
}

impl PlacementEngine {
    /// Generate optimal placement plan
    pub fn plan(&self, workload: &ClassifiedWorkload) -> PlacementPlan;

    /// Execute placement (move data, set affinities)
    pub fn execute(&self, plan: &PlacementPlan) -> Result<()>;

    /// Dynamically rebalance running workload
    pub fn rebalance(&self) -> Result<RebalanceResult>;
}
```

### 5. Memory Tier Manager

```rust
/// Manages memory across tiers: L1/L2 → RAM → VRAM → NVMe
pub struct MemoryTierManager {
    ram_manager: RamManager,
    vram_manager: VramManager,
    nvme_cache: Option<NvmeCache>,
}

pub struct MemoryTier {
    pub tier_type: TierType,
    pub capacity: u64,
    pub used: u64,
    pub bandwidth: Bandwidth,
    pub latency: Duration,
}

pub enum TierType {
    CpuCache,
    Ram,
    Vram { device_id: u32 },
    NvmeCache,
    Swap,
}

impl MemoryTierManager {
    /// Allocate memory on optimal tier
    pub fn allocate(&self, size: u64, access_pattern: AccessPattern) -> Allocation;

    /// Migrate data between tiers
    pub fn migrate(&self, alloc: &Allocation, target_tier: TierType) -> Result<()>;

    /// Prefetch data to faster tier
    pub fn prefetch(&self, alloc: &Allocation) -> Result<()>;

    /// Evict cold data to slower tier
    pub fn evict_cold(&self) -> Result<EvictionResult>;
}
```

## Optimization Strategies

### Strategy 1: LLM Layer Offloading

```rust
pub struct LLMOffloadStrategy {
    model_layers: Vec<LayerInfo>,
    available_vram: u64,
    available_ram: u64,
}

impl LLMOffloadStrategy {
    /// Calculate optimal layer split between GPU and CPU
    pub fn plan_offload(&self) -> OffloadPlan {
        let total_layer_size: u64 = self.model_layers.iter().map(|l| l.size).sum();

        if total_layer_size <= self.available_vram {
            // Full GPU
            OffloadPlan::FullGpu
        } else {
            // Calculate split point
            let mut gpu_layers = 0;
            let mut gpu_size = 0;

            for layer in &self.model_layers {
                if gpu_size + layer.size <= self.available_vram * 95 / 100 {
                    gpu_layers += 1;
                    gpu_size += layer.size;
                } else {
                    break;
                }
            }

            OffloadPlan::Split {
                gpu_layers,
                cpu_layers: self.model_layers.len() - gpu_layers,
                estimated_slowdown: self.estimate_slowdown(gpu_layers),
            }
        }
    }
}
```

### Strategy 2: Batch Size Optimization

```rust
pub struct BatchOptimizer {
    device_memory: u64,
    model_size: u64,
    kv_cache_per_token: u64,
}

impl BatchOptimizer {
    /// Calculate maximum batch size for given memory
    pub fn max_batch_size(&self, context_length: usize) -> usize {
        let kv_cache_total = self.kv_cache_per_token * context_length as u64;
        let available = self.device_memory.saturating_sub(self.model_size);
        (available / kv_cache_total) as usize
    }

    /// Find optimal batch size for latency target
    pub fn optimal_batch_size(&self, target_latency_ms: u32) -> usize;

    /// Dynamic batch sizing based on queue depth
    pub fn dynamic_batch(&self, queue_depth: usize) -> usize;
}
```

### Strategy 3: Thermal-Aware Scheduling

```rust
pub struct ThermalScheduler {
    thermal_monitor: ThermalMonitor,
    power_budget: f32,
    temp_limits: TempLimits,
}

impl ThermalScheduler {
    /// Predict time to thermal throttle
    pub fn time_to_throttle(&self) -> Option<Duration>;

    /// Adjust workload to prevent throttling
    pub fn preemptive_throttle(&self) -> ThrottleAction;

    /// Distribute load across devices for thermal balance
    pub fn thermal_balance(&self, workload: &Workload) -> BalancedPlan;
}

pub enum ThrottleAction {
    None,
    ReduceBatchSize { new_size: usize },
    OffloadLayers { count: usize },
    PauseAndCool { duration: Duration },
}
```

## CLI Commands

```bash
# Resource overview
ruvector-memopt bridge status         # Unified resource view
ruvector-memopt bridge devices        # List all compute devices
ruvector-memopt bridge topology       # Show device interconnect

# Workload analysis
ruvector-memopt bridge analyze        # Analyze current workloads
ruvector-memopt bridge recommend      # Get optimization recommendations

# Placement control
ruvector-memopt bridge plan <model>   # Plan optimal placement
ruvector-memopt bridge execute        # Execute placement plan
ruvector-memopt bridge rebalance      # Rebalance running workloads

# Memory tiers
ruvector-memopt bridge memory         # Memory tier status
ruvector-memopt bridge migrate        # Migrate data between tiers
ruvector-memopt bridge prefetch       # Prefetch to fast tier

# Monitoring
ruvector-memopt bridge watch          # Real-time monitoring
ruvector-memopt bridge benchmark      # Cross-device benchmarks
```

## Configuration

```toml
[bridge]
# Strategy selection
strategy = "adaptive"  # gpu_first, balanced, latency, power, adaptive

# Memory management
ram_reserve_gb = 8           # Keep free for system
vram_reserve_percent = 5     # Keep free for VRAM
enable_nvme_cache = true     # Use NVMe as memory tier
nvme_cache_path = "C:/RuVectorCache"
nvme_cache_size_gb = 50

# Thermal management
thermal_aware = true
max_gpu_temp_c = 83
max_cpu_temp_c = 95
preemptive_throttle = true

# Power management
power_budget_watts = 300     # Total system power budget
efficiency_mode = false      # Prioritize perf/watt

# Device preferences
[bridge.device_priority]
nvidia_gpu = 1               # Highest priority
intel_npu = 2
amd_gpu = 3
cpu = 4                      # Lowest priority
```

## Performance Targets

| Metric | Target |
|--------|--------|
| Placement decision latency | <10ms |
| Memory migration overhead | <5% throughput loss |
| Thermal prediction accuracy | >90% |
| Resource utilization | >85% across devices |
| OOM prevention | 99.9% |

## Implementation Phases

### Phase 1: Device Discovery (v0.3.0)
- [ ] NVIDIA GPU detection (NVML)
- [ ] AMD GPU detection (ROCm/ADL)
- [ ] Intel GPU/NPU detection
- [ ] Unified device abstraction

### Phase 2: Memory Management (v0.3.1)
- [ ] Cross-device memory tracking
- [ ] RAM ↔ VRAM migration
- [ ] NVMe cache tier

### Phase 3: Workload Analysis (v0.4.0)
- [ ] Process classification
- [ ] Resource prediction
- [ ] Placement engine

### Phase 4: Advanced Features (v0.5.0)
- [ ] Thermal scheduling
- [ ] Power optimization
- [ ] Multi-GPU coordination

## Consequences

### Positive
- Unified resource management across heterogeneous hardware
- Automatic optimization for AI workloads
- Prevention of OOM and thermal issues
- Better hardware utilization

### Negative
- Increased complexity
- Hardware-specific code paths
- Potential for over-optimization

### Neutral
- Requires elevated permissions for some features
- May conflict with other GPU management tools

## References

- [NVIDIA CUDA Best Practices](https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/)
- [AMD ROCm Documentation](https://rocm.docs.amd.com/)
- [Intel oneAPI](https://www.intel.com/content/www/us/en/developer/tools/oneapi/overview.html)
- [DirectML](https://docs.microsoft.com/en-us/windows/ai/directml/dml)
