# ADR-025: Agentic Desktop Automation

## Status
**Proposed**

## Date
2026-01-28 (Updated with SOTA research)

## Context

The convergence of **tiny computer-use vision models** (2B-7B parameters), **LoRA fine-tuning** for domain adaptation, and **online reinforcement learning** for self-improvement creates an opportunity for a fundamentally new kind of desktop automation: **AI agents that can see, understand, and operate any desktop application** - not through scripted macros, but through visual understanding, adaptive reasoning, and continuous self-learning.

### State of the Art (2025-2026)

The computer-use agent field has exploded with open-source models specifically designed for GUI interaction:

| Model | Size | Type | Benchmark | Key Innovation |
|-------|------|------|-----------|----------------|
| **UI-TARS-2** (ByteDance) | 2B/7B/72B | Vision-Language-Action | 50.6 WindowsAgentArena | Multi-turn RL, data flywheel |
| **ShowUI** (CVPR 2025) | 2B | Vision-Language-Action | ScreenSpot SOTA | End-to-end GUI grounding |
| **GUI-Actor** (Microsoft) | 7B | GUI Grounding | 44.6 ScreenSpot-Pro | Attention action head, coordinate-free |
| **Qwen-GUI-3B** | 3B | GUI Grounding | Cross-resolution | Two-stage SFT, lightweight |
| **OmniParser V2** (Microsoft) | YOLO+Florence | Screen Parsing | 39.5 ScreenSpot-Pro | Pure vision UI element detection |
| **OS-Atlas** (ICLR 2025) | 4B/7B | Foundation Action | Cross-platform | Generalist grounding |
| **Aguvis** | 7B | Pure Vision Agent | Multi-platform | Unified vision-only framework |

**Key insight**: These models are small enough (2B-7B) to run locally on consumer GPUs. A Q4-quantized UI-TARS-2B fits in ~1.5GB VRAM. Combined with LoRA fine-tuning, RuVector can create **personalized desktop agents** that learn each user's specific applications and workflows.

### Self-Learning Breakthroughs

| Framework | Method | Result |
|-----------|--------|--------|
| **ZeroGUI** (2025) | VLM auto-task generation + auto-reward + online RL | +14% UI-TARS, +63% Aguvis improvement |
| **GUI-RCPO** | Test-time RL via region consistency | +5.5% on Qwen2.5-VL-3B with 0 labels |
| **UI-AGILE** | RFT on 9K examples, 2 epochs | SOTA for 3B/7B on ScreenSpot-Pro |
| **ComputerRL** | End-to-end online RL for computer use | Continuous self-improvement loop |

These frameworks prove that GUI agents can **self-improve without human annotation** - exactly what RuVector needs for watch-and-learn automation.

### RuVector Existing Infrastructure

- **RuVLLM** runtime (`AIRuntime::RuVLLM` in `src/ai/detector.rs`) - Rust LLM runtime on port 8080
- **NeuralDecisionEngine** (`src/neural/engine.rs`) - Pattern index + attention scoring + EWC learning
- **HNSW PatternIndex** (`src/neural/hnsw_patterns.rs`) - Fast similarity search for learned patterns
- **AttentionScorer** (`src/neural/attention.rs`) - Temporal attention with learned weights
- **EWCLearner** (`src/neural/ewc_learner.rs`) - Elastic Weight Consolidation to prevent catastrophic forgetting
- **SpectralAnalyzer** (`src/algorithms/spectral.rs`) - Anomaly detection for behavior changes
- **WASM plugin runtime** (ADR-021) - Wasmer 4.3 for extensible agent behaviors
- **GPU monitoring** (ADR-022) - NVML + DXGI for VRAM-aware model management
- **Ollama detection** (`src/ai/detector.rs`) - Already detects Ollama on port 11434

## Decision

### 1. Tiered Vision Model Architecture

RuVector uses a **three-tier model stack** that balances latency, accuracy, and resource usage:

```rust
/// Tiered model architecture - use the smallest model that works
pub struct VisionModelStack {
    /// Tier 0: OmniParser (YOLO + Florence) - ~50ms, no LLM needed
    /// Pure vision parsing: detect UI elements, extract icons, OCR text
    screen_parser: OmniParserV2,

    /// Tier 1: Tiny CU model (2B) - ~200ms on GPU, ~2s on CPU
    /// GUI grounding: "click the Save button" -> (x, y) coordinates
    grounding_model: TinyGroundingModel,

    /// Tier 2: Full VLA model (7B) - ~500ms-2s on GPU
    /// Complex reasoning: multi-step planning, error recovery, replanning
    reasoning_model: Option<ReasoningModel>,

    /// RuVLLM LoRA adapter registry - personalized fine-tunes per app
    lora_registry: LoraAdapterRegistry,
}

pub struct OmniParserV2 {
    /// YOLO-based interactable element detector (~5MB model)
    icon_detector: YoloDetector,
    /// Florence-based icon captioner (~200MB model)
    icon_captioner: FlorenceCaptioner,
    /// Confidence threshold for detected elements
    detection_threshold: f32,  // 0.3 default
}

pub struct TinyGroundingModel {
    pub model_id: String,              // "UI-TARS-2B-SFT" or "ShowUI-2B" or "Qwen-GUI-3B"
    pub backend: InferenceBackend,
    pub quantization: Quantization,
    pub vram_usage_mb: u64,            // ~1500MB for Q4 2B model
    pub active_lora: Option<String>,   // Current LoRA adapter name
}

pub struct ReasoningModel {
    pub model_id: String,              // "UI-TARS-7B-DPO" or "GUI-Actor-7B"
    pub backend: InferenceBackend,
    pub quantization: Quantization,
    pub vram_usage_mb: u64,            // ~4500MB for Q4 7B model
    pub active_lora: Option<String>,
}

pub enum InferenceBackend {
    Ollama { endpoint: String },       // http://localhost:11434
    LlamaCpp { model_path: PathBuf },  // Direct GGUF loading
    RuVLLM { endpoint: String },       // RuVector's Rust LLM runtime
    ONNX { model_path: PathBuf },      // ONNX Runtime (DirectML on Windows)
}

pub enum Quantization {
    F16,       // Full precision - needs ~4GB for 2B
    Q8_0,      // 8-bit - ~2GB for 2B
    Q4_K_M,    // 4-bit (recommended) - ~1.5GB for 2B
    Q4_K_S,    // 4-bit small - ~1.2GB for 2B
    Q3_K_M,    // 3-bit - ~1GB for 2B (accuracy tradeoff)
}

impl VisionModelStack {
    /// Intelligent routing: use cheapest model tier that can handle the task
    pub async fn perceive(&self, screenshot: &Screenshot, task: &PerceptionTask) -> PerceptionResult {
        match task {
            // Tier 0: Pure vision parsing (no LLM, fastest)
            PerceptionTask::ListElements => {
                let elements = self.screen_parser.detect_elements(screenshot);
                PerceptionResult::Elements(elements)
            }

            // Tier 1: Element grounding (tiny model, fast)
            PerceptionTask::FindElement(description) => {
                let parsed = self.screen_parser.detect_elements(screenshot);
                let grounded = self.grounding_model.ground(
                    screenshot, description, &parsed
                ).await;
                PerceptionResult::Location(grounded)
            }

            // Tier 2: Complex reasoning (full model, slower)
            PerceptionTask::PlanActions(goal) => {
                if let Some(reasoner) = &self.reasoning_model {
                    let plan = reasoner.plan(screenshot, goal).await;
                    PerceptionResult::ActionPlan(plan)
                } else {
                    // Fallback: use grounding model with chain-of-thought
                    let plan = self.grounding_model.plan_with_cot(
                        screenshot, goal
                    ).await;
                    PerceptionResult::ActionPlan(plan)
                }
            }
        }
    }
}
```

