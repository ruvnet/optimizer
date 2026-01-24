# ADR-001: AI Mode - Intelligent LLM and GPU/CPU Optimization Bridge

## Status
**Proposed** | Date: 2024-01-24

## Context

Large Language Model (LLM) inference and AI workloads have unique resource requirements that differ significantly from traditional applications:

1. **Memory Pressure**: LLMs require massive amounts of RAM/VRAM (7B model = ~14GB, 70B = ~140GB)
2. **GPU-CPU Offloading**: When VRAM is insufficient, layers offload to RAM (slower but functional)
3. **KV Cache Growth**: Attention cache grows with context length, causing OOM errors
4. **Batch Dynamics**: Inference batching affects throughput vs latency tradeoffs
5. **Quantization**: 4-bit/8-bit models trade quality for memory efficiency

Current memory optimizers don't understand AI workloads. RuVector can become the **bridge** between system resources and AI runtimes.

## Decision

Implement **AI Mode** - a specialized optimization profile that:

1. Detects AI workloads (Ollama, llama.cpp, vLLM, PyTorch, ONNX Runtime)
2. Manages GPU/CPU memory allocation for optimal inference
3. Provides real-time KV cache monitoring and optimization
4. Enables intelligent layer offloading decisions
5. Integrates with popular AI runtimes via APIs

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         RuVector AI Mode                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                     AI Workload Detector                          │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────────┐ │  │
│  │  │ Ollama  │ │llama.cpp│ │  vLLM   │ │ PyTorch │ │ ONNX/DirectML│ │  │
│  │  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └──────┬──────┘ │  │
│  │       └───────────┴───────────┴───────────┴─────────────┘        │  │
│  └──────────────────────────────────┬───────────────────────────────┘  │
│                                     │                                   │
│  ┌──────────────────────────────────▼───────────────────────────────┐  │
│  │                    Resource Bridge Controller                     │  │
│  │  ┌─────────────────────┐     ┌─────────────────────────────────┐ │  │
│  │  │   VRAM Manager      │◄───►│      RAM Manager                │ │  │
│  │  │  - Model layers     │     │  - Offloaded layers             │ │  │
│  │  │  - KV cache         │     │  - System memory                │ │  │
│  │  │  - Attention buffers│     │  - Swap prevention              │ │  │
│  │  └─────────────────────┘     └─────────────────────────────────┘ │  │
│  │                 │                           │                     │  │
│  │                 └─────────┬─────────────────┘                     │  │
│  │                           │                                       │  │
│  │              ┌────────────▼────────────┐                         │  │
│  │              │   Offload Orchestrator  │                         │  │
│  │              │  - Layer placement      │                         │  │
│  │              │  - Dynamic rebalancing  │                         │  │
│  │              │  - Thermal awareness    │                         │  │
│  │              └─────────────────────────┘                         │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                    Neural Optimization Engine                     │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │  │
│  │  │  Workload   │  │  Inference  │  │    Predictive           │  │  │
│  │  │  Classifier │  │  Predictor  │  │    Pre-allocation       │  │  │
│  │  │  (GNN/HNSW) │  │  (Patterns) │  │    (Context Growth)     │  │  │
│  │  └─────────────┘  └─────────────┘  └─────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. AI Workload Detector

Automatically detects running AI inference engines:

```rust
pub struct AIWorkloadDetector {
    known_processes: Vec<AIProcess>,
    gpu_monitor: GpuMonitor,
    inference_patterns: PatternMatcher,
}

pub enum AIRuntime {
    Ollama,           // REST API on :11434
    LlamaCpp,         // Direct process
    VLLM,             // OpenAI-compatible API
    PyTorch,          // torch.exe / python with torch
    ONNXRuntime,      // onnxruntime.dll loaded
    DirectML,         // Windows ML
    Whisper,          // Audio transcription
    StableDiffusion,  // Image generation
    ComfyUI,          // SD workflow
}

impl AIWorkloadDetector {
    pub fn detect_active_runtimes(&self) -> Vec<ActiveAIWorkload>;
    pub fn get_model_info(&self, runtime: &AIRuntime) -> Option<ModelInfo>;
    pub fn estimate_memory_requirements(&self, model: &ModelInfo) -> MemoryEstimate;
}
```

### 2. VRAM Manager

Direct GPU memory management:

