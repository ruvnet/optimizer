# ADR-006: Frame Drop Oracle - Predictive Gaming Optimization

## Status
Proposed

## Date
2025-01-25

## Context

Gaming performance issues (frame drops, stutters) have predictable precursors:
- Memory pressure building up
- Background process memory spikes
- VRAM approaching limits
- Page file activity increasing
- Thermal throttling approaching

By the time a frame drop occurs, it's too late - the user already experienced it.

RuVector can predict frame drops 2-5 seconds before they happen by monitoring:
- Memory allocation trends
- VRAM usage trajectory
- Background process activity
- Thermal headroom

## Decision

Implement **Predictive Frame Drop Prevention** for gaming workloads.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Frame Drop Oracle System                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
│  │   Sensors    │───▶│  Predictor   │───▶│  Preventer   │      │
│  │              │    │              │    │              │      │
│  │ • RAM usage  │    │ • LSTM/GRU   │    │ • Pre-trim   │      │
│  │ • VRAM usage │    │ • Trend      │    │ • Priority   │      │
│  │ • Page file  │    │   analysis   │    │   boost      │      │
│  │ • Thermals   │    │ • Threshold  │    │ • BG pause   │      │
│  │ • Process    │    │   crossing   │    │ • VRAM free  │      │
│  └──────────────┘    └──────────────┘    └──────────────┘      │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Prediction Window                       │  │
│  │                                                            │  │
│  │   Now          +2s         +3s         +4s        +5s     │  │
│  │    │            │           │           │          │       │  │
│  │    ▼            ▼           ▼           ▼          ▼       │  │
│  │  [Sense] ───▶ [Predict] ─▶ [Act] ──▶ [Verify] ─▶ [Learn] │  │
│  │                                                            │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Prediction Features

| Feature | Sample Rate | Prediction Horizon |
|---------|-------------|-------------------|
| RAM usage trend | 100ms | 2-3 seconds |
| VRAM usage trend | 100ms | 2-3 seconds |
| Page file activity | 200ms | 3-5 seconds |
| GPU temperature | 500ms | 5-10 seconds |
| Background process spikes | 500ms | 1-2 seconds |

### Stutter Precursor Signatures

```rust
pub struct StutterPrecursor {
    /// RAM approaching working set limit
    pub ram_pressure: f32,           // 0-1, >0.85 = warning

    /// VRAM nearing capacity
    pub vram_pressure: f32,          // 0-1, >0.90 = warning

    /// Page file write rate (indicates RAM overflow)
    pub pagefile_write_mb_s: f32,    // >10 MB/s = warning

    /// Thermal headroom
    pub thermal_headroom_c: f32,     // <10°C = warning

    /// Background memory spike detected
    pub bg_spike_detected: bool,

    /// Composite stutter probability
    pub stutter_probability: f32,    // 0-1

    /// Estimated time to stutter
    pub eta_ms: Option<u32>,
}
```

### Prevention Actions

| Probability | ETA | Action |
|-------------|-----|--------|
| > 0.3 | > 5s | Monitor closely |
| > 0.5 | 3-5s | Soft background trim |
| > 0.7 | 2-3s | Aggressive trim + priority boost |
| > 0.9 | < 2s | Emergency VRAM/RAM free |

### API Design

```rust
pub struct FrameDropOracle {
    game_process: Option<u32>,
    sensors: SensorArray,
    predictor: StutterPredictor,
    preventer: StutterPreventer,
}

impl FrameDropOracle {
    /// Start monitoring for a game
    pub fn monitor_game(&mut self, pid: u32);

    /// Get current stutter prediction
    pub fn predict(&self) -> StutterPrecursor;

    /// Get prevention recommendations
    pub fn get_actions(&self) -> Vec<PreventionAction>;

    /// Execute prevention automatically
    pub fn auto_prevent(&mut self) -> PreventionResult;

    /// Learn from actual stutter event
    pub fn record_stutter(&mut self, timestamp: Instant);

    /// Get session statistics
    pub fn session_stats(&self) -> GamingSessionStats;
}

pub struct GamingSessionStats {
    pub stutters_predicted: u32,
    pub stutters_prevented: u32,
    pub false_positives: u32,
    pub avg_prediction_lead_time_ms: u32,
    pub memory_freed_mb: f64,
}
```

### Game Detection Integration

Leverages existing Game Mode detection:
```rust
// From existing is_game_running()
const GAME_PROCESSES: &[&str] = &[
    "valorant", "csgo", "cs2", "fortnite", "minecraft",
    // ... 40+ games
];
```

### Tray Integration

- **Gaming Icon State**: Controller icon when game detected
- **Prediction Indicator**: Color bar showing stutter probability
- **Stats Tooltip**: "Prevented 12 stutters this session"

## Consequences

### Positive
- Prevents stutters before they happen
- Learns game-specific patterns
- Minimal intervention when not needed
- Measurable improvement in gaming experience

### Negative
- May cause brief micro-stutters from prevention actions
- False positives may trigger unnecessary optimization
- Some games may not benefit (CPU-bound)
- Requires GPU monitoring (NVML/DXGI)

### Risks
- Prevention action itself causes stutter
- Aggressive optimization affects game state
- GPU monitoring overhead

## Implementation Phases

### Phase 1: Sensor Infrastructure (1 week)
- High-frequency memory sampling
- VRAM monitoring integration
- Page file activity tracking

### Phase 2: Prediction Model (2 weeks)
- Trend analysis algorithms
- Threshold crossing detection
- Probability estimation

### Phase 3: Prevention Actions (1 week)
- Tiered response system
- Game-safe optimization methods
- Priority boosting

### Phase 4: Learning & Tuning (2 weeks)
- Per-game calibration
- False positive reduction
- User feedback integration

## Success Metrics

| Metric | Target |
|--------|--------|
| Prediction Accuracy | > 75% |
| Prevention Success Rate | > 60% |
| False Positive Rate | < 20% |
| Prediction Lead Time | > 2 seconds |
| Performance Overhead | < 0.5% |

## References

- [NVIDIA Reflex](https://www.nvidia.com/en-us/geforce/technologies/reflex/)
- [Frame Pacing Analysis](https://developer.nvidia.com/frame-pacing)
- Existing: `src/ai/modes.rs` (Game Mode), `src/ai/gpu.rs`