### 2. RuVLLM LoRA Fine-Tuning Pipeline

RuVector watches user interactions and fine-tunes per-application LoRA adapters:

```rust
pub struct LoraAdapterRegistry {
    adapters: HashMap<String, LoraAdapter>,  // app_name -> adapter
    training_queue: VecDeque<TrainingBatch>,
    config: LoraConfig,
}

pub struct LoraAdapter {
    pub name: String,                  // "photoshop-v3", "excel-v2"
    pub app_name: String,              // "photoshop.exe"
    pub base_model: String,            // "UI-TARS-2B-SFT"
    pub adapter_path: PathBuf,         // ~/.ruvector/lora/photoshop-v3/
    pub rank: u8,                      // LoRA rank (8-16 recommended)
    pub alpha: u16,                    // LoRA alpha (2x rank)
    pub target_modules: Vec<String>,   // ["q_proj", "v_proj", "k_proj", "o_proj"]
    pub training_samples: u32,         // Number of trajectories trained on
    pub accuracy: f64,                 // Grounding accuracy on validation set
    pub last_trained: DateTime<Local>,
    pub version: u32,
}

pub struct LoraConfig {
    pub rank: u8,                      // 8 (fast) or 16 (better quality)
    pub alpha: u16,                    // 2 * rank
    pub dropout: f32,                  // 0.05
    pub target_modules: Vec<String>,   // All attention projections
    pub learning_rate: f64,            // 2e-4
    pub epochs: u32,                   // 2-3 (UI-AGILE showed 2 epochs on 9K samples = SOTA)
    pub batch_size: u32,               // 4 (limited by VRAM)
    pub gradient_accumulation: u32,    // 16 (effective batch = 64)
    pub max_vram_mb: u64,              // Training VRAM budget
    pub quantize_base: bool,           // QLoRA: 4-bit base + LoRA (fits in 12GB)
}

impl LoraAdapterRegistry {
    /// Record a user interaction trajectory for future training
    pub fn record_trajectory(&mut self, trajectory: &InteractionTrajectory) {
        let app = &trajectory.app_name;

        // Convert trajectory to training format:
        // (screenshot, instruction, action) tuples
        let samples: Vec<TrainingSample> = trajectory.steps.iter().map(|step| {
            TrainingSample {
                screenshot: step.screenshot_before.clone(),
                instruction: step.natural_language_description.clone(),
                action: step.action.to_model_output(),  // Normalized coordinates
                app_context: app.clone(),
                success: step.verified_success,
                timestamp: step.timestamp,
            }
        }).collect();

        // Add to training queue
        self.training_queue.push_back(TrainingBatch {
            app_name: app.clone(),
            samples,
            collected_at: Local::now(),
        });

        // Trigger training when we have enough samples
        if self.pending_samples_for(app) >= 100 {
            self.schedule_training(app);
        }
    }

    /// Fine-tune a LoRA adapter for a specific application
    pub async fn train_adapter(&mut self, app_name: &str) -> Result<LoraAdapter, String> {
        let batches: Vec<TrainingBatch> = self.training_queue.iter()
            .filter(|b| b.app_name == app_name)
            .cloned().collect();

        let total_samples: usize = batches.iter().map(|b| b.samples.len()).sum();
        tracing::info!("Training LoRA adapter for {} with {} samples", app_name, total_samples);

        // Determine base model from current stack
        let base_model = self.get_base_model();

        // Configure LoRA training
        let adapter = LoraAdapter {
            name: format!("{}-v{}", app_name, self.next_version(app_name)),
            app_name: app_name.to_string(),
            base_model: base_model.clone(),
            adapter_path: self.adapter_dir(app_name),
            rank: self.config.rank,
            alpha: self.config.alpha,
            target_modules: self.config.target_modules.clone(),
            training_samples: total_samples as u32,
            accuracy: 0.0,  // Set after evaluation
            last_trained: Local::now(),
            version: self.next_version(app_name),
        };

        // Launch training via RuVLLM or external trainer
        // QLoRA: 4-bit quantized base + LoRA adapters
        // Fits in 12GB VRAM (RTX 4070 level) for 2B model
        // Fits in 24GB VRAM (RTX 4090) for 7B model
        self.launch_training(&adapter, &batches).await?;

        // Evaluate on held-out validation set (10% of samples)
        let accuracy = self.evaluate_adapter(&adapter).await?;

        let mut adapter = adapter;
        adapter.accuracy = accuracy;

        // EWC consolidation: prevent forgetting previous knowledge
        // Uses existing EWCLearner from neural engine
        self.ewc_consolidate(&adapter).await?;

        self.adapters.insert(app_name.to_string(), adapter.clone());
        Ok(adapter)
    }

    /// Hot-swap LoRA adapter when user switches applications
    pub async fn activate_adapter(&self, app_name: &str, model: &mut TinyGroundingModel) {
        if let Some(adapter) = self.adapters.get(app_name) {
            model.active_lora = Some(adapter.name.clone());
            // Merge LoRA weights into model (zero inference latency overhead)
            // or load as separate adapter (allows quick swapping)
            model.load_lora(&adapter.adapter_path).await;
            tracing::info!("Activated LoRA adapter '{}' (accuracy: {:.1}%)",
                adapter.name, adapter.accuracy * 100.0);
        }
    }
}
```

### 3. Watch-and-Learn Trajectory Collection

