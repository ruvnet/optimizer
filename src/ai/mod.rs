//! AI Mode - Intelligent optimization for LLM inference and AI workloads
//!
//! Provides deep integration with AI runtimes (Ollama, llama.cpp, vLLM)
//! and manages GPU/CPU resources for optimal inference performance.
//!
//! ## Features (optional, enabled with `ai` feature)
//!
//! - **Workload Detection**: ML-based classification of gaming, coding, video editing
//! - **GPU/VRAM Management**: Monitor and optimize GPU memory for AI workloads
//! - **Ollama Integration**: Manage models, unload inactive, optimize VRAM
//! - **Resource Bridge**: Unified CPU/GPU/NPU resource orchestration
//! - **Game Mode**: Auto-detect games and maximize performance
//! - **Focus Mode**: Detect meetings/calls and reduce background activity
//! - **Predictive Preloading**: Pre-optimize before launching heavy apps
//! - **Thermal Prediction**: Preemptive cooling to prevent throttling

pub mod detector;
pub mod gpu;
pub mod ollama;
pub mod bridge;
pub mod workload;
pub mod modes;

pub use detector::{AIWorkloadDetector, AIRuntime, ActiveWorkload};
pub use gpu::{GpuMonitor, VramStatus, GpuInfo, GpuVendor};
pub use ollama::OllamaClient;
pub use bridge::{ResourceBridge, PlacementStrategy, PlacementPlan, DeviceType};
pub use workload::{WorkloadClassifier, WorkloadType, WorkloadProfile};
pub use modes::{GameMode, FocusMode, PerformanceMode};

use serde::{Deserialize, Serialize};

/// AI Mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIModeConfig {
    /// Enable automatic AI workload detection
    pub auto_detect: bool,
    /// VRAM reserve percentage (keep free)
    pub vram_reserve_percent: u32,
    /// RAM reserve for offloading (GB)
    pub ram_reserve_gb: u32,
    /// KV cache limit as percentage of VRAM
    pub kv_cache_limit_percent: u32,
    /// Auto-offload threshold (VRAM usage %)
    pub auto_offload_threshold: u32,
    /// Auto-compact KV cache threshold
    pub auto_compact_threshold: u32,
    /// Prefer throughput over latency
    pub prefer_throughput: bool,
    /// Enable Ollama integration
    pub ollama_integration: bool,
    /// Ollama API URL
    pub ollama_url: String,
    /// Enable Game Mode auto-detection
    pub game_mode_enabled: bool,
    /// Enable Focus Mode auto-detection
    pub focus_mode_enabled: bool,
    /// Enable predictive preloading
    pub predictive_preload: bool,
    /// Enable thermal prediction
    pub thermal_prediction: bool,
}

impl Default for AIModeConfig {
    fn default() -> Self {
        Self {
            auto_detect: true,
            vram_reserve_percent: 5,
            ram_reserve_gb: 8,
            kv_cache_limit_percent: 30,
            auto_offload_threshold: 95,
            auto_compact_threshold: 80,
            prefer_throughput: false,
            ollama_integration: true,
            ollama_url: "http://localhost:11434".into(),
            game_mode_enabled: true,
            focus_mode_enabled: true,
            predictive_preload: true,
            thermal_prediction: true,
        }
    }
}

/// AI Mode optimizer
pub struct AIMode {
    config: AIModeConfig,
    detector: AIWorkloadDetector,
    gpu_monitor: Option<GpuMonitor>,
    ollama: Option<OllamaClient>,
    workload_classifier: WorkloadClassifier,
    resource_bridge: ResourceBridge,
    game_mode: GameMode,
    focus_mode: FocusMode,
}

impl AIMode {
    pub fn new(config: AIModeConfig) -> Self {
        let detector = AIWorkloadDetector::new();
        let gpu_monitor = GpuMonitor::new().ok();
        let ollama = if config.ollama_integration {
            OllamaClient::new(&config.ollama_url).ok()
        } else {
            None
        };
        let workload_classifier = WorkloadClassifier::new();
        let resource_bridge = ResourceBridge::new(PlacementStrategy::GPUFirst);
        let game_mode = GameMode::new(config.game_mode_enabled);
        let focus_mode = FocusMode::new(config.focus_mode_enabled);

        Self {
            config,
            detector,
            gpu_monitor,
            ollama,
            workload_classifier,
            resource_bridge,
            game_mode,
            focus_mode,
        }
    }

