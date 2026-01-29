# ADR-022: GPU Memory Optimizer

## Status
**Proposed**

## Date
2026-01-28

## Context

GPU memory (VRAM) is a scarce, expensive resource that is increasingly contested between AI workloads (Ollama, Stable Diffusion, LLM inference), gaming, video editing, and even browser hardware acceleration. Unlike system RAM, VRAM has no automatic paging - when it's full, applications crash or fail with cryptic errors.

RuVector already has NVML bindings (`nvml-wrapper` in Cargo.toml) and DXGI support (`Win32_Graphics_Dxgi` in features). The GPU Memory Optimizer extends memory management from system RAM into VRAM, providing:
- Real-time VRAM monitoring per-process
- VRAM allocation tracking and leak detection
- Intelligent VRAM reclamation between workloads
- AI model layer management (offload to RAM when VRAM is scarce)

### Target Hardware
- **NVIDIA**: Full support via NVML + CUDA APIs
- **AMD**: DXGI-based monitoring + AMD ADL
- **Intel Arc**: DXGI + Intel Graphics Command Center API
- **Apple Silicon**: Metal API (macOS only, shared memory model)

## Decision

### 1. VRAM Monitor

```rust
pub struct GpuMemoryMonitor {
    devices: Vec<GpuDevice>,
    per_process_vram: HashMap<u32, VramAllocation>,
    history: VecDeque<GpuMemorySnapshot>,
}

pub struct GpuDevice {
    pub index: u32,
    pub name: String,
    pub vendor: GpuVendor,
    pub total_vram_mb: u64,
    pub used_vram_mb: u64,
    pub free_vram_mb: u64,
    pub temperature_c: f64,
    pub power_watts: f64,
    pub clock_mhz: u32,
    pub memory_clock_mhz: u32,
    pub utilization_percent: u32,
}

pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,  // macOS unified memory
}

pub struct VramAllocation {
    pub pid: u32,
    pub process_name: String,
    pub dedicated_mb: u64,     // Dedicated VRAM usage
    pub shared_mb: u64,        // Shared (system RAM used as VRAM)
    pub total_mb: u64,
    pub category: VramCategory,
}

pub enum VramCategory {
    AiModel,           // Ollama, llama.cpp, vLLM model layers
    GameTextures,      // Game texture and buffer data
    VideoEditing,      // Premiere/Resolve timeline + preview
    BrowserAccel,      // Chrome/Edge GPU compositing
    DesktopCompositor, // DWM (Windows desktop)
    ThreeDModeling,    // Blender, Maya viewport
    Other,
}
```

### 2. VRAM Reclamation

```rust
pub struct VramOptimizer {
    monitor: GpuMemoryMonitor,
    neural: NeuralDecisionEngine,
}

impl VramOptimizer {
    pub fn optimize(&self, target_free_mb: u64) -> Vec<VramAction> {
        let mut actions = vec![];
        let current_free = self.monitor.devices[0].free_vram_mb;

        if current_free >= target_free_mb {
            return actions;
        }

        let deficit = target_free_mb - current_free;

        // Strategy 1: Kill GPU hardware acceleration in background browsers
        let browser_vram = self.get_browser_vram();
        if browser_vram > 200 {  // Browsers typically use 200-800MB VRAM
            actions.push(VramAction::DisableBrowserGpuAccel);
        }

        // Strategy 2: Flush stale GPU caches
        actions.push(VramAction::FlushGpuCaches);

        // Strategy 3: Offload AI model layers to system RAM
        if let Some(ai_proc) = self.get_ai_process() {
            let offloadable = ai_proc.total_mb - ai_proc.min_vram_mb;
            if offloadable > 0 {
                actions.push(VramAction::OffloadModelLayers {
                    pid: ai_proc.pid,
                    layers_to_offload: self.compute_offload_count(deficit, &ai_proc),
                });
            }
        }

        // Strategy 4: Reduce game texture quality (if applicable)
        if let Some(game) = self.get_game_process() {
            actions.push(VramAction::SuggestLowerTextures {
                game_name: game.name,
                current_vram_mb: game.total_mb,
            });
        }

        actions
    }
}

pub enum VramAction {
    DisableBrowserGpuAccel,
    FlushGpuCaches,
    OffloadModelLayers { pid: u32, layers_to_offload: u32 },
    SuggestLowerTextures { game_name: String, current_vram_mb: u64 },
    KillIdleGpuProcess { pid: u32, name: String },
    NotifyUser { message: String },
}
```

### 3. AI Model Memory Management

For Ollama/llama.cpp workloads, manage model layer placement:

```rust
pub struct ModelMemoryManager {
    pub model_name: String,
    pub total_layers: u32,
    pub layers_in_vram: u32,
    pub layers_in_ram: u32,
    pub vram_per_layer_mb: f64,
    pub ram_per_layer_mb: f64,
    pub inference_speed_tokens_per_sec: f64,
}

impl ModelMemoryManager {
    /// Dynamically adjust layer placement based on available VRAM
    pub fn rebalance(&mut self, available_vram_mb: u64) {
        let max_vram_layers = (available_vram_mb as f64 / self.vram_per_layer_mb) as u32;
        let optimal = max_vram_layers.min(self.total_layers);

        if optimal != self.layers_in_vram {
            self.layers_in_vram = optimal;
            self.layers_in_ram = self.total_layers - optimal;

            // Estimate performance impact
            let vram_ratio = self.layers_in_vram as f64 / self.total_layers as f64;
            let est_speed = self.baseline_speed * (0.3 + 0.7 * vram_ratio);

            show_banner(
                "Model Rebalanced",
                &format!("{}/{} layers in VRAM (~{:.0} tok/s)",
                    self.layers_in_vram, self.total_layers, est_speed),
                None,
            );
        }
    }
}
```