```rust
/// Passively observe user interactions to build training data
/// Inspired by OmniTool's trajectory logging and ZeroGUI's data flywheel
pub struct TrajectoryCollector {
    /// Windows event hooks for monitoring
    win_event_hook: WinEventHook,
    /// Screen capture at key moments
    capture: ScreenCapture,
    /// OmniParser for element detection in captured frames
    parser: OmniParserV2,
    /// Current recording session
    session: Option<RecordingSession>,
    /// HNSW index for deduplicating similar trajectories
    trajectory_index: PatternIndex,
}

pub struct RecordingSession {
    pub app_name: String,
    pub window_title: String,
    pub hwnd: HWND,
    pub steps: Vec<ObservedStep>,
    pub start_time: DateTime<Local>,
}

pub struct ObservedStep {
    pub timestamp: DateTime<Local>,
    pub event_type: UserEvent,
    pub screenshot_before: Screenshot,
    pub screenshot_after: Screenshot,
    pub ui_elements: Vec<DetectedElement>,   // OmniParser output
    pub target_element: Option<DetectedElement>, // Which element was interacted with
    pub natural_language_description: String,     // Auto-generated description
}

pub enum UserEvent {
    MouseClick { x: i32, y: i32, button: MouseButton },
    KeyboardInput { text: String },
    Hotkey { keys: Vec<Key> },
    Scroll { delta: i32 },
    DragDrop { from: (i32, i32), to: (i32, i32) },
    WindowSwitch { from: String, to: String },
    MenuSelect { path: Vec<String> },
}

impl TrajectoryCollector {
    /// Start observing user interactions (passive, low-overhead)
    pub fn start_recording(&mut self, hwnd: HWND) {
        // Install Windows event hooks:
        // - EVENT_OBJECT_FOCUS: Track focused elements
        // - EVENT_SYSTEM_FOREGROUND: Track window switches
        // - Low-level keyboard/mouse hooks for input events
        self.win_event_hook.install(vec![
            WinEvent::ObjectFocus,
            WinEvent::SystemForeground,
        ]);

        self.session = Some(RecordingSession {
            app_name: self.get_process_name(hwnd),
            window_title: self.get_window_title(hwnd),
            hwnd,
            steps: vec![],
            start_time: Local::now(),
        });
    }

    /// Called on each user action (from Win32 hooks)
    pub async fn on_user_action(&mut self, event: UserEvent) {
        let session = match &mut self.session {
            Some(s) => s,
            None => return,
        };

        // Capture screenshot BEFORE the action takes effect
        let screenshot_before = self.capture.capture_window(session.hwnd);

        // Detect UI elements using OmniParser (Tier 0 - fast, no LLM)
        let elements = self.parser.detect_elements(&screenshot_before);

        // Match the user's click/action to a detected element
        let target = match &event {
            UserEvent::MouseClick { x, y, .. } => {
                elements.iter().find(|e| e.bounding_rect.contains(*x, *y)).cloned()
            }
            _ => None,
        };

        // Wait for UI to update, then capture AFTER screenshot
        tokio::time::sleep(Duration::from_millis(500)).await;
        let screenshot_after = self.capture.capture_window(session.hwnd);

        // Auto-generate natural language description
        let description = self.describe_action(&event, &target, &elements);

        session.steps.push(ObservedStep {
            timestamp: Local::now(),
            event_type: event,
            screenshot_before,
            screenshot_after,
            ui_elements: elements,
            target_element: target,
            natural_language_description: description,
        });
    }

    /// Auto-generate training descriptions from observed actions
    fn describe_action(
        &self,
        event: &UserEvent,
        target: &Option<DetectedElement>,
        _elements: &[DetectedElement],
    ) -> String {
        match (event, target) {
            (UserEvent::MouseClick { .. }, Some(elem)) => {
                format!("Click on '{}'", elem.caption.as_deref()
                    .or(elem.name.as_deref())
                    .unwrap_or("unknown element"))
            }
            (UserEvent::KeyboardInput { text }, _) => {
                format!("Type '{}'", text)
            }
            (UserEvent::Hotkey { keys }, _) => {
                let key_str: Vec<&str> = keys.iter().map(|k| k.name()).collect();
                format!("Press {}", key_str.join("+"))
            }
            (UserEvent::MenuSelect { path }, _) => {
                format!("Select menu: {}", path.join(" > "))
            }
            _ => "Unknown action".to_string(),
        }
    }

    /// Finalize recording and submit for training
    pub fn finish_recording(&mut self) -> Option<InteractionTrajectory> {
        let session = self.session.take()?;

        if session.steps.len() < 3 {
            return None;  // Too short to be useful
        }

        // Check if similar trajectory already exists (deduplication)
        let trajectory_embedding = self.embed_trajectory(&session);
        let similar = self.trajectory_index.search(&trajectory_embedding, 1);
        if let Some((_, similarity)) = similar.first() {
            if *similarity > 0.95 {
                return None;  // Already have this trajectory
            }
        }

        // Store trajectory embedding for future dedup
        self.trajectory_index.add(&trajectory_embedding).ok();

        Some(InteractionTrajectory {
            app_name: session.app_name,
            steps: session.steps,
            total_duration: Local::now() - session.start_time,
            success: true,  // User completed the workflow
        })
    }
}
```

### 4. Online Reinforcement Learning (ZeroGUI-inspired)

```rust
/// Self-improving agent using online RL - no human annotation needed
/// Based on ZeroGUI framework: auto-task generation + auto-reward + online RL
pub struct OnlineRLTrainer {
    /// Base agent to improve
    agent: DesktopAgent,
    /// Task generator (VLM-based, generates tasks from current screen state)
    task_generator: TaskGenerator,
    /// Reward estimator (VLM-based, judges success from trajectory screenshots)
    reward_estimator: RewardEstimator,
    /// EWC++ from existing neural engine - prevents catastrophic forgetting
    ewc: EWCLearner,
    /// Training config
    config: RLConfig,
}

pub struct RLConfig {
    pub method: RLMethod,
    pub learning_rate: f64,            // 1e-5 for RL fine-tuning
    pub kl_coefficient: f64,           // 0.01 - prevent policy collapse
    pub reward_voting_rounds: u32,     // 3 - multi-vote reward estimation
    pub max_episodes_per_session: u32, // 10 - training budget
    pub rollout_max_steps: u32,        // 15 - max steps per episode
    pub min_reward_threshold: f64,     // 0.5 - minimum reward to train on
}

pub enum RLMethod {
    /// Reinforcement Fine-Tuning (simple, effective for GUI)
    /// UI-AGILE showed SOTA with just SFT on 9K RL-filtered examples
    RFT,
    /// Group Relative Policy Optimization (ZeroGUI's approach)
    /// +14% improvement on UI-TARS with auto-generated tasks
    GRPO,
    /// Direct Preference Optimization
    /// UI-TARS-7B-DPO is the recommended production model
    DPO,
}

pub struct TaskGenerator {
    /// Uses the reasoning model to generate plausible GUI tasks
    /// from the current screen state - no human needed
    model: ReasoningModel,
}

impl TaskGenerator {
    /// Generate diverse training tasks from current screen state
    /// Based on ZeroGUI's example-guided multi-candidate generation
    pub async fn generate_tasks(&self, screenshot: &Screenshot, app_name: &str) -> Vec<GeneratedTask> {
        let prompt = format!(
            "You are looking at a screenshot of {}. Generate 5 diverse, \
             realistic tasks a user might want to perform in this application. \
             Each task should be specific and completable from the current screen state. \
             Include both simple (1-3 steps) and complex (5-10 steps) tasks. \
             Format: task_description | expected_outcome",
            app_name
        );

        let response = self.model.generate(screenshot, &prompt).await;
        self.parse_tasks(&response)
    }
}

pub struct RewardEstimator {
    model: ReasoningModel,
}

impl RewardEstimator {
    /// Estimate task success from trajectory screenshots
    /// Uses multi-vote mechanism to avoid hallucinated success
    pub async fn estimate_reward(
        &self,
        task: &str,
        trajectory_screenshots: &[Screenshot],
    ) -> f64 {
        let mut votes = vec![];

        for _ in 0..3 {  // 3 voting rounds
            let prompt = format!(
                "Task: {}\n\n\
                 Given the sequence of screenshots showing an agent's attempt \
                 to complete this task, did the agent succeed?\n\
                 Consider: Did the expected outcome occur? Are there any error dialogs?\n\
                 Answer ONLY 'SUCCESS' or 'FAILURE' with a brief reason.",
                task
            );

            let verdict = self.model.generate_with_images(
                trajectory_screenshots, &prompt
            ).await;

            votes.push(verdict.contains("SUCCESS"));
        }

        // Majority vote: 2/3 or 3/3 = success
        let success_count = votes.iter().filter(|&&v| v).count();
        if success_count >= 2 { 1.0 } else { 0.0 }
    }
}

impl OnlineRLTrainer {
    /// Run one self-improvement session
    /// Can run in background while user works on other things
    pub async fn train_session(&mut self, app_name: &str) -> TrainingResult {
        let mut successful_episodes = 0;
        let mut total_episodes = 0;
        let mut training_samples = vec![];

        for _ in 0..self.config.max_episodes_per_session {
            // 1. Capture current screen state
            let screenshot = self.agent.perception.capture_screenshot();

            // 2. Auto-generate a task
            let tasks = self.task_generator.generate_tasks(&screenshot, app_name).await;
            let task = &tasks[total_episodes % tasks.len()];

            // 3. Agent attempts the task (rollout)
            let trajectory = self.agent.attempt_task(
                &task.description,
                self.config.rollout_max_steps,
            ).await;

            // 4. Auto-estimate reward (no human needed)
            let reward = self.reward_estimator.estimate_reward(
                &task.description,
                &trajectory.screenshots,
            ).await;

            // 5. Collect training data from successful episodes
            if reward >= self.config.min_reward_threshold {
                training_samples.extend(trajectory.to_training_samples(reward));
                successful_episodes += 1;
            }

            total_episodes += 1;

            // Safety: undo any changes made during training
            if let Some(undo_fn) = &trajectory.undo_actions {
                undo_fn().await;
            }
        }

        // 6. Fine-tune on collected samples (RFT/GRPO/DPO)
        if !training_samples.is_empty() {
            self.fine_tune(&training_samples).await;

            // 7. EWC consolidation - preserve important weights
            self.ewc.consolidate().await;
        }

        TrainingResult {
            total_episodes,
            successful_episodes,
            training_samples: training_samples.len(),
            accuracy_improvement: self.measure_improvement().await,
        }
    }
}
```