    /// Get comprehensive AI status
    pub async fn status(&self) -> AIStatus {
        let workloads = self.detector.detect();
        let gpu = self.gpu_monitor.as_ref().map(|m| m.status());
        let ollama = if let Some(client) = &self.ollama {
            client.get_models().await.ok()
        } else {
            None
        };
        let current_workload = self.workload_classifier.classify_current();
        let game_active = self.game_mode.is_active();
        let focus_active = self.focus_mode.is_active();

        AIStatus {
            workloads,
            gpu,
            ollama_models: ollama,
            current_workload,
            game_mode_active: game_active,
            focus_mode_active: focus_active,
        }
    }

    /// Optimize AI workloads
    pub async fn optimize(&self) -> AIOptimizeResult {
        let mut result = AIOptimizeResult::default();

        // Check VRAM pressure
        if let Some(gpu) = &self.gpu_monitor {
            let status = gpu.status();
            if let Some(vram) = status.first() {
                let usage_percent = (vram.used as f64 / vram.total as f64 * 100.0) as u32;

                if usage_percent > self.config.auto_offload_threshold {
                    result.vram_freed_mb = self.optimize_vram().await;
                }
            }
        }

        // Unload inactive Ollama models
        if let Some(client) = &self.ollama {
            if let Ok(models) = client.get_models().await {
                for model in models {
                    if !model.is_active {
                        if client.unload_model(&model.name).await.is_ok() {
                            result.models_unloaded += 1;
                        }
                    }
                }
            }
        }

        result
    }

    /// Check and apply Game Mode if needed
    pub fn check_game_mode(&mut self) -> Option<GameModeAction> {
        if !self.config.game_mode_enabled {
            return None;
        }

        self.game_mode.check_and_activate()
    }

    /// Check and apply Focus Mode if needed
    pub fn check_focus_mode(&mut self) -> Option<FocusModeAction> {
        if !self.config.focus_mode_enabled {
            return None;
        }

        self.focus_mode.check_and_activate()
    }

    /// Get GPU monitor reference
    pub fn gpu_monitor(&self) -> Option<&GpuMonitor> {
        self.gpu_monitor.as_ref()
    }

    /// Get resource bridge reference
    pub fn resource_bridge(&self) -> &ResourceBridge {
        &self.resource_bridge
    }

    /// Get workload classifier reference
    pub fn workload_classifier(&self) -> &WorkloadClassifier {
        &self.workload_classifier
    }

    async fn optimize_vram(&self) -> u64 {
        // Unload inactive models first
        if let Some(client) = &self.ollama {
            if let Ok(models) = client.get_models().await {
                let mut freed = 0u64;
                for model in models {
                    if !model.is_active {
                        if let Some(vram) = model.vram_mb {
                            if client.unload_model(&model.name).await.is_ok() {
                                freed += vram;
                            }
                        }
                    }
                }
                return freed;
            }
        }
        0
    }
}

/// AI system status
#[derive(Debug, Clone)]
pub struct AIStatus {
    pub workloads: Vec<ActiveWorkload>,
    pub gpu: Option<Vec<VramStatus>>,
    pub ollama_models: Option<Vec<ollama::OllamaModel>>,
    pub current_workload: WorkloadType,
    pub game_mode_active: bool,
    pub focus_mode_active: bool,
}

/// Result of AI optimization
#[derive(Debug, Clone, Default)]
pub struct AIOptimizeResult {
    pub vram_freed_mb: u64,
    pub ram_freed_mb: u64,
    pub kv_cache_compacted_mb: u64,
    pub models_unloaded: usize,
}

/// Game Mode action taken
#[derive(Debug, Clone)]
pub struct GameModeAction {
    pub game_detected: String,
    pub optimizations_applied: Vec<String>,
}

/// Focus Mode action taken
#[derive(Debug, Clone)]
pub struct FocusModeAction {
    pub trigger: String,
    pub actions_taken: Vec<String>,
}

impl std::fmt::Display for AIOptimizeResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VRAM: {} MB, RAM: {} MB, KV: {} MB, Models unloaded: {}",
            self.vram_freed_mb, self.ram_freed_mb, self.kv_cache_compacted_mb, self.models_unloaded
        )
    }
}
