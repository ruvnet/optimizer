# ADR-003: Behavioral Fingerprint Authentication

## Status
Proposed

## Date
2025-01-25

## Context

Traditional authentication methods (passwords, biometrics) have limitations:
- Passwords can be stolen, phished, or forgotten
- Biometrics require special hardware and can be spoofed
- Both are point-in-time checks, not continuous

RuVector already collects rich behavioral data:
- Memory usage patterns over time
- Process launch sequences and frequencies
- Application switching patterns
- Temporal usage patterns (time of day, day of week)

This data creates a unique "fingerprint" that could authenticate users based on HOW they use their computer, not what they know or have.

## Decision

Implement **Continuous Behavioral Authentication** using memory and process patterns.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                 Behavioral Fingerprint System                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
│  │   Collector  │───▶│   Encoder    │───▶│   Matcher    │      │
│  │              │    │              │    │              │      │
│  │ • Processes  │    │ • Embedding  │    │ • Similarity │      │
│  │ • Memory     │    │ • Normalize  │    │ • Threshold  │      │
│  │ • Timing     │    │ • Compress   │    │ • Confidence │      │
│  └──────────────┘    └──────────────┘    └──────────────┘      │
│          │                   │                   │               │
│          ▼                   ▼                   ▼               │
│  ┌──────────────────────────────────────────────────────┐      │
│  │                  Pattern Database                      │      │
│  │  • Stored user profiles (HNSW indexed)                │      │
│  │  • Temporal baselines                                  │      │
│  │  • Anomaly thresholds                                  │      │
│  └──────────────────────────────────────────────────────┘      │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Features Extracted

| Category | Features |
|----------|----------|
| **Process** | Launch order, frequency, co-occurrence, dwell time |
| **Memory** | Usage patterns, peak times, allocation rates |
| **Temporal** | Active hours, break patterns, session lengths |
| **Behavioral** | App switching speed, multitasking patterns |

### API Design

```rust
pub struct BehavioralFingerprint {
    user_id: String,
    embedding: Vec<f32>,        // 128-dim behavioral embedding
    confidence: f32,            // 0.0-1.0 match confidence
    last_updated: DateTime,
}

impl BehavioralAuth {
    /// Enroll a new user (learning period: 1-7 days)
    pub fn enroll(&mut self, user_id: &str) -> EnrollmentStatus;

    /// Continuous authentication check
    pub fn verify(&self) -> AuthResult;

    /// Get current confidence level
    pub fn confidence(&self) -> f32;

    /// Check for anomalous behavior
    pub fn detect_anomaly(&self) -> Option<AnomalyReport>;
}
```

### Use Cases

1. **Passive MFA** - Second factor that requires no user action
2. **Session Locking** - Auto-lock when behavior deviates
3. **Insider Threat** - Detect account compromise
4. **Shared Computer** - Identify which user is active

### Privacy Considerations

- All data stored locally, never transmitted
- Embeddings are one-way (can't reconstruct behavior)
- User controls enrollment and can delete profile
- Opt-in only, clearly explained

## Consequences

### Positive
- Passwordless continuous authentication
- Detects account takeover in real-time
- Zero user friction after enrollment
- Works with existing RuVector infrastructure

### Negative
- 1-7 day learning period required
- Behavior changes (new apps, schedule) need re-calibration
- False positives possible during unusual usage
- Privacy perception concerns

### Risks
- Behavior can be observed and mimicked (mitigated by complexity)
- Requires consistent usage patterns
- May not work for highly variable users

## Implementation Phases

### Phase 1: Data Collection (2 weeks)
- Extend pattern collection
- Build feature extraction pipeline
- Create training data format

### Phase 2: Model Training (2 weeks)
- Train embedding model
- Build HNSW index for matching
- Tune similarity thresholds

### Phase 3: Integration (1 week)
- Tray icon confidence indicator
- Lock trigger on anomaly
- Settings UI for enrollment

### Phase 4: Hardening (2 weeks)
- Anti-spoofing measures
- Adaptive threshold tuning
- Performance optimization

## Success Metrics

| Metric | Target |
|--------|--------|
| True Positive Rate | > 95% |
| False Positive Rate | < 2% |
| Detection Latency | < 30 seconds |
| CPU Overhead | < 1% |

## References

- [Behavioral Biometrics Survey](https://arxiv.org/abs/2008.09742)
- [Continuous Authentication Systems](https://dl.acm.org/doi/10.1145/3243734.3243778)
- Existing: `src/neural/hnsw_patterns.rs`, `src/core/patterns.rs`