### 5. Perception Pipeline (OmniParser + Grounding Model)

```rust
/// OmniParser V2 integration - pure vision screen parsing
/// YOLO detects interactable elements, Florence captions them
pub struct OmniParserV2 {
    /// Fine-tuned YOLO model for UI element detection (~5MB)
    detector: YoloDetector,
    /// Florence-based icon/element captioner (~200MB)
    captioner: FlorenceCaptioner,
    /// Optional: interactability classifier
    interactable_classifier: Option<InteractableClassifier>,
}

pub struct DetectedElement {
    pub bounding_rect: Rect,           // Pixel coordinates on screen
    pub normalized_rect: NormalizedRect, // 0-1000 range (model format)
    pub caption: Option<String>,        // "Save button", "Search text field"
    pub element_type: ElementType,      // Button, TextField, Icon, etc.
    pub is_interactable: bool,
    pub confidence: f32,
    pub ocr_text: Option<String>,       // Text content via OCR
}

pub enum ElementType {
    Button,
    TextField,
    Icon,
    Checkbox,
    Dropdown,
    Menu,
    Tab,
    Link,
    Image,
    Text,
    ScrollBar,
    Unknown,
}

impl OmniParserV2 {
    /// Fast screen parsing - no LLM needed
    /// Returns structured element list from raw screenshot
    pub fn detect_elements(&self, screenshot: &Screenshot) -> Vec<DetectedElement> {
        // Step 1: YOLO detection of interactable regions
        let detections = self.detector.detect(screenshot, self.detection_threshold);

        // Step 2: Caption each detected element
        let mut elements = vec![];
        for det in detections {
            let cropped = screenshot.crop(&det.bounding_rect);
            let caption = self.captioner.caption(&cropped);

            let is_interactable = self.interactable_classifier
                .as_ref()
                .map(|c| c.predict(&cropped))
                .unwrap_or(true);

            elements.push(DetectedElement {
                bounding_rect: det.bounding_rect,
                normalized_rect: det.bounding_rect.normalize(screenshot.width, screenshot.height),
                caption: Some(caption),
                element_type: self.classify_element(&det, &caption),
                is_interactable,
                confidence: det.confidence,
                ocr_text: self.extract_text(&cropped),
            });
        }

        elements
    }
}

/// GUI Grounding via tiny CU model (ShowUI-2B, UI-TARS-2B, Qwen-GUI-3B)
/// Given instruction + screenshot -> click coordinates
pub struct TinyGroundingModel {
    pub model_id: String,
    pub backend: InferenceBackend,
    pub quantization: Quantization,
    pub vram_usage_mb: u64,
    pub active_lora: Option<String>,
}

impl TinyGroundingModel {
    /// Ground a natural language instruction to screen coordinates
    /// "Click the Save button" -> (x: 450, y: 320)
    pub async fn ground(
        &self,
        screenshot: &Screenshot,
        instruction: &str,
        parsed_elements: &[DetectedElement],
    ) -> GroundingResult {
        // Format: combine OmniParser element list with visual screenshot
        let element_text = parsed_elements.iter()
            .enumerate()
            .map(|(i, e)| format!("[{}] {} at ({},{})",
                i, e.caption.as_deref().unwrap_or("?"),
                e.bounding_rect.center_x(), e.bounding_rect.center_y()))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Instruction: {}\n\nDetected elements:\n{}\n\n\
             Output the action to perform as: click(x, y) or type(text) or scroll(direction)",
            instruction, element_text
        );

        let response = match &self.backend {
            InferenceBackend::Ollama { endpoint } => {
                self.query_ollama(endpoint, screenshot, &prompt).await
            }
            InferenceBackend::LlamaCpp { model_path } => {
                self.query_llamacpp(model_path, screenshot, &prompt).await
            }
            InferenceBackend::RuVLLM { endpoint } => {
                self.query_ruvllm(endpoint, screenshot, &prompt).await
            }
            InferenceBackend::ONNX { model_path } => {
                self.query_onnx(model_path, screenshot, &prompt).await
            }
        };

        self.parse_grounding_output(&response)
    }
}
```

### 6. VRAM-Aware Model Management

