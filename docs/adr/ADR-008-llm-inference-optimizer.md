# ADR-008: LLM Inference Optimizer - Deep AI Workload Management

## Status
Proposed

## Date
2025-01-25

## Context

Local LLM inference has unique memory characteristics:
- Large model weights (7B = ~14GB, 70B = ~140GB)
- KV cache grows with context length
- Layer-by-layer memory access patterns
- GPU/CPU split for partial offloading
- Batch size affects memory/throughput tradeoff

Current optimizers treat LLMs like any other process, missing:
- KV cache optimization opportunities
- Predictable layer access patterns
- Context length memory prediction
- Optimal CPU/GPU layer placement

## Decision

Implement **Deep LLM Inference Optimization** with model-aware memory management.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                   LLM Inference Optimizer                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────────────┐   │
│  │   Runtime   │   │    Model    │   │     Placement       │   │
│  │   Detector  │──▶│   Profiler  │──▶│     Engine          │   │
│  │             │   │             │   │                     │   │
│  │ • Ollama    │   │ • Layers    │   │ • GPU layers        │   │
│  │ • llama.cpp │   │ • KV cache  │   │ • CPU layers        │   │
│  │ • vLLM      │   │ • Context   │   │ • NPU offload       │   │
│  │ • PyTorch   │   │ • Batch     │   │ • Disk cache        │   │
│  └─────────────┘   └─────────────┘   └─────────────────────┘   │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Memory Tiers                            │  │
│  │                                                            │  │
│  │   ┌────────┐   ┌────────┐   ┌────────┐   ┌────────┐      │  │
│  │   │  VRAM  │ ▶ │  RAM   │ ▶ │  NPU   │ ▶ │  Disk  │      │  │
│  │   │ 24 GB  │   │ 64 GB  │   │ 16 GB  │   │ NVMe   │      │  │
│  │   │ Fast   │   │ Medium │   │ Medium │   │ Slow   │      │  │
│  │   └────────┘   └────────┘   └────────┘   └────────┘      │  │
│  │                                                            │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Model Profiles

```rust
pub struct LLMProfile {
    pub name: String,
    pub parameter_count: u64,        // 7B, 13B, 70B, etc.
    pub quantization: Quantization,  // Q4_K_M, Q8_0, F16, etc.
    pub context_length: u32,         // 4K, 8K, 32K, 128K
    pub layer_count: u32,
    pub layer_size_mb: f32,
    pub kv_cache_per_token_kb: f32,
}

pub struct RuntimeProfile {
    pub runtime: LLMRuntime,         // Ollama, llama.cpp, vLLM
    pub model: LLMProfile,
    pub current_context: u32,
    pub batch_size: u32,
    pub gpu_layers: u32,
    pub cpu_layers: u32,
}
```

### KV Cache Optimization

```rust
pub struct KVCacheOptimizer {
    /// Current KV cache size
    pub current_size_mb: f64,

    /// Predicted size at max context
    pub predicted_max_mb: f64,

    /// Recommended pruning threshold
    pub prune_threshold: f32,
}

impl KVCacheOptimizer {
    /// Predict memory needed for given context length
    pub fn predict_memory(&self, tokens: u32) -> MemoryRequirement;

    /// Suggest context length for available memory
    pub fn suggest_context_length(&self, available_mb: f64) -> u32;

    /// Recommend KV cache pruning
    pub fn recommend_prune(&self) -> Option<PruneStrategy>;
}
```

### Placement Engine

```rust
pub enum PlacementStrategy {
    /// Maximum performance (all on GPU)
    MaxPerformance,
    /// Fit in available VRAM
    FitVRAM { target_free_mb: u64 },
    /// Balance latency and throughput
    Balanced,
    /// Minimize power usage
    PowerEfficient,
    /// Maximize context length
    MaxContext,
}

pub struct PlacementDecision {
    pub gpu_layers: u32,
    pub cpu_layers: u32,
    pub npu_layers: u32,
    pub kv_cache_location: MemoryTier,
    pub estimated_tokens_per_sec: f32,
    pub estimated_memory_gb: f32,
}

impl PlacementEngine {
    /// Calculate optimal placement for model
    pub fn calculate(
        &self,
        model: &LLMProfile,
        strategy: PlacementStrategy,
        resources: &SystemResources,
    ) -> PlacementDecision;

    /// Rebalance during inference
    pub fn rebalance(&self, current: &RuntimeProfile) -> Option<PlacementDelta>;
}
```

