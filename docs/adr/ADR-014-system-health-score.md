# ADR-014: System Health Score

## Status
**Proposed**

## Date
2026-01-28

## Context

Users lack a unified, at-a-glance metric for overall system health. Windows Task Manager shows raw CPU/RAM/Disk percentages but provides no interpretation. Users cannot answer: "Is my system healthy?" without expertise. RuVector has the data sources to synthesize a single actionable number.

A System Health Score (0-100) acts as a "credit score for your PC" - always visible in the tray, with drill-down to individual factors and actionable recommendations.

### Requirements
- Single 0-100 composite score
- Sub-scores for each dimension (memory, thermal, disk, startup, security, bloat)
- Real-time updates (every 5 seconds)
- Trend tracking (is health improving or degrading over days/weeks?)
- Actionable recommendations tied to each sub-score
- Tray icon badge showing current score
- Historical score graph

## Decision

### 1. Health Dimensions & Weights

```rust
pub struct HealthScore {
    pub total: u32,              // 0-100 composite
    pub grade: HealthGrade,      // A+ through F
    pub dimensions: Vec<HealthDimension>,
    pub trend: HealthTrend,      // Improving, Stable, Degrading
    pub recommendations: Vec<Recommendation>,
}

pub struct HealthDimension {
    pub name: String,
    pub score: u32,              // 0-100
    pub weight: f64,             // Contribution to total
    pub details: String,         // Human-readable explanation
    pub severity: Severity,      // Good, Warning, Critical
}
```

| Dimension | Weight | Scoring Criteria | Data Source |
|-----------|--------|-----------------|-------------|
| Memory Pressure | 20% | Available %, page fault rate, swap usage | `MEMORYSTATUSEX`, `sysinfo` |
| Thermal Headroom | 15% | Distance from throttle temp, fan RPM | WMI `MSAcpi_ThermalZoneTemperature` |
| Disk Health | 15% | SMART attributes, free space %, I/O latency | WMI `MSFT_Disk`, `GetDiskFreeSpaceEx` |
| Startup Impact | 10% | Boot time, startup item count, deferred vs. immediate | Registry + Task Scheduler query |
| Process Bloat | 10% | Background process count, idle CPU/RAM waste | `sysinfo` + PageRank scoring |
| Security Posture | 20% | Defender status, firewall, OS update age, unsigned startup items | WMI `SecurityCenter2`, Authenticode |
| Driver Health | 5% | Driver age, crash count (Event Log) | `SetupDiGetDeviceRegistryProperty` |

### 2. Scoring Algorithm

```rust
impl HealthScorer {
    pub fn compute(&self) -> HealthScore {
        let mut dimensions = vec![];

        // Memory (25%)
        let mem = self.score_memory();
        // Score: 100 if <50% used, linear decay to 0 at 100% used
        // Penalty: -20 if swap > 50% used
        // Penalty: -10 if page faults > 1000/sec
        dimensions.push(mem);

        // Thermal (15%)
        let thermal = self.score_thermal();
        // Score: 100 if <60°C, linear decay to 0 at TjMax
        // Penalty: -30 if currently throttling
        // Bonus: +10 if fans are silent (<1000 RPM)
        dimensions.push(thermal);

        // Disk (15%)
        let disk = self.score_disk();
        // Score: 100 if >50% free, decay to 0 at 5% free
        // Penalty: -40 if SMART predicts failure
        // Penalty: -20 if avg I/O latency > 50ms
        dimensions.push(disk);

        // Startup (10%)
        let startup = self.score_startup();
        // Score: 100 if <5 startup items, -5 per additional item
        // Penalty: -20 if boot time > 60s
        // Bonus: +10 if all startup items are deferred
        dimensions.push(startup);

        // Process bloat (15%)
        let bloat = self.score_bloat();
        // Score: 100 if <80 processes, -1 per additional process
        // Penalty: -5 per process using >0.5% CPU while backgrounded
        // Uses PageRank: low-rank background processes are "bloat"
        dimensions.push(bloat);

        // Security (10%)
        let security = self.score_security();
        // Score: 100 if Defender active + firewall on + updates <7 days
        // Penalty: -30 if Defender disabled
        // Penalty: -20 if updates >30 days old
        // Penalty: -10 if firewall disabled
        dimensions.push(security);

        // Drivers (5%)
        let drivers = self.score_drivers();
        // Score: 100 if no crashes in 30 days, -10 per crash
        // Penalty: -20 if any driver >2 years old
        dimensions.push(drivers);

        // Network (5%)
        let network = self.score_network();
        // Score: 100 if DNS <20ms + 0% packet loss
        // Penalty: -20 if DNS >100ms
        // Penalty: -30 if packet loss >1%
        dimensions.push(network);

        let total = dimensions.iter()
            .map(|d| d.score as f64 * d.weight)
            .sum::<f64>() as u32;

        HealthScore {
            total: total.min(100),
            grade: HealthGrade::from_score(total),
            dimensions,
            trend: self.compute_trend(),
            recommendations: self.generate_recommendations(&dimensions),
        }
    }
}
```