```rust
/// Integrates with GPU Memory Optimizer (ADR-022) to manage model placement
pub struct ModelMemoryManager {
    gpu_monitor: GpuMemoryMonitor,       // From ADR-022
    loaded_models: Vec<LoadedModel>,
    vram_budget_mb: u64,
}

pub struct LoadedModel {
    pub name: String,
    pub tier: ModelTier,
    pub vram_mb: u64,
    pub ram_mb: u64,
    pub last_used: Instant,
    pub can_offload: bool,
}

pub enum ModelTier {
    ScreenParser,    // OmniParser - always loaded (~200MB)
    Grounding,       // 2B model - loaded when agent is active (~1.5GB Q4)
    Reasoning,       // 7B model - loaded on demand (~4.5GB Q4)
    LoraAdapter,     // Per-app adapter - hot-swapped (~50MB each)
}

impl ModelMemoryManager {
    /// Ensure models fit within VRAM budget, offloading as needed
    pub fn manage_vram(&mut self) {
        let total_vram_needed: u64 = self.loaded_models.iter()
            .map(|m| m.vram_mb).sum();

        if total_vram_needed > self.vram_budget_mb {
            // Offload least-recently-used models to RAM
            self.loaded_models.sort_by_key(|m| m.last_used);

            for model in &mut self.loaded_models {
                if total_vram_needed <= self.vram_budget_mb { break; }
                if model.can_offload {
                    model.vram_mb = 0;  // Move to RAM
                    // total_vram_needed recalculated
                }
            }
        }
    }

    /// VRAM budget varies by model tier and available hardware
    pub fn compute_vram_budget(&self) -> u64 {
        let free = self.gpu_monitor.devices[0].free_vram_mb;
        let total = self.gpu_monitor.devices[0].total_vram_mb;

        match total {
            0..=4096 => {
                // 4GB GPU: OmniParser only, grounding on CPU
                200
            }
            4097..=8192 => {
                // 8GB GPU: OmniParser + Q4 2B grounding
                free.min(2000)
            }
            8193..=16384 => {
                // 12-16GB GPU: Full stack with Q4 7B reasoning
                free.min(6000)
            }
            _ => {
                // 24GB+ GPU: Full stack, multiple LoRA adapters cached
                free.min(12000)
            }
        }
    }
}
```

### 7. Integration with Existing Neural Engine

```rust
/// Extends NeuralDecisionEngine with GUI agent capabilities
/// Reuses: PatternIndex (HNSW), AttentionScorer, EWCLearner
impl NeuralDecisionEngine {
    /// Store a learned GUI interaction pattern
    pub fn store_gui_pattern(&mut self, pattern: &GuiInteractionPattern) {
        let vector = pattern.to_embedding();
        self.pattern_index.add(&vector).ok();
        self.history.push(LabeledPattern {
            features: vector,
            success: pattern.success,
            timestamp: Local::now(),
        });
    }

    /// Find similar past interactions for a new GUI task
    pub fn find_similar_gui_patterns(
        &self,
        task_embedding: &[f32],
        top_k: usize,
    ) -> Vec<(usize, f32)> {
        self.pattern_index.search(task_embedding, top_k)
            .unwrap_or_default()
    }

    /// Use attention scorer to weight GUI action importance
    pub fn score_gui_action(&self, action: &GuiInteractionPattern) -> f32 {
        // Reuse temporal attention (time-of-day patterns)
        // + add application context attention
        let features = action.to_memory_pattern();
        let dummy_status = MemoryStatus::default(); // GUI doesn't need memory status
        self.attention.score(&features, &dummy_status)
    }

    /// EWC consolidation after LoRA training
    /// Prevents catastrophic forgetting of general GUI knowledge
    /// when fine-tuning on specific application patterns
    pub fn consolidate_gui_knowledge(&mut self) {
        self.ewc.consolidate_patterns(&self.history);
    }
}

pub struct GuiInteractionPattern {
    pub app_name: String,
    pub task_description: String,
    pub element_types_used: Vec<ElementType>,
    pub action_sequence_length: u32,
    pub success: bool,
    pub duration_ms: u64,
    pub hour: u8,
    pub day_of_week: u8,
}
```

### 8. Configuration

```toml
[agents]
enabled = true
watch_and_learn = true                # Passively observe user interactions

[agents.models]
# Tier 0: Screen Parser (always loaded)
screen_parser = "omniparser-v2"       # YOLO + Florence
screen_parser_threshold = 0.3

# Tier 1: Grounding Model (loaded when agent active)
grounding_model = "UI-TARS-2B-SFT"   # Options: ShowUI-2B, Qwen-GUI-3B, UI-TARS-2B-SFT
grounding_backend = "ollama"          # ollama | llamacpp | ruvllm | onnx
grounding_quantization = "Q4_K_M"    # F16 | Q8_0 | Q4_K_M | Q4_K_S | Q3_K_M

# Tier 2: Reasoning Model (loaded on demand)
reasoning_model = "UI-TARS-7B-DPO"   # Options: GUI-Actor-7B, UI-TARS-1.5-7B
reasoning_backend = "ollama"
reasoning_quantization = "Q4_K_M"

# VRAM management
max_agent_vram_mb = 4000              # Max VRAM for agent models
offload_when_idle_secs = 300          # Offload to RAM after 5min idle

[agents.lora]
enabled = true
rank = 8                              # LoRA rank (8=fast, 16=quality)
alpha = 16                            # LoRA alpha (2x rank)
target_modules = ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"]
learning_rate = 2e-4
epochs = 2                            # UI-AGILE: 2 epochs on 9K samples = SOTA
quantize_base = true                  # QLoRA: 4-bit base model during training
min_samples_to_train = 100            # Minimum trajectories before first fine-tune
retrain_interval_days = 7             # Retrain adapters weekly with new data
adapter_dir = "~/.ruvector/lora"

[agents.self_learning]
method = "RFT"                        # RFT | GRPO | DPO
auto_task_generation = true           # ZeroGUI-style auto task generation
auto_reward_estimation = true         # VLM-based reward estimation
reward_voting_rounds = 3              # Multi-vote for reliable reward
max_episodes_per_session = 10
rollout_max_steps = 15
train_during_idle = true              # Train when system is idle
ewc_consolidation = true              # Prevent catastrophic forgetting

[agents.perception]
capture_method = "desktop_duplication"
use_omniparser = true                 # Tier 0: always-on element detection
use_uiautomation = true               # Structured element access (when available)
use_ocr = true                        # Text extraction fallback

[agents.safety]
requires_user_present = true
max_actions_per_minute = 30
confirm_before_send = true
confirm_financial = true
confirm_delete = true
audit_log = true
allowed_apps = ["*"]
rl_training_sandbox = true            # Sandbox RL training episodes

[agents.recording]
enabled = true
capture_on_action = true              # Screenshot on each user action
max_trajectory_length = 50
dedup_similarity_threshold = 0.95     # Skip near-duplicate trajectories
storage_path = "~/.ruvector/trajectories"
max_storage_gb = 10                   # Limit trajectory storage
```

### 9. Hardware Requirements Matrix