```rust
pub struct VRAMManager {
    gpu_handle: GpuHandle,
    allocated_blocks: Vec<VRAMBlock>,
    kv_cache_tracker: KVCacheTracker,
}

impl VRAMManager {
    /// Get current VRAM usage
    pub fn get_usage(&self) -> VRAMStatus;

    /// Estimate available space for model loading
    pub fn available_for_model(&self) -> u64;

    /// Request VRAM defragmentation
    pub fn defragment(&mut self) -> Result<DefragResult>;

    /// Monitor KV cache growth
    pub fn track_kv_cache(&self, model_id: &str) -> KVCacheMetrics;

    /// Trigger KV cache compaction
    pub fn compact_kv_cache(&mut self, model_id: &str) -> Result<u64>;
}
```

### 3. Offload Orchestrator

Intelligent CPU-GPU memory bridging:

```rust
pub struct OffloadOrchestrator {
    vram_manager: VRAMManager,
    ram_manager: RAMManager,
    layer_map: HashMap<String, LayerPlacement>,
    thermal_monitor: ThermalMonitor,
}

pub enum LayerPlacement {
    GPU { device_id: u32 },
    CPU { numa_node: Option<u32> },
    Split { gpu_percent: f32 },
}

impl OffloadOrchestrator {
    /// Calculate optimal layer placement for a model
    pub fn plan_placement(&self, model: &ModelInfo) -> PlacementPlan;

    /// Dynamically rebalance layers based on current load
    pub fn rebalance(&mut self) -> Result<RebalanceResult>;

    /// Preemptively offload to prevent OOM
    pub fn preemptive_offload(&mut self, urgency: Urgency) -> Result<()>;

    /// Restore layers to GPU when space available
    pub fn restore_to_gpu(&mut self) -> Result<u64>;
}
```

### 4. Inference Optimizer

Runtime-specific optimizations:

```rust
pub struct InferenceOptimizer {
    runtime: AIRuntime,
    config: InferenceConfig,
}

pub struct InferenceConfig {
    /// Max batch size for throughput mode
    pub max_batch_size: usize,
    /// Target latency for interactive mode
    pub target_latency_ms: u32,
    /// KV cache size limit (0 = auto)
    pub kv_cache_limit_mb: u64,
    /// Context length limit
    pub max_context_length: usize,
    /// Quantization preference
    pub quantization: Quantization,
}

pub enum Quantization {
    FP16,
    INT8,
    INT4,
    GPTQ,
    AWQ,
    GGUF(GGUFQuant),
}

pub enum GGUFQuant {
    Q4_0, Q4_1, Q4_K_M, Q4_K_S,
    Q5_0, Q5_1, Q5_K_M, Q5_K_S,
    Q6_K, Q8_0, F16, F32,
}
```

## AI Mode Profiles

### Profile: Local LLM (Ollama/llama.cpp)

```toml
[ai_mode.local_llm]
# Memory allocation
reserve_vram_percent = 90          # Reserve 90% VRAM for model
reserve_ram_gb = 8                 # Keep 8GB RAM free for offload
kv_cache_limit_percent = 30        # Limit KV cache to 30% of VRAM

# Optimization triggers
auto_offload_threshold = 95        # Offload layers at 95% VRAM
auto_compact_threshold = 80        # Compact KV cache at 80%

# Performance tuning
prefer_throughput = false          # Optimize for latency (interactive)
dynamic_batching = true            # Enable dynamic batching
numa_aware = true                  # NUMA-aware memory allocation
```

### Profile: Batch Inference (vLLM/TGI)

```toml
[ai_mode.batch_inference]
# Memory allocation
reserve_vram_percent = 95
reserve_ram_gb = 4
kv_cache_limit_percent = 50        # Higher for batching

# Optimization triggers
auto_offload_threshold = 98
auto_compact_threshold = 90

# Performance tuning
prefer_throughput = true           # Optimize for throughput
dynamic_batching = true
continuous_batching = true
max_batch_size = 32
```

### Profile: Image Generation (SD/ComfyUI)

```toml
[ai_mode.image_generation]
# Memory allocation
reserve_vram_percent = 85
reserve_ram_gb = 16                # More RAM for image buffers

# Optimization triggers
auto_offload_threshold = 90
clear_cache_between_generations = true

# Performance tuning
prefer_throughput = false
enable_attention_slicing = true
enable_vae_tiling = true
```

## API Integration

### Ollama Integration

```rust
pub struct OllamaIntegration {
    base_url: String,
    client: HttpClient,
}

impl OllamaIntegration {
    /// Get loaded models and their memory usage
    pub async fn get_loaded_models(&self) -> Result<Vec<OllamaModel>>;

    /// Unload a model to free VRAM
    pub async fn unload_model(&self, name: &str) -> Result<()>;

    /// Preload a model before use
    pub async fn preload_model(&self, name: &str) -> Result<()>;

    /// Monitor inference metrics
    pub async fn get_metrics(&self) -> Result<OllamaMetrics>;
}
```

### vLLM Integration