### 4. VRAM Leak Detection

Extend the spectral leak detector (ADR-018) to VRAM:

```rust
impl GpuLeakDetector {
    pub fn detect(&self, history: &[GpuMemorySnapshot]) -> Option<VramLeakAlert> {
        for (pid, allocations) in &self.per_process_history {
            let vram_series: Vec<f64> = allocations.iter()
                .map(|a| a.dedicated_mb as f64)
                .collect();

            let (slope, r_squared) = linear_regression(&vram_series);

            if slope > 1.0 && r_squared > 0.8 {
                // VRAM growing >1MB per sample with high linearity
                return Some(VramLeakAlert {
                    pid: *pid,
                    process_name: allocations.last().unwrap().process_name.clone(),
                    growth_rate_mb_per_hour: slope * 12.0,
                    current_usage_mb: allocations.last().unwrap().dedicated_mb,
                    total_vram_mb: self.total_vram,
                    estimated_crash_hours: (self.total_vram as f64 - allocations.last().unwrap().dedicated_mb as f64) / (slope * 12.0),
                });
            }
        }
        None
    }
}
```

### 5. UI Widget

```
┌──────────────────────────────────────────────────┐
│  GPU Memory                                       │
│                                                   │
│  NVIDIA RTX 4090 - 24,576 MB VRAM                │
│  ████████████████░░░░░░░░  67% (16,434 MB used)  │
│                                                   │
│  Process               VRAM      Category         │
│  ollama.exe           8,192 MB   AI Model         │
│  chrome.exe             612 MB   Browser          │
│  dwm.exe                384 MB   Desktop          │
│  code.exe               256 MB   Editor           │
│  explorer.exe            48 MB   System           │
│  (Free)              7,084 MB                     │
│                                                   │
│  [Optimize VRAM]  [AI Layer Manager]  [Monitor]  │
│                                                   │
│  VRAM Usage (1 hour)                             │
│  24GB ┤                                          │
│  16GB ┤────────────────╮                          │
│   8GB ┤                ╰──────                    │
│   0GB ┤                                          │
│       └─────────────────────────                  │
└──────────────────────────────────────────────────┘
```

## Consequences

### Positive
- First consumer tool to manage VRAM as a resource alongside RAM
- AI model layer management is unique and valuable for local LLM users
- VRAM leak detection prevents mysterious GPU crashes
- Browser GPU acceleration control reclaims significant VRAM
- Supports the growing AI/gaming crossover audience

### Negative
- NVML only works with NVIDIA GPUs
- DXGI provides limited per-process data (no control, just monitoring)
- Model layer offloading requires integration with Ollama/llama.cpp APIs
- GPU vendor fragmentation means different code paths

### Security Considerations
- **API authentication**: Ollama API communication uses localhost binding + optional auth token; no remote API exposure
- **NVML access control**: NVML queries require no special privileges (read-only); GPU state modification (clocks, power) requires admin and is opt-in only
- **Process verification**: Before disabling browser GPU acceleration, verify the process is a genuine browser via Authenticode (not a GPU-abusing malware mimicking `chrome.exe`)
- **VRAM allocation tracking**: Per-process VRAM data stored locally; no transmission of GPU usage patterns
- **Model file integrity**: AI model files referenced by the optimizer are not modified; only metadata (layer counts, VRAM allocation) is tracked

### Risks
- Incorrect VRAM management could crash GPU applications
- Some applications don't gracefully handle VRAM pressure
- Driver-level access is needed for some optimizations
- Apple Silicon unified memory doesn't have separate VRAM concept

## Implementation Plan

### Phase 1: Monitoring
- [ ] NVML-based VRAM monitoring per GPU
- [ ] DXGI-based per-process VRAM tracking
- [ ] GPU temperature and power monitoring
- [ ] VRAM usage history

### Phase 2: Optimization
- [ ] Browser GPU acceleration toggle
- [ ] GPU cache flushing
- [ ] Background GPU process identification

### Phase 3: AI Integration
- [ ] Ollama model detection and layer counting
- [ ] Dynamic layer offloading recommendations
- [ ] Inference speed estimation per layer count

### Phase 4: UI
- [ ] GPU page in Control Center
- [ ] Per-process VRAM bar chart
- [ ] VRAM usage timeline
- [ ] AI layer manager dialog

## References

- [NVML API](https://docs.nvidia.com/deploy/nvml-api/)
- [DXGI Process Memory](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-queryvideomemoryinfo)
- [Ollama API](https://github.com/ollama/ollama/blob/main/docs/api.md)
- [llama.cpp GPU layers](https://github.com/ggerganov/llama.cpp#gpu-offloading)
