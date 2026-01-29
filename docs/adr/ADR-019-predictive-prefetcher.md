# ADR-019: Predictive Prefetcher

## Status
**Proposed**

## Date
2026-01-28

## Context

Users follow habitual patterns when using their computers. A developer opens VS Code, then a terminal, then Chrome for documentation - every morning. A designer opens Photoshop, then Illustrator, then a file browser. These sequences are predictable with high accuracy after 1-2 weeks of observation.

Windows Superfetch/SysMain prefetches based on simple frequency heuristics. RuVector's neural engine can learn temporal sequences (what app follows what app, at what time, on what day) and preload the next application into standby memory before the user clicks it. This reduces "time to first paint" from seconds to near-instant.

### RuVector Advantage
- Neural engine with temporal attention scoring (existing `AttentionScorer`)
- HNSW pattern matching (existing `PatternIndex`)
- Memory management infrastructure (can prefetch into standby)
- Time-of-day awareness (existing in neural features)

## Decision

### 1. Sequence Learning Model

```rust
pub struct AppSequenceModel {
    transitions: HashMap<AppContext, Vec<Prediction>>,
    temporal_weights: [f32; 168],  // 24 hours * 7 days
    attention: AttentionScorer,
    min_confidence: f64,           // Only prefetch above this threshold
    training_samples: u32,
}

pub struct AppContext {
    pub current_app: String,
    pub time_bucket: u8,           // 0-23 (hour)
    pub day_of_week: u8,           // 0-6
    pub recent_apps: Vec<String>,  // Last 3 apps launched (Markov chain context)
}

pub struct Prediction {
    pub app_name: String,
    pub app_path: PathBuf,
    pub confidence: f64,           // 0.0 - 1.0
    pub avg_delay_seconds: f64,    // Typical time until user launches this
    pub prefetch_benefit_ms: u64,  // Estimated time saved by prefetching
}
```

### 2. Observation & Training

```rust
pub struct AppObserver {
    current_foreground: Option<String>,
    launch_history: VecDeque<AppLaunch>,
    max_history: usize,  // 10,000 events (~1 month)
}

pub struct AppLaunch {
    pub app_name: String,
    pub app_path: PathBuf,
    pub timestamp: DateTime<Local>,
    pub day_of_week: u8,
    pub hour: u8,
    pub previous_app: Option<String>,
    pub time_since_previous_ms: u64,
    pub memory_at_launch_mb: f64,
}

impl AppObserver {
    /// Called when foreground window changes
    pub fn on_foreground_change(&mut self, new_app: &str, app_path: &Path) {
        let launch = AppLaunch {
            app_name: new_app.to_string(),
            app_path: app_path.to_path_buf(),
            timestamp: Local::now(),
            day_of_week: Local::now().weekday().num_days_from_monday() as u8,
            hour: Local::now().hour() as u8,
            previous_app: self.current_foreground.clone(),
            time_since_previous_ms: /* calculate */,
            memory_at_launch_mb: /* query */,
        };
        self.launch_history.push_back(launch);
        self.current_foreground = Some(new_app.to_string());
    }

    /// Train the sequence model from accumulated observations
    pub fn train(&self) -> AppSequenceModel {
        let mut transitions: HashMap<AppContext, Vec<(String, u32)>> = HashMap::new();

        for window in self.launch_history.windows(2) {
            let context = AppContext {
                current_app: window[0].app_name.clone(),
                time_bucket: window[0].hour,
                day_of_week: window[0].day_of_week,
                recent_apps: vec![], // Extended context from previous launches
            };

            transitions.entry(context)
                .or_default()
                .push((window[1].app_name.clone(), 1));
        }

        // Convert counts to probabilities
        // Apply temporal weighting (morning patterns vs evening)
        // Build confidence scores
    }
}
```

### 3. Prefetch Engine

```rust
pub struct Prefetcher {
    model: AppSequenceModel,
    prefetched: HashSet<String>,     // Currently prefetched apps
    max_prefetch_mb: u64,            // Memory budget for prefetching
}

impl Prefetcher {
    /// Called when foreground app changes - predict and prefetch next apps
    pub async fn on_app_switch(&mut self, current_app: &str) {
        let context = AppContext {
            current_app: current_app.to_string(),
            time_bucket: Local::now().hour() as u8,
            day_of_week: Local::now().weekday().num_days_from_monday() as u8,
            recent_apps: self.get_recent_apps(),
        };

        let predictions = self.model.predict(&context);

        for pred in predictions.iter().take(3) {
            if pred.confidence >= self.model.min_confidence
                && !self.prefetched.contains(&pred.app_name)
            {
                self.prefetch(&pred).await;
            }
        }
    }

    async fn prefetch(&mut self, prediction: &Prediction) {
        // Strategy 1: Pre-read the application binary into OS file cache
        // This is safe and non-destructive - just reads the file
        let _ = tokio::fs::read(&prediction.app_path).await;

        // Strategy 2: Pre-read common DLLs used by the application
        if let Some(dlls) = self.known_dependencies(&prediction.app_name) {
            for dll in dlls {
                let _ = tokio::fs::read(&dll).await;
            }
        }

        // Strategy 3: For apps with known config locations, pre-read configs
        if let Some(config_paths) = self.known_configs(&prediction.app_name) {
            for path in config_paths {
                let _ = tokio::fs::read(&path).await;
            }
        }

        self.prefetched.insert(prediction.app_name.clone());
    }
}
```