### 3. Grade System

| Score | Grade | Tray Color | Meaning |
|-------|-------|-----------|---------|
| 90-100 | A+ | Green | Excellent - system running optimally |
| 80-89 | A | Green | Great - minor improvements possible |
| 70-79 | B | Blue | Good - some areas need attention |
| 60-69 | C | Orange | Fair - several issues detected |
| 40-59 | D | Orange | Poor - significant problems |
| 0-39 | F | Red | Critical - immediate action needed |

### 4. Recommendations Engine

Each sub-score generates specific, actionable recommendations:

```rust
pub struct Recommendation {
    pub title: String,
    pub description: String,
    pub impact: u32,           // Estimated score improvement (points)
    pub effort: Effort,        // Easy, Medium, Hard
    pub action: Action,        // What RuVector can do
    pub category: String,      // Which dimension
}

pub enum Action {
    OneClick(Box<dyn Fn() -> Result<(), String>>),  // RuVector does it
    Guide(String),                                   // Link to instructions
    External(String),                                // Open external tool
}

// Examples:
// "Free 2.4 GB of memory" -> OneClick(optimize_memory)
// "Remove 4 unnecessary startup items" -> OneClick(disable_startup_items)
// "Update NVIDIA driver (347 days old)" -> External(nvidia_update_url)
// "Enable Windows Defender real-time protection" -> Guide(defender_howto)
// "Clean 8.2 GB of temp files" -> OneClick(clean_temp_files)
```

### 5. Trend Tracking

```rust
pub struct TrendTracker {
    history: VecDeque<(DateTime<Local>, u32)>,  // (timestamp, score)
    max_entries: usize,                          // 30 days * 288 samples/day
}

impl TrendTracker {
    /// Compare last 24h average to previous 24h
    pub fn compute_trend(&self) -> HealthTrend {
        let recent_avg = self.average_last(Duration::hours(24));
        let previous_avg = self.average_range(
            Duration::hours(48),
            Duration::hours(24),
        );
        let delta = recent_avg - previous_avg;

        if delta > 3.0 { HealthTrend::Improving }
        else if delta < -3.0 { HealthTrend::Degrading }
        else { HealthTrend::Stable }
    }

    /// Detect gradual degradation over weeks
    pub fn detect_entropy(&self) -> Option<EntropyAlert> {
        // Linear regression over 7-day window
        // If slope < -0.5 points/day, alert user
        // "Your system health has declined 4 points this week"
    }
}
```

### 6. Tray Integration

The tray icon dynamically shows the health score:

```rust
// Update tray tooltip with score
fn update_tray_tooltip(score: &HealthScore) {
    let tooltip = format!(
        "RuVector MemOpt - Health: {} ({})\n\
         Memory: {} | Thermal: {} | Disk: {}",
        score.total, score.grade,
        score.dimensions[0].score,
        score.dimensions[1].score,
        score.dimensions[2].score,
    );
    tray_icon.set_tooltip(&tooltip);
}

// Banner notification on significant change
if previous_score.total - current_score.total > 10 {
    show_banner(
        "Health Score Dropped",
        &format!("{} -> {} - {} needs attention",
            previous_score.total,
            current_score.total,
            worst_dimension.name),
        None,
    );
}
```

