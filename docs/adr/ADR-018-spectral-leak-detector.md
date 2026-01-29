# ADR-018: Spectral Memory Leak Detector

## Status
**Proposed**

## Date
2026-01-28

## Context

Memory leaks are one of the most insidious system health problems. A leaking application slowly consumes RAM over hours or days, degrading system performance until the user notices and restarts the app (or the system). Traditional leak detection requires developer tools (Valgrind, AddressSanitizer, WinDbg) that are inaccessible to end users.

RuVector already has a spectral analysis algorithm (`src/algorithms/spectral.rs`) designed for detecting anomalous patterns in memory usage. By continuously monitoring per-process memory growth curves, we can detect leaks early - before they become user-visible problems - using statistical anomaly detection rather than code instrumentation.

### Key Insight
A healthy process has a memory curve that levels off (allocate at startup, stabilize). A leaking process has a monotonically increasing memory curve with a positive slope that doesn't decay. The spectral analysis can decompose memory time series into periodic components (normal allocation/deallocation cycles) and trend components (leaks).

## Decision

### 1. Per-Process Memory Time Series

```rust
pub struct ProcessMemoryTracker {
    series: HashMap<u32, MemoryTimeSeries>,  // pid -> time series
    spectral: SpectralAnalyzer,               // From existing module
    alert_threshold: f64,                      // Leak confidence threshold
}

pub struct MemoryTimeSeries {
    pub pid: u32,
    pub name: String,
    pub samples: VecDeque<MemorySample>,      // Ring buffer, 5-min intervals
    pub max_samples: usize,                    // 288 = 24 hours at 5-min
    pub baseline_mb: f64,                      // Memory at first observation
    pub current_mb: f64,                       // Latest sample
    pub growth_rate_mb_per_hour: f64,          // Linear regression slope
    pub leak_confidence: f64,                  // 0.0 - 1.0
    pub spectral_score: f64,                   // Anomaly score from spectral analysis
}

pub struct MemorySample {
    pub timestamp: DateTime<Local>,
    pub working_set_mb: f64,
    pub private_bytes_mb: f64,
    pub page_faults: u64,
    pub handle_count: u32,
    pub gdi_objects: u32,
    pub user_objects: u32,
}
```

### 2. Leak Detection Algorithm

```rust
impl LeakDetector {
    pub fn analyze(&self, series: &MemoryTimeSeries) -> LeakAnalysis {
        if series.samples.len() < 12 { // Need at least 1 hour of data
            return LeakAnalysis::InsufficientData;
        }

        // Step 1: Linear regression on memory over time
        let (slope, r_squared) = self.linear_regression(&series.samples);

        // Step 2: Spectral decomposition
        // Separate periodic components (normal GC cycles, cache flush)
        // from monotonic trend (leak)
        let spectral = self.spectral.decompose(
            &series.samples.iter().map(|s| s.working_set_mb).collect::<Vec<_>>()
        );

        // Step 3: Handle/GDI leak detection
        let handle_leak = self.detect_handle_leak(&series.samples);
        let gdi_leak = self.detect_gdi_leak(&series.samples);

        // Step 4: Confidence scoring
        let confidence = self.compute_confidence(
            slope,
            r_squared,
            &spectral,
            handle_leak,
            gdi_leak,
            series.samples.len(),
        );

        // Step 5: Classify
        if confidence > 0.85 {
            LeakAnalysis::ConfirmedLeak {
                growth_rate_mb_per_hour: slope * 12.0, // samples per hour
                estimated_oom_hours: self.estimate_oom_time(series),
                leak_type: self.classify_leak_type(slope, handle_leak, gdi_leak),
                confidence,
            }
        } else if confidence > 0.5 {
            LeakAnalysis::SuspectedLeak {
                growth_rate_mb_per_hour: slope * 12.0,
                confidence,
                needs_more_data_hours: ((0.85 - confidence) / 0.05).ceil() as u32,
            }
        } else {
            LeakAnalysis::Healthy
        }
    }

    fn compute_confidence(
        &self,
        slope: f64,          // Memory growth rate
        r_squared: f64,      // How linear the growth is (1.0 = perfectly linear)
        spectral: &SpectralDecomposition,
        handle_leak: bool,
        gdi_leak: bool,
        sample_count: usize,
    ) -> f64 {
        let mut confidence = 0.0;

        // Positive slope = growing memory
        if slope > 0.1 { // >0.1 MB per 5-min sample = >1.2 MB/hour
            confidence += 0.3;
        }

        // High R² = consistent growth (not spiky allocation)
        if r_squared > 0.8 {
            confidence += 0.25;
        }

        // Spectral: high trend component vs periodic components
        let trend_ratio = spectral.trend_power / spectral.total_power;
        if trend_ratio > 0.6 {
            confidence += 0.2;
        }

        // Handle leak is a strong signal
        if handle_leak { confidence += 0.15; }
        if gdi_leak { confidence += 0.1; }

        // More data = more confidence
        let data_factor = (sample_count as f64 / 288.0).min(1.0); // max at 24h
        confidence * data_factor
    }

    fn estimate_oom_time(&self, series: &MemoryTimeSeries) -> Option<f64> {
        let available_mb = self.get_available_memory_mb();
        let growth_rate = series.growth_rate_mb_per_hour;

        if growth_rate > 0.0 {
            Some(available_mb / growth_rate)  // Hours until OOM
        } else {
            None
        }
    }
}

pub enum LeakType {
    MemoryLeak,          // Private bytes growing
    HandleLeak,          // Handle count growing
    GdiLeak,             // GDI object count growing
    UserObjectLeak,      // User object count growing
    CacheBloat,          // Cache not being evicted
    Combined(Vec<LeakType>),
}
```

### 3. Handle & GDI Object Leak Detection