```
┌─────────────────────────────────────────────────────────────────┐
│  Hardware Tier → Available Features                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  CPU Only (no GPU):                                             │
│  ├── OmniParser element detection (ONNX CPU)                   │
│  ├── UIAutomation structured access                             │
│  ├── OCR text extraction                                        │
│  ├── Watch-and-learn trajectory recording                       │
│  └── Workflow replay (learned sequences, no reasoning)          │
│                                                                  │
│  4GB VRAM (GTX 1650, Intel Arc A380):                          │
│  ├── All CPU features, plus:                                    │
│  ├── Q3 2B grounding model (~1GB)                              │
│  └── Basic grounding: "click the Save button" → coordinates    │
│                                                                  │
│  8GB VRAM (RTX 3060/4060, RX 7600):                            │
│  ├── All 4GB features, plus:                                    │
│  ├── Q4 2B grounding model (~1.5GB) with LoRA adapters         │
│  ├── Full grounding + simple planning                           │
│  └── LoRA training (QLoRA, 2B model)                           │
│                                                                  │
│  12GB VRAM (RTX 4070, RTX 3080):                               │
│  ├── All 8GB features, plus:                                    │
│  ├── Q4 7B reasoning model (~4.5GB)                            │
│  ├── Full planning, replanning, error recovery                  │
│  ├── LoRA training (QLoRA, 7B model)                           │
│  └── Online RL self-improvement                                 │
│                                                                  │
│  24GB+ VRAM (RTX 4090, RTX 3090):                              │
│  ├── All 12GB features, plus:                                   │
│  ├── Multiple LoRA adapters cached in VRAM                     │
│  ├── F16/Q8 models for maximum accuracy                        │
│  ├── Parallel perception + reasoning                            │
│  └── Full ZeroGUI online RL training loop                      │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 10. AIDefence WASM Security Gateway

Autonomous desktop agents executing real system actions create a critical attack surface:
**prompt injection** (adversarial text in observed screen content or user commands),
**PII leakage** (screenshots capturing passwords, financial data, personal info),
**jailbreak escalation** (crafted instructions bypassing safety guardrails), and
**data poisoning** (malicious trajectories corrupting LoRA fine-tuning).

RuVector integrates an **AIDefence WASM module** compiled from the
[claude-flow security layer](https://github.com/ruvnet/claude-flow) to run
as a sub-10ms inline security gateway. Because RuVector already ships
Wasmer 4.3 (ADR-021), the WASM module executes inside the existing sandbox
with zero additional runtime dependencies.

```rust
/// AIDefence WASM security gateway - sits between user/screen input and agent execution
/// Compiled from npm @claude-flow/security to .wasm, loaded via Wasmer 4.3
pub struct AIDefenceGateway {
    /// Wasmer instance of the AIDefence WASM module
    wasm_instance: wasmer::Instance,
    /// Threat pattern store (HNSW-indexed for fast lookup)
    threat_patterns: PatternIndex,
    /// PII detection patterns (regex + ML hybrid)
    pii_patterns: PiiDetector,
    /// Learned threat signatures from past detections
    adaptive_signatures: Vec<ThreatSignature>,
    /// Configuration
    config: AIDefenceConfig,
}

pub struct AIDefenceConfig {
    pub enabled: bool,
    pub scan_user_commands: bool,        // Scan natural language instructions
    pub scan_screen_content: bool,       // Scan OCR text from screenshots
    pub scan_agent_actions: bool,        // Validate actions before execution
    pub scan_training_data: bool,        // Validate trajectories before LoRA training
    pub pii_redact_screenshots: bool,    // Redact PII from stored screenshots
    pub block_on_threat: bool,           // Hard block vs. warn
    pub threat_threshold: f64,           // 0.0-1.0, default 0.7
    pub max_scan_latency_ms: u64,       // Budget: 10ms max per scan
}

pub struct ThreatScanResult {
    pub is_safe: bool,
    pub threat_level: ThreatLevel,
    pub threats_detected: Vec<DetectedThreat>,
    pub pii_found: Vec<PiiMatch>,
    pub scan_latency_us: u64,           // Microseconds (target: <10ms)
}

pub enum ThreatLevel {
    Safe,           // No threats detected
    Low,            // Suspicious but likely benign
    Medium,         // Possible injection, warn user
    High,           // Likely injection, block action
    Critical,       // Active attack pattern, block + alert
}

pub struct DetectedThreat {
    pub threat_type: ThreatType,
    pub confidence: f64,
    pub source: ThreatSource,
    pub matched_pattern: Option<String>,
    pub recommendation: String,
}

pub enum ThreatType {
    PromptInjection,     // "Ignore previous instructions and..."
    IndirectInjection,   // Adversarial text in observed screen content
    Jailbreak,           // Attempts to bypass safety guardrails
    PiiExposure,         // Passwords, SSNs, API keys in screenshots
    DataPoisoning,       // Malicious training data in trajectories
    PrivilegeEscalation, // Agent trying to exceed its permissions
    ExfiltrationAttempt, // Agent trying to send data externally
}

pub enum ThreatSource {
    UserCommand,         // Direct user instruction
    ScreenContent,       // OCR text from observed UI
    AgentAction,         // Proposed agent action
    TrainingData,        // Trajectory being used for fine-tuning
    LoraOutput,          // Model output after LoRA inference
}

pub struct PiiMatch {
    pub pii_type: PiiType,
    pub location: String,              // "screenshot:342x186" or "command:offset:42"
    pub redacted: Option<String>,      // Redacted version if applicable
}

pub enum PiiType {
    Email,
    PhoneNumber,
    SocialSecurityNumber,
    CreditCard,
    ApiKey,
    Password,
    Address,
    IpAddress,
}

impl AIDefenceGateway {
    /// Load AIDefence WASM module via Wasmer
    pub fn new(config: AIDefenceConfig) -> Result<Self, String> {
        let wasm_bytes = include_bytes!("../../assets/aidefence.wasm");
        let store = wasmer::Store::default();
        let module = wasmer::Module::new(&store, wasm_bytes)?;

        // WASM module exports: scan(), analyze(), detect_pii(), learn()
        let instance = wasmer::Instance::new(&module, &wasmer::imports! {})?;

        Ok(Self {
            wasm_instance: instance,
            threat_patterns: PatternIndex::new(128, 16, 200), // HNSW index
            pii_patterns: PiiDetector::default(),
            adaptive_signatures: vec![],
            config,
        })
    }

    /// Scan user command before agent processes it
    /// Latency budget: <5ms
    pub fn scan_command(&self, command: &str) -> ThreatScanResult {
        let start = Instant::now();

        // 1. WASM-accelerated pattern matching (prompt injection signatures)
        let wasm_result = self.wasm_scan(command);

        // 2. HNSW similarity search against known threat patterns
        let embedding = self.embed_text(command);
        let similar_threats = self.threat_patterns.search(&embedding, 5);

        // 3. PII detection (regex + heuristic)
        let pii_matches = self.pii_patterns.detect(command);

        // 4. Adaptive signature matching (learned from past detections)
        let adaptive_hits = self.check_adaptive_signatures(command);

        self.aggregate_results(
            ThreatSource::UserCommand,
            wasm_result, similar_threats, pii_matches, adaptive_hits,
            start.elapsed(),
        )
    }

