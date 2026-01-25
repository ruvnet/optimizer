# ADR-005: Flow State Detection & Productivity Optimization

## Status
Proposed

## Date
2025-01-25

## Context

"Flow state" is the mental state of complete immersion and focus. During flow:
- Single application focus
- Minimal context switching
- Consistent memory patterns
- Predictable process behavior

Interruptions during flow are costly:
- 23 minutes average to regain focus
- Reduced productivity and creativity
- Increased stress and frustration

RuVector can detect flow state from memory patterns and optimize accordingly:
- Defer non-critical optimizations
- Suppress notifications
- Pre-load anticipated resources
- Protect focused application priority

## Decision

Implement **Flow State Detection** with productivity-aware optimization scheduling.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                   Flow State Detection System                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                  Activity Monitor                         │   │
│  │  • Foreground app tracking                               │   │
│  │  • Window switch frequency                               │   │
│  │  • Input activity patterns                               │   │
│  │  • Memory stability metrics                              │   │
│  └─────────────────────────────────────────────────────────┘   │
│                            │                                     │
│                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                  Flow Classifier                          │   │
│  │                                                           │   │
│  │  States:  [Distracted] ─▶ [Focusing] ─▶ [Flow] ─▶ [Deep] │   │
│  │                 │              │           │         │    │   │
│  │  Optimize:    Normal       Reduce      Defer      Protect │   │
│  │                                                           │   │
│  └─────────────────────────────────────────────────────────┘   │
│                            │                                     │
│                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                Optimization Scheduler                     │   │
│  │  • Queue non-critical optimizations                      │   │
│  │  • Protect focused app memory                            │   │
│  │  • Pre-load predicted resources                          │   │
│  │  • Schedule cleanup during breaks                        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Flow State Indicators

| Indicator | Distracted | Focusing | Flow | Deep Flow |
|-----------|------------|----------|------|-----------|
| App switches/min | > 5 | 2-5 | < 2 | 0 |
| Foreground stability | < 30s | 30s-2m | 2-10m | > 10m |
| Memory volatility | High | Medium | Low | Very Low |
| Input pattern | Erratic | Variable | Consistent | Rhythmic |
| Time in state | - | > 2m | > 10m | > 30m |

### Feature Vector

```rust
pub struct FlowStateFeatures {
    // Application focus
    pub foreground_app: String,
    pub foreground_duration_secs: u64,
    pub app_switches_last_5min: u32,

    // Input patterns
    pub keystrokes_per_min: f32,
    pub mouse_distance_per_min: f32,
    pub input_consistency: f32,      // 0-1 regularity score

    // Memory patterns
    pub memory_volatility: f32,
    pub foreground_app_memory_trend: MemoryTrend,
    pub background_activity_level: f32,

    // Temporal context
    pub time_of_day: u8,
    pub session_duration_mins: u32,
    pub break_since_mins: u32,
}

pub enum FlowState {
    Distracted,
    Focusing { progress: f32 },
    Flow { duration_mins: u32 },
    DeepFlow { duration_mins: u32 },
    Break,
}
```

### API Design

```rust
pub struct FlowDetector {
    state: FlowState,
    history: FlowHistory,
    scheduler: OptimizationScheduler,
}

impl FlowDetector {
    /// Get current flow state
    pub fn current_state(&self) -> FlowState;

    /// Get flow score (0-100)
    pub fn flow_score(&self) -> u8;

    /// Check if optimization should be deferred
    pub fn should_defer_optimization(&self) -> bool;

    /// Get optimal time for next optimization
    pub fn next_optimization_window(&self) -> Option<Duration>;

    /// Get productivity insights
    pub fn daily_summary(&self) -> ProductivitySummary;
}

pub struct ProductivitySummary {
    pub total_flow_minutes: u32,
    pub longest_flow_session: u32,
    pub flow_sessions_count: u32,
    pub most_productive_hours: Vec<u8>,
    pub focus_score: u8,  // 0-100
}
```

### Optimization Behavior by State

| State | Memory Optimization | Notifications | Background Tasks |
|-------|--------------------|--------------| ----------------|
| Distracted | Normal | Normal | Normal |
| Focusing | Reduced frequency | Batched | Deferred |
| Flow | Deferred | Suppressed | Blocked |
| Deep Flow | Fully deferred | Blocked | Blocked |
| Break | Aggressive | Normal | Allowed |

### Integration Points

1. **Tray Icon** - Color/icon indicates flow state
2. **Auto DND** - System Do Not Disturb during flow
3. **Calendar** - Predict flow windows from patterns
4. **Reports** - Weekly productivity insights

## Consequences

### Positive
- Protects user focus automatically
- Learns individual productivity patterns
- Optimizes at better times
- Provides productivity insights

### Negative
- Delayed optimizations may accumulate
- Input monitoring may raise privacy concerns
- Learning period needed
- May conflict with urgent optimization needs

### Risks
- Over-protection may delay critical optimizations
- Users may become dependent on system
- False flow detection may frustrate users

## Implementation Phases

### Phase 1: Activity Monitoring (1 week)
- Foreground app tracking
- Basic switch counting
- Memory pattern correlation

### Phase 2: State Machine (1 week)
- Flow state classifier
- State transition logic
- Hysteresis for stability

### Phase 3: Scheduler Integration (1 week)
- Deferred optimization queue
- Break detection
- Catch-up optimization

### Phase 4: Insights & UI (1 week)
- Tray icon flow indicator
- Daily/weekly summaries
- Settings for sensitivity

## Success Metrics

| Metric | Target |
|--------|--------|
| Flow Detection Accuracy | > 85% |
| User-Reported Interruptions | -50% |
| Optimization Effectiveness | Unchanged |
| CPU Overhead | < 0.5% |

## References

- [Flow: The Psychology of Optimal Experience](https://www.harpercollins.com/products/flow-mihaly-csikszentmihalyi)
- [The Cost of Interrupted Work](https://www.ics.uci.edu/~gmark/chi08-mark.pdf)
- Existing: `src/tray/mod.rs`, `src/neural/attention.rs`