### 7. Dashboard Widget (Control Center Home Page)

```
┌──────────────────────────────────────────────┐
│         System Health Score                   │
│                                               │
│            ┌─────────┐                        │
│           /     87    \    Grade: A           │
│          │    /100     │   Trend: ▲ Improving │
│           \           /                       │
│            └─────────┘                        │
│                                               │
│  Memory ████████████████████░░  92            │
│  Thermal ██████████████████░░░  85            │
│  Disk    ████████████████░░░░░  78            │
│  Startup ██████████████████████  95            │
│  Bloat   ███████████████░░░░░░  73            │
│  Security████████████████████░░  90            │
│  Drivers █████████████████████░  88            │
│  Network ██████████████████████  96            │
│                                               │
│  Recommendations (3)                          │
│  ▸ Free 1.8 GB memory (+5 pts) [Fix Now]     │
│  ▸ Remove 2 startup items (+3 pts) [Fix Now]  │
│  ▸ Update GPU driver (+2 pts) [Details]       │
└──────────────────────────────────────────────┘
```

## Consequences

### Positive
- Single number makes system health tangible to non-technical users
- Trend tracking catches gradual degradation before it's noticeable
- Actionable recommendations provide clear next steps
- Gamification encourages users to maintain system health
- Score visible in tray provides constant awareness

### Negative
- Score weighting is subjective and may not match all users' priorities
- Some data sources (SMART, WMI thermal) may not be available on all hardware
- Computing score every 5 seconds adds ~2% CPU overhead
- Users may obsess over score optimization unnecessarily

### Security Considerations
- **Security dimension weight raised to 20%**: Security is the most consequential health dimension; a compromised system is unhealthy regardless of performance
- **Expanded security scoring**: Includes Defender status, firewall enabled, OS updates recency, unsigned startup items count, open RDP ports, guest account status
- **No arbitrary execution via recommendations**: `Action::OneClick` functions are a fixed set of known-safe operations (trim memory, disable startup item, clean temp), not arbitrary closures
- **Score history integrity**: Score time-series stored with HMAC to detect tampering (prevents a compromised process from inflating health scores)
- **Recommendation URL validation**: External links validated against allow-list of known vendor domains

### Risks
- Inaccurate scores erode trust (must be conservative with claims)
- Some recommendations require admin privileges
- Hardware without thermal sensors gets default 80/100 thermal score
- Security scoring clarifies it cannot replace dedicated security tools

## Implementation Plan

### Phase 1: Core Scorer
- [ ] Implement memory, thermal, disk sub-scores
- [ ] Weighted composite calculation
- [ ] Grade mapping

### Phase 2: Extended Dimensions
- [ ] Startup impact scoring
- [ ] Process bloat analysis (PageRank integration)
- [ ] Security posture checks
- [ ] Driver and network health

### Phase 3: Recommendations
- [ ] Recommendation generator per dimension
- [ ] One-click fix actions
- [ ] Impact estimation

### Phase 4: Trend & History
- [ ] Score persistence (SQLite or TOML log)
- [ ] 30-day trend tracking
- [ ] Entropy detection alerts

### Phase 5: UI
- [ ] Home page gauge widget
- [ ] Dimension bars with drill-down
- [ ] Recommendation list with actions
- [ ] Tray tooltip integration

## References

- [Windows Security Center WMI](https://learn.microsoft.com/en-us/windows/win32/secprov/security-center-wmi-providers)
- [SMART disk monitoring](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-diskdrive)
- [WMI Thermal Zone](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-thermalzonetemperature)
- [Process performance counters](https://learn.microsoft.com/en-us/windows/win32/perfctrs/performance-counters-portal)