    /// Scan screen content (OCR text) for indirect injection attacks
    /// An attacker could place adversarial text in a document/webpage
    /// that the agent reads and follows as instructions
    pub fn scan_screen_content(&self, ocr_text: &str) -> ThreatScanResult {
        let start = Instant::now();

        let wasm_result = self.wasm_scan(ocr_text);
        let pii_matches = if self.config.pii_redact_screenshots {
            self.pii_patterns.detect(ocr_text)
        } else {
            vec![]
        };

        // Indirect injection detection: look for instruction-like patterns
        // in content that should be data, not commands
        let indirect_score = self.score_indirect_injection(ocr_text);

        self.aggregate_results(
            ThreatSource::ScreenContent,
            wasm_result, vec![], pii_matches, vec![],
            start.elapsed(),
        )
    }

    /// Validate agent action before execution
    /// Catches: privilege escalation, exfiltration, unsafe operations
    pub fn validate_action(&self, action: &AgentAction) -> ThreatScanResult {
        let start = Instant::now();
        let mut threats = vec![];

        match action {
            // Block typing API keys or passwords
            AgentAction::Type { text } => {
                let pii = self.pii_patterns.detect(text);
                if !pii.is_empty() {
                    threats.push(DetectedThreat {
                        threat_type: ThreatType::PiiExposure,
                        confidence: 0.95,
                        source: ThreatSource::AgentAction,
                        matched_pattern: Some("Agent attempting to type sensitive data".into()),
                        recommendation: "Block action and alert user".into(),
                    });
                }
            }
            // Block launching shell/terminal without explicit permission
            AgentAction::LaunchApp { path } => {
                if self.is_shell_or_terminal(path) {
                    threats.push(DetectedThreat {
                        threat_type: ThreatType::PrivilegeEscalation,
                        confidence: 0.9,
                        source: ThreatSource::AgentAction,
                        matched_pattern: Some(format!("Shell launch: {}", path)),
                        recommendation: "Require explicit user confirmation".into(),
                    });
                }
            }
            _ => {}
        }

        ThreatScanResult {
            is_safe: threats.is_empty(),
            threat_level: if threats.is_empty() { ThreatLevel::Safe } else { ThreatLevel::High },
            threats_detected: threats,
            pii_found: vec![],
            scan_latency_us: start.elapsed().as_micros() as u64,
        }
    }

    /// Validate training data before LoRA fine-tuning
    /// Prevents data poisoning attacks
    pub fn validate_training_data(&self, batch: &TrainingBatch) -> Vec<usize> {
        let mut poisoned_indices = vec![];

        for (i, sample) in batch.samples.iter().enumerate() {
            // Check if instruction contains injection patterns
            let cmd_result = self.scan_command(&sample.instruction);
            if !cmd_result.is_safe {
                poisoned_indices.push(i);
                continue;
            }

            // Check if OCR content from screenshots contains adversarial text
            if let Some(ocr) = &sample.ocr_text {
                let screen_result = self.scan_screen_content(ocr);
                if !screen_result.is_safe {
                    poisoned_indices.push(i);
                }
            }
        }

        if !poisoned_indices.is_empty() {
            tracing::warn!(
                "AIDefence: Blocked {}/{} training samples as potentially poisoned",
                poisoned_indices.len(), batch.samples.len()
            );
        }

        poisoned_indices
    }

    /// Learn from confirmed threats (adaptive pattern learning)
    pub fn learn_threat(&mut self, threat: &DetectedThreat, confirmed: bool) {
        if confirmed {
            // Add to HNSW threat pattern index
            let embedding = self.embed_text(
                threat.matched_pattern.as_deref().unwrap_or("")
            );
            self.threat_patterns.add(&embedding).ok();

            // Store adaptive signature
            self.adaptive_signatures.push(ThreatSignature {
                pattern: threat.matched_pattern.clone().unwrap_or_default(),
                threat_type: threat.threat_type.clone(),
                confidence: threat.confidence,
                learned_at: Local::now(),
            });
        }
    }
}
```

#### Architecture: AIDefence in the Agent Pipeline

```
User Command ──→ ┌──────────────┐     ┌──────────────────┐
                  │  AIDefence   │────→│  Agent Executor   │
                  │  WASM Scan   │     │  (if safe)        │
Screen OCR ────→ │  <10ms/scan  │     └──────────────────┘
                  │              │              │
Agent Output ──→ │  • Injection │              ▼
                  │  • PII       │     ┌──────────────────┐
Training Data ─→ │  • Jailbreak │     │  Action Output    │
                  │  • Poisoning │     │  (mouse/keyboard) │
                  └──────┬───────┘     └──────────────────┘
                         │
                         ▼
                  ┌──────────────┐
                  │ Threat Log + │
                  │ HNSW Learn   │
                  └──────────────┘
```

#### AIDefence Configuration

```toml
[agents.security]
# AIDefence WASM module
aidefence_enabled = true
aidefence_wasm = "assets/aidefence.wasm"       # Compiled from @claude-flow/security

# Scan targets
scan_user_commands = true                       # NL instructions
scan_screen_content = true                      # OCR text from screenshots
scan_agent_actions = true                       # Validate before execution
scan_training_data = true                       # LoRA training data validation

# PII protection
pii_redact_screenshots = true                   # Redact PII from stored screenshots
pii_types = ["email", "phone", "ssn", "credit_card", "api_key", "password"]

# Threat response
block_on_threat = true                          # Hard block vs. warn
threat_threshold = 0.7                          # Confidence threshold
max_scan_latency_ms = 10                        # Performance budget per scan