```rust
pub struct VLLMIntegration {
    metrics_endpoint: String,
}

impl VLLMIntegration {
    /// Get GPU memory utilization
    pub async fn get_gpu_memory(&self) -> Result<GpuMemoryMetrics>;

    /// Get KV cache utilization
    pub async fn get_kv_cache_usage(&self) -> Result<KVCacheMetrics>;

    /// Get batch statistics
    pub async fn get_batch_stats(&self) -> Result<BatchStats>;
}
```

## CLI Commands

```bash
# AI Mode commands
ruvector-memopt ai status              # Show AI workload status
ruvector-memopt ai detect              # Detect running AI runtimes
ruvector-memopt ai optimize            # Run AI-specific optimization
ruvector-memopt ai profile <name>      # Apply optimization profile

# VRAM management
ruvector-memopt vram status            # GPU memory status
ruvector-memopt vram defrag            # Defragment VRAM
ruvector-memopt vram clear             # Clear unused VRAM

# Model management (Ollama)
ruvector-memopt ai models              # List loaded models
ruvector-memopt ai unload <model>      # Unload model from VRAM
ruvector-memopt ai preload <model>     # Preload model to VRAM

# Monitoring
ruvector-memopt ai watch               # Real-time AI metrics dashboard
ruvector-memopt ai benchmark           # Run inference benchmarks
```

## Tray Menu (AI Mode)

```
┌─────────────────────────────────┐
│ Memory: 61% | VRAM: 78%         │
├─────────────────────────────────┤
│ ✓ AI Mode                       │
│   ├─ Ollama (llama3.2:8b)      │
│   │   └─ VRAM: 6.2 GB          │
│   └─ KV Cache: 1.8 GB          │
├─────────────────────────────────┤
│ ► Optimize AI Workload          │
│ ► Compact KV Cache              │
│ ► Unload Inactive Models        │
├─────────────────────────────────┤
│   Profiles ►                    │
│   ├─ Local LLM (Active)        │
│   ├─ Batch Inference           │
│   └─ Image Generation          │
├─────────────────────────────────┤
│ System Info                     │
│ Quit                            │
└─────────────────────────────────┘
```

## Implementation Phases

### Phase 1: Foundation (v0.3.0)
- [ ] GPU detection and VRAM monitoring (NVML/AMD equivalent)
- [ ] AI process detection (Ollama, llama.cpp, PyTorch)
- [ ] Basic VRAM status in tray
- [ ] CLI commands: `ai status`, `vram status`

### Phase 2: Ollama Integration (v0.3.1)
- [ ] Ollama API integration
- [ ] Model listing and unloading
- [ ] KV cache monitoring
- [ ] Auto-unload inactive models

### Phase 3: Smart Optimization (v0.4.0)
- [ ] Predictive memory allocation
- [ ] Dynamic layer offloading
- [ ] KV cache compaction
- [ ] Profile system

### Phase 4: Advanced Features (v0.5.0)
- [ ] vLLM/TGI integration
- [ ] Stable Diffusion optimization
- [ ] Multi-GPU support
- [ ] NUMA-aware allocation

## Dependencies

```toml
[dependencies]
# GPU monitoring
nvml-wrapper = "0.9"              # NVIDIA Management Library
windows = { features = ["Win32_Graphics_Dxgi"] }  # DirectX for AMD

# HTTP client for API integration
reqwest = { version = "0.11", features = ["json"] }

# Async runtime (already have)
tokio = { version = "1.36", features = ["full"] }
```

## Success Metrics

| Metric | Target |
|--------|--------|
| VRAM utilization efficiency | >95% |
| OOM prevention rate | 99%+ |
| Inference latency impact | <5% overhead |
| KV cache compaction savings | 20-40% |
| Model load time improvement | 2x faster (preloading) |

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| GPU vendor fragmentation | Abstract via trait, support NVIDIA first |
| Runtime API changes | Version detection, graceful degradation |
| Incorrect memory estimation | Conservative estimates, user override |
| Performance regression | Extensive benchmarking, opt-out flag |

## References

- [Ollama API Documentation](https://github.com/ollama/ollama/blob/main/docs/api.md)
- [vLLM Memory Management](https://docs.vllm.ai/en/latest/serving/memory_management.html)
- [NVIDIA NVML API](https://developer.nvidia.com/nvidia-management-library-nvml)
- [llama.cpp Memory Layout](https://github.com/ggerganov/llama.cpp/blob/master/docs/memory.md)

## Decision Outcome

**Approved** - Proceed with Phase 1 implementation.

RuVector AI Mode will differentiate from generic memory optimizers by providing deep integration with AI inference runtimes, making it the go-to tool for local LLM users.