Windows-specific leak types that don't involve memory directly but exhaust system resources:

```rust
impl LeakDetector {
    fn detect_handle_leak(&self, samples: &VecDeque<MemorySample>) -> bool {
        // Handles should stabilize after startup
        // If handle count grows >10/hour consistently, it's a leak
        let handle_slope = self.linear_regression_field(samples, |s| s.handle_count as f64);
        handle_slope.0 > 2.0 && handle_slope.1 > 0.7 // >2 handles/sample with R²>0.7
    }

    fn detect_gdi_leak(&self, samples: &VecDeque<MemorySample>) -> bool {
        // GDI object limit is 10,000 per process
        // Growing GDI count = leak that will eventually crash the app
        let gdi_slope = self.linear_regression_field(samples, |s| s.gdi_objects as f64);
        gdi_slope.0 > 0.5 && gdi_slope.1 > 0.7
    }
}
```

### 4. Alerts & Notifications

```rust
pub struct LeakAlert {
    pub pid: u32,
    pub process_name: String,
    pub leak_type: LeakType,
    pub growth_rate: String,          // "2.4 MB/hour"
    pub current_usage: String,        // "1.8 GB"
    pub estimated_oom: Option<String>, // "~6 hours"
    pub confidence: f64,
    pub recommendation: LeakRecommendation,
}

pub enum LeakRecommendation {
    RestartApp,                       // Restart the leaking application
    ReduceUsage(String),              // "Close unused tabs in Chrome"
    UpdateApp,                        // Known leak fixed in newer version
    ReportBug(String),                // Link to app's issue tracker
    MonitoringContinues,              // Low confidence, keep watching
}
```

Banner notification:
```
┌──────────────────────────────────────────────────┐
│ ▎ Memory Leak Detected                           │
│ ▎ Chrome.exe growing 4.2 MB/hr · 3.1 GB now     │
│ ▎ Estimated issue in ~8 hours  [Restart] [Ignore]│
└──────────────────────────────────────────────────┘
```

### 5. Known Leak Database

```rust
pub struct KnownLeakDatabase {
    entries: Vec<KnownLeak>,
}

pub struct KnownLeak {
    pub process_name: String,
    pub affected_versions: Vec<String>,
    pub leak_type: LeakType,
    pub workaround: String,
    pub fixed_in_version: Option<String>,
    pub source_url: String,
}

// Community-maintained database of known leaks:
// - Chrome tab leak (>50 tabs)
// - Teams memory growth (meetings)
// - Electron apps (VS Code extensions)
// - Adobe Creative Cloud agent
// - OneDrive sync engine
```

### 6. Spectral Decomposition Integration

Use existing `SpectralAnalyzer` to separate normal patterns from leaks:

```rust
pub struct SpectralDecomposition {
    pub trend_power: f64,      // Monotonic growth component
    pub periodic_power: f64,   // Cyclic patterns (GC, cache flush)
    pub noise_power: f64,      // Random fluctuations
    pub total_power: f64,
    pub dominant_period: Option<Duration>,  // Strongest cycle length
}

// A healthy process: trend_power << periodic_power + noise_power
// A leaking process: trend_power >> periodic_power + noise_power
// A bursty process: noise_power >> trend_power + periodic_power
```

## Consequences

### Positive
- Detects leaks before they cause user-visible problems
- No code instrumentation needed (pure external observation)
- Spectral analysis distinguishes leaks from normal usage patterns
- Handle/GDI leak detection catches non-memory resource exhaustion
- Known leak database provides immediate workarounds
- OOM time estimation gives users urgency context

### Negative
- 5-minute sampling may miss fast leaks (mitigated: suspicious processes get 30s sampling)
- False positives during initial application loading (mitigated: ignore first 10 minutes)
- Memory growth ≠ leak in all cases (growing cache is intentional)
- Per-process monitoring adds ~1% CPU overhead for tracking

### Security Considerations
- **Process monitoring privacy**: Time-series data stored locally only, encrypted at rest (AES-256-GCM), auto-deleted after 30 days
- **No external transmission**: Leak detection data never leaves the device; known leak database updates fetched via HTTPS with certificate pinning
- **Alert rate limiting**: Maximum 1 alert per process per hour to prevent notification fatigue and denial-of-service via false positives
- **Known leak database integrity**: Database file signed with Ed25519; signature verified before loading; tampered databases rejected
- **URL validation**: `ReportBug` URLs validated against known issue tracker domains (github.com, gitlab.com, etc.) to prevent phishing links

### Risks
- Users may blame RuVector for false alerts
- Some leaks are in Windows components (can't recommend restart)
- Corporate environments may restrict process monitoring

## Implementation Plan

### Phase 1: Time Series Collection
- [ ] Per-process memory sampling (working set, private bytes, handles, GDI)
- [ ] Ring buffer storage (24 hours per tracked process)
- [ ] Top-N process selection (don't track all 200+ processes)

### Phase 2: Detection
- [ ] Linear regression on memory curves
- [ ] Handle and GDI object trend analysis
- [ ] Integration with existing spectral analyzer

### Phase 3: Alerts
- [ ] Leak classification and confidence scoring
- [ ] Banner notifications with action buttons
- [ ] OOM time estimation

### Phase 4: Intelligence
- [ ] Known leak database (built-in + community updates)
- [ ] Neural learning for false positive reduction
- [ ] Per-application baseline establishment

## References

- [Working Set vs Private Bytes](https://learn.microsoft.com/en-us/windows/win32/memory/working-set)
- [GDI Objects](https://learn.microsoft.com/en-us/windows/win32/sysinfo/gdi-objects)
- [Handle Count](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesshandlecount)
- [Spectral Analysis for Anomaly Detection](https://arxiv.org/abs/1906.03821)