# Adaptive learning
learn_from_detections = true                    # HNSW pattern learning
threat_pattern_namespace = "aidefence_threats"  # Memory namespace
```

## Consequences

### Positive
- **Tiny models run locally**: 2B models fit in 1.5GB VRAM (Q4) - works on most GPUs
- **Watch-and-learn**: Passive observation builds training data without user effort
- **LoRA personalization**: Per-app adapters specialize the base model for each user's applications
- **Self-improving**: ZeroGUI-style online RL means the agent gets better over time automatically
- **No API costs**: All inference and training runs locally via Ollama/RuVLLM/llama.cpp
- **EWC prevents forgetting**: Existing neural engine ensures new learning doesn't erase old
- **Tiered architecture**: Works on everything from CPU-only laptops to 24GB GPU workstations
- **OmniParser is free**: YOLO+Florence provides structured UI parsing without any LLM
- **AIDefence WASM**: Sub-10ms inline security scanning with zero npm runtime dependencies (Wasmer-hosted)
- **Anti-poisoning**: Training data validated before LoRA fine-tuning, preventing adversarial trajectory injection
- **PII redaction**: Screenshots stored for training automatically redacted of sensitive data

### Negative
- GGUF quantized models have lower accuracy than full-precision (mitigated: LoRA fine-tuning recovers accuracy)
- LoRA training requires dedicated GPU time (mitigated: train during idle periods)
- OmniParser YOLO model uses AGPL license (need to evaluate implications)
- UI-TARS-2B is less capable than 7B/72B variants for complex multi-step reasoning
- Online RL training episodes may leave application in unexpected state (mitigated: sandbox/undo)

### Risks
- Fine-tuned models could overfit to specific application versions (mitigated: periodic retraining)
- Trajectory recording captures potentially sensitive screen content (mitigated: AIDefence PII redaction, local-only storage)
- Self-learning could reinforce incorrect behaviors without human oversight (mitigated: reward voting, accuracy monitoring)
- LoRA adapter compatibility across model updates (mitigated: versioned adapters, re-training on base model update)
- Indirect prompt injection via screen content (mitigated: AIDefence WASM scans OCR text before agent processing)
- Data poisoning via adversarial trajectories (mitigated: AIDefence validates training batches pre-fine-tuning)

## Implementation Plan

### Phase 1: Perception Pipeline
- [ ] OmniParser V2 integration (YOLO element detection + Florence captioning)
- [ ] DXGI Desktop Duplication screen capture
- [ ] Windows UIAutomation bridge
- [ ] OCR engine (Windows built-in or Tesseract)
- [ ] Tiered model loading and VRAM management

### Phase 2: Tiny CU Model Integration
- [ ] UI-TARS-2B-SFT deployment via Ollama/llama.cpp/RuVLLM
- [ ] GGUF model auto-download and caching
- [ ] Grounding pipeline: instruction + screenshot -> coordinates
- [ ] Model tier routing (parser -> grounding -> reasoning)
- [ ] Hot-swap between base model and LoRA adapters

### Phase 3: Watch-and-Learn
- [ ] Win32 event hooks for user action observation
- [ ] Trajectory recording with before/after screenshots
- [ ] Auto-description generation from observed actions
- [ ] HNSW-based trajectory deduplication
- [ ] Training data format conversion (screenshots + instructions + actions)

### Phase 4: LoRA Fine-Tuning
- [ ] QLoRA training pipeline (4-bit base + LoRA adapters)
- [ ] Per-application adapter management
- [ ] Training scheduler (idle-time training)
- [ ] EWC++ consolidation after each training run
- [ ] Adapter versioning and validation

### Phase 5: Online RL Self-Improvement
- [ ] Auto-task generation from screen state (ZeroGUI-inspired)
- [ ] Multi-vote reward estimation (no human labels needed)
- [ ] RFT/GRPO training loop
- [ ] Sandboxed training episodes with undo
- [ ] Accuracy monitoring and regression detection

### Phase 6: Action Execution & Safety
- [ ] SendInput-based mouse/keyboard simulation
- [ ] UIAutomation pattern invocation
- [ ] Safety guardrails and confirmation dialogs
- [ ] Audit logging
- [ ] Application-specific agent profiles

### Phase 7: AIDefence WASM Security Gateway
- [ ] Compile @claude-flow/security to WASM via wasm-pack
- [ ] Load AIDefence WASM module via Wasmer 4.3 (ADR-021 runtime)
- [ ] Inline scan pipeline: user commands, screen OCR, agent actions
- [ ] PII detection + screenshot redaction before trajectory storage
- [ ] Training data validation (anti-poisoning) before LoRA fine-tuning
- [ ] HNSW-indexed threat pattern store with adaptive learning
- [ ] Indirect prompt injection detection for screen-observed text
- [ ] Threat audit logging and alerting

## References

### Models
- [UI-TARS](https://github.com/bytedance/UI-TARS) - ByteDance's GUI agent (2B/7B/72B)
- [UI-TARS-2 Technical Report](https://arxiv.org/html/2509.02544v1) - Multi-turn RL advancement
- [UI-TARS Desktop](https://github.com/bytedance/UI-TARS-desktop) - Desktop agent application
- [ShowUI](https://github.com/showlab/ShowUI) - CVPR 2025, Vision-Language-Action for GUI
- [GUI-Actor](https://microsoft.github.io/GUI-Actor/) - Microsoft, coordinate-free grounding
- [Qwen-GUI-3B](https://arxiv.org/html/2506.23491v1) - Lightweight cross-resolution grounding
- [OmniParser V2](https://github.com/microsoft/OmniParser) - Microsoft, pure vision screen parsing
- [OS-Atlas](https://github.com/OS-Copilot/OS-Atlas) - ICLR 2025, foundation action model
- [Aguvis](https://github.com/ZJU-REAL/Awesome-GUI-Agents) - Pure vision GUI agent framework

### Self-Learning & RL
- [ZeroGUI](https://github.com/OpenGVLab/ZeroGUI) - Zero human cost online GUI learning
- [UI-AGILE](https://arxiv.org/html/2507.22025) - SOTA 3B/7B with RFT on 9K examples
- [GUI-RCPO](https://arxiv.org/html/2508.05615v2) - Test-time RL via region consistency

### Fine-Tuning
- [Qwen-VL Fine-Tuning](https://github.com/2U1/Qwen-VL-Series-Finetune) - LoRA for Qwen-VL series
- [LoRA Fine-Tuning Guide](https://datature.io/blog/how-to-fine-tune-qwen2-5-vl) - Qwen2.5-VL with LoRA
- [UI-TARS-2B-SFT GGUF](https://huggingface.co/bartowski/UI-TARS-2B-SFT-GGUF) - Quantized models

### GGUF & Local Deployment
- [UI-TARS on Ollama](https://ollama.com/avil/UI-TARS) - Local deployment
- [UI-TARS-2B-SFT-Q4 GGUF](https://huggingface.co/enacimie/UI-TARS-2B-SFT-Q4_K_M-GGUF)

### Windows APIs
- [Windows UI Automation](https://learn.microsoft.com/en-us/windows/win32/winauto/entry-uiauto-win32)
- [DXGI Desktop Duplication](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)
- [SendInput API](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)

### Security
- [claude-flow Security Module](https://github.com/ruvnet/claude-flow) - AIDefence threat detection, input validation, PII scanning
- [OWASP LLM Prompt Injection](https://genai.owasp.org/llmrisk/llm01-prompt-injection/) - LLM01:2025 attack taxonomy
- [Prompt Injection Defenses](https://github.com/tldrsec/prompt-injection-defenses) - Comprehensive defense catalog
- [WASP Benchmark](https://arxiv.org/pdf/2504.18575) - Web Agent Security against Prompt Injection
- [PIShield](https://arxiv.org/abs/2510.14005) - Intrinsic LLM feature-based injection detection
- [Anthropic Prompt Injection Mitigations](https://platform.claude.com/docs/en/test-and-evaluate/strengthen-guardrails/mitigate-jailbreaks) - Claude guardrail patterns
- [AIDEFEND Framework](https://www.helpnetsecurity.com/2025/09/01/aidefend-free-ai-defense-framework/) - Open AI security knowledge base (MITRE ATLAS, MAESTRO, OWASP)

### Ecosystem
- [2025-2026 AI Computer-Use Benchmarks Guide](https://o-mega.ai/articles/the-2025-2026-guide-to-ai-computer-use-benchmarks-and-top-ai-agents)
- [Curated Computer Use Resources](https://github.com/trycua/acu)
- [GUI Agents Paper List](https://github.com/OSU-NLP-Group/GUI-Agents-Paper-List)