### 4. Measuring Prefetch Benefit

```rust
pub struct PrefetchMetrics {
    pub predictions_made: u64,
    pub predictions_correct: u64,     // User launched the predicted app
    pub accuracy: f64,                 // correct / made
    pub avg_time_saved_ms: f64,        // Measured startup time difference
    pub total_time_saved_seconds: f64, // Cumulative
}

impl Prefetcher {
    fn measure_benefit(&mut self, app: &str, launch_time_ms: u64) {
        // Compare launch time to baseline (without prefetch)
        // Baseline is established from the app's first few non-prefetched launches
        let baseline = self.baselines.get(app).unwrap_or(&3000); // 3s default
        let saved = (*baseline as i64 - launch_time_ms as i64).max(0) as u64;
        self.metrics.avg_time_saved_ms =
            (self.metrics.avg_time_saved_ms * 0.95) + (saved as f64 * 0.05);
    }
}
```

### 5. Configuration

```toml
[prefetcher]
enabled = true
min_confidence = 0.7          # Only prefetch if >70% confident
max_prefetch_mb = 512          # Don't use more than 512MB for prefetching
max_simultaneous = 3           # Prefetch at most 3 apps ahead
training_period_days = 7       # Wait 7 days before first predictions
respect_memory_pressure = true # Don't prefetch if memory >80% used
```

## Consequences

### Positive
- Near-instant application launches for habitual workflows
- Measurable time savings (typical 1-3 seconds per app launch)
- Neural learning adapts to changing habits
- Passive operation - no user configuration needed
- Uses existing standby memory (doesn't consume active RAM)

### Negative
- Training period required (1-2 weeks for good predictions)
- Prefetching uses I/O bandwidth (mitigated: low-priority I/O)
- Incorrect predictions waste I/O and cache space
- Privacy consideration: app launch history is stored locally

### Security Considerations
- **Behavioral data encryption**: App launch history encrypted at rest with AES-256-GCM using DPAPI-derived key (tied to Windows user account)
- **Data minimization**: Only app names and hourly time buckets stored; no window titles, URLs, or document names
- **Opt-out and deletion**: User can disable prefetcher and purge all history with one click; data wiped with secure zeroing
- **Path traversal prevention**: `app_path` validated against known executable directories (`C:\Program Files\`, `C:\Windows\`, user AppData); arbitrary paths rejected
- **No side-channel leakage**: Prefetch I/O performed at lowest priority (`FILE_FLAG_SEQUENTIAL_SCAN`) to avoid observable timing differences
- **Local-only processing**: Prediction model runs entirely on-device; no telemetry, no cloud sync of behavioral patterns
- **Anti-fingerprinting**: Launch history cannot be queried by other processes; IPC does not expose prediction data

### Risks
- Users with unpredictable workflows see little benefit
- Prefetching during memory pressure could worsen performance
- SSD wear from additional reads (negligible: <1GB/day extra)

## Implementation Plan

### Phase 1: Observation
- [ ] Foreground window change detection (`SetWinEventHook`)
- [ ] App launch history recording
- [ ] Time-of-day and day-of-week bucketing

### Phase 2: Model
- [ ] Markov chain transition matrix
- [ ] Temporal weighting (morning/evening patterns)
- [ ] Confidence scoring

### Phase 3: Prefetching
- [ ] File cache pre-reading
- [ ] DLL dependency discovery
- [ ] Memory budget management

### Phase 4: Measurement
- [ ] Launch time measurement
- [ ] Prediction accuracy tracking
- [ ] Time-saved reporting

## References

- [Windows Superfetch/SysMain](https://learn.microsoft.com/en-us/windows/win32/memory/superfetch)
- [SetWinEventHook for foreground changes](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwineventhook)
- [Markov Chain prediction](https://en.wikipedia.org/wiki/Markov_chain)
- [Temporal attention mechanisms](https://arxiv.org/abs/1706.03762)