### Runtime Integration

| Runtime | Detection | Integration Level |
|---------|-----------|-------------------|
| Ollama | Process + API | Deep (API control) |
| llama.cpp | Process | Medium (CLI params) |
| vLLM | Process + Port | Medium (API) |
| LM Studio | Process | Basic (monitoring) |
| Text Gen WebUI | Process + Port | Medium (API) |
| PyTorch | Process + GPU | Basic (monitoring) |

### API Design

```rust
pub struct LLMOptimizer {
    detector: RuntimeDetector,
    profiler: ModelProfiler,
    placement: PlacementEngine,
    kv_optimizer: KVCacheOptimizer,
}

impl LLMOptimizer {
    /// Detect running LLM runtimes
    pub fn detect_runtimes(&self) -> Vec<DetectedRuntime>;

    /// Get optimization for specific runtime
    pub fn optimize(&self, runtime: &DetectedRuntime) -> OptimizationPlan;

    /// Apply optimization (if runtime supports)
    pub async fn apply(&self, plan: &OptimizationPlan) -> Result<(), Error>;

    /// Monitor inference performance
    pub fn monitor(&self) -> InferenceMetrics;

    /// Predict OOM and prevent
    pub fn predict_oom(&self) -> Option<OOMPrediction>;
}

pub struct InferenceMetrics {
    pub tokens_per_second: f32,
    pub time_to_first_token_ms: u32,
    pub vram_used_mb: u64,
    pub ram_used_mb: u64,
    pub context_utilization: f32,
    pub batch_efficiency: f32,
}
```

### Ollama Deep Integration

```rust
pub struct OllamaOptimizer {
    client: OllamaClient,
    base_optimizer: LLMOptimizer,
}

impl OllamaOptimizer {
    /// Get loaded models
    pub async fn list_models(&self) -> Vec<OllamaModel>;

    /// Optimize model loading
    pub async fn optimize_load(&self, model: &str) -> LoadPlan;

    /// Pre-warm model before use
    pub async fn prewarm(&self, model: &str) -> Result<(), Error>;

    /// Unload least-used models
    pub async fn cleanup(&self) -> CleanupResult;

    /// Set optimal parameters
    pub async fn configure(&self, model: &str, config: &ModelConfig) -> Result<(), Error>;
}
```

### Predictive Features

1. **Context Length Prediction**
   - Monitor prompt patterns
   - Predict needed context before OOM
   - Suggest model swaps if needed

2. **Pre-warming**
   - Learn usage patterns
   - Pre-load models before needed
   - Keep hot models in memory

3. **Batch Optimization**
   - Detect multiple pending requests
   - Suggest batch size increase
   - Balance latency vs throughput

## Consequences

### Positive
- Optimal model placement automatically
- Prevents OOM during inference
- Maximizes tokens/second
- Learns user's model usage patterns

### Negative
- Runtime-specific integration work
- May conflict with runtime's own optimization
- Requires API access for deep features
- Complex memory tier management

### Risks
- Runtime updates may break integration
- Aggressive optimization may affect quality
- NPU support is limited

## Implementation Phases

### Phase 1: Detection & Profiling (2 weeks)
- Runtime detection
- Model profiling
- Memory monitoring

### Phase 2: Placement Engine (2 weeks)
- GPU/CPU layer calculation
- Strategy implementation
- Memory tier management

### Phase 3: Ollama Integration (2 weeks)
- API client
- Model management
- Configuration control

### Phase 4: Predictive Features (2 weeks)
- OOM prediction
- Pre-warming
- Pattern learning

## Success Metrics

| Metric | Target |
|--------|--------|
| OOM Prevention Rate | > 95% |
| Tokens/sec Improvement | > 15% |
| VRAM Utilization | > 90% |
| Time to First Token | -20% |
| User Intervention Needed | < 5% |

## References

- [llama.cpp](https://github.com/ggerganov/llama.cpp)
- [Ollama](https://ollama.ai/)
- [vLLM](https://github.com/vllm-project/vllm)
- Existing: `src/ai/ollama.rs`, `src/ai/gpu.rs`, `src/ai/bridge.rs`
