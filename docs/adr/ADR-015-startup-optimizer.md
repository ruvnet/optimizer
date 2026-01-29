# ADR-015: Startup Optimizer with PageRank Scoring

## Status
**Proposed**

## Date
2026-01-28

## Context

Windows startup is a common pain point. Users accumulate startup items over time from application installations, and most have no visibility into their actual impact. Windows Task Manager shows a basic "Startup Impact" (High/Medium/Low) but provides no intelligence about which items are truly needed vs. which can be safely deferred or disabled.

RuVector already has a PageRank-based process scoring system (`src/algorithms/pagerank.rs`) that evaluates process importance based on inter-process dependencies. This same algorithm can score startup items by how critical they are to the user's actual workflow, creating a "smart startup" that boots only what matters and defers the rest.

### Requirements
- Enumerate all startup items (Registry, Task Scheduler, Startup folder, Services)
- Measure actual boot time impact per item (boot tracing)
- Score importance using PageRank + user behavior learning
- Provide one-click: disable, defer (delay 60s), or keep
- Show total boot time savings estimate
- Support "staggered startup" - launch items over 2 minutes instead of all at once
- Reversible - re-enable anything with one click

## Decision

### 1. Startup Item Discovery

```rust
pub struct StartupItem {
    pub name: String,
    pub publisher: String,
    pub command: String,
    pub location: StartupLocation,
    pub enabled: bool,
    pub impact: BootImpact,           // Measured or estimated
    pub pagerank_score: f64,          // 0.0-1.0 importance
    pub user_dependency: f64,         // How often user interacts within 10min of boot
    pub recommendation: StartupAction,
}

pub enum StartupLocation {
    RegistryCurrentUser(String),      // HKCU\Software\Microsoft\Windows\CurrentVersion\Run
    RegistryLocalMachine(String),     // HKLM\...\Run
    TaskScheduler(String),            // Task Scheduler path
    StartupFolder(PathBuf),           // Shell:startup
    Service(String),                  // Windows Service (auto-start)
    ScheduledTask(String),            // Scheduled tasks triggered at logon
}

pub struct BootImpact {
    pub cpu_seconds: f64,             // CPU time consumed in first 60s
    pub disk_mb_read: f64,            // Disk I/O in first 60s
    pub memory_mb: f64,               // Peak memory in first 60s
    pub estimated_delay_seconds: f64, // Contribution to boot time
    pub measurement_confidence: f64,  // 0-1 based on sample count
}
```

### 2. Discovery Sources

| Source | API | Items Found |
|--------|-----|-------------|
| `HKCU\...\Run` | `RegEnumValueW` | Per-user startup apps |
| `HKLM\...\Run` | `RegEnumValueW` | Machine-wide startup apps |
| `HKCU\...\RunOnce` | `RegEnumValueW` | One-time tasks |
| Shell:startup folder | `SHGetFolderPath` + `ReadDir` | Shortcut-based items |
| Task Scheduler | `ITaskService` COM | Scheduled logon tasks |
| Services (Auto) | `EnumServicesStatus` | Auto-start services |
| WMI `Win32_StartupCommand` | WMI query | Aggregate view |

### 3. PageRank-Based Importance Scoring

Extend the existing `ProcessPageRank` to score startup items:

```rust
pub struct StartupRanker {
    pagerank: ProcessPageRank,
    usage_tracker: UsageTracker,
}

impl StartupRanker {
    pub fn score(&self, item: &StartupItem) -> f64 {
        let mut score = 0.0;

        // Factor 1: Process PageRank (dependency importance)
        // High rank = many other processes depend on this
        score += self.pagerank.get_score_by_name(&item.name) * 0.3;

        // Factor 2: User interaction frequency
        // How soon after boot does the user interact with this app?
        score += self.usage_tracker.interaction_score(&item.name) * 0.3;

        // Factor 3: System criticality
        // Is this a Windows component, driver, or security tool?
        score += self.system_criticality(&item) * 0.25;

        // Factor 4: Resource impact (inverse - heavy items score lower)
        // High boot impact = more reason to defer
        score -= self.normalized_impact(&item.impact) * 0.15;

        score.max(0.0).min(1.0)
    }

    fn system_criticality(&self, item: &StartupItem) -> f64 {
        // SECURITY: Verify publisher via Windows Authenticode signature,
        // NOT by string matching (which is trivially spoofable).
        // WinVerifyTrust + CryptQueryObject extracts the real publisher.
        let verified_publisher = self.verify_authenticode(&item.command);

        match verified_publisher.as_deref() {
            Some("Microsoft Corporation") => 0.7,
            Some("Microsoft Windows") => 0.9,  // Core OS component
            Some(p) if p.contains("NVIDIA") || p.contains("AMD") || p.contains("Intel") => 0.6,
            Some(p) if self.is_security_vendor(p) => 0.95,  // Never disable security
            None => 0.2,  // Unsigned = suspicious, low trust
            _ => 0.3,
        }
        // Additionally check if it's a service dependency
    }

    fn is_security_vendor(&self, publisher: &str) -> bool {
        const SECURITY_PUBLISHERS: &[&str] = &[
            "Malwarebytes", "Norton", "Kaspersky", "Bitdefender",
            "ESET", "Avast", "CrowdStrike", "SentinelOne",
            "Microsoft Windows",  // Defender
        ];
        SECURITY_PUBLISHERS.iter().any(|v| publisher.contains(v))
    }
}
```

### 4. Recommendation Engine

```rust
pub enum StartupAction {
    Keep,                      // Critical, don't touch
    Defer { delay_secs: u32 }, // Delay startup by N seconds
    Disable,                   // Safe to disable
    Remove,                    // Bloatware, should be uninstalled
}

impl StartupOptimizer {
    pub fn recommend(&self, item: &StartupItem) -> StartupAction {
        let score = self.ranker.score(item);

        if score > 0.8 {
            StartupAction::Keep
        } else if score > 0.5 {
            // Important but not critical - defer
            let delay = ((1.0 - score) * 120.0) as u32; // 0-60s delay
            StartupAction::Defer { delay_secs: delay }
        } else if score > 0.2 {
            StartupAction::Disable
        } else {
            // Very low importance, known bloatware pattern
            StartupAction::Remove
        }
    }

    /// Estimate total boot time savings if all recommendations applied
    pub fn estimated_savings(&self) -> BootSavings {
        let items = self.discover_all();
        let deferred: f64 = items.iter()
            .filter(|i| matches!(self.recommend(i), StartupAction::Defer { .. }))
            .map(|i| i.impact.estimated_delay_seconds)
            .sum();
        let disabled: f64 = items.iter()
            .filter(|i| matches!(self.recommend(i), StartupAction::Disable | StartupAction::Remove))
            .map(|i| i.impact.estimated_delay_seconds)
            .sum();

        BootSavings {
            deferred_seconds: deferred,
            disabled_seconds: disabled,
            total_seconds: deferred + disabled,
            freed_memory_mb: /* sum of disabled items' memory */,
        }
    }
}
```

### 5. Staggered Startup

Instead of launching everything at boot, distribute startup items across a 2-minute window:

```rust
pub struct StaggeredStartup {
    tiers: Vec<StartupTier>,
}

pub struct StartupTier {
    pub delay_seconds: u32,
    pub items: Vec<StartupItem>,
}

// Tier 0 (immediate): Critical system components, security
// Tier 1 (15s delay): Important user apps (IDE, browser)
// Tier 2 (45s delay): Background services (cloud sync, updaters)
// Tier 3 (90s delay): Nice-to-have (telemetry, analytics, tips)
// Tier 4 (disabled): Known bloatware

impl StaggeredStartup {
    /// Install staggered startup via Task Scheduler
    /// Disables original startup entry, creates delayed task
    pub fn install(&self) -> Result<(), String> {
        for tier in &self.tiers {
            for item in &tier.items {
                if tier.delay_seconds > 0 {
                    // 1. Disable original startup entry
                    self.disable_original(item)?;
                    // 2. Create Task Scheduler task with delay
                    self.create_delayed_task(item, tier.delay_seconds)?;
                }
            }
        }
        Ok(())
    }

    /// Uninstall staggered startup (restore original entries)
    pub fn uninstall(&self) -> Result<(), String> {
        // Reverse all changes using saved original state
    }
}
```

### 6. Boot Time Measurement

```rust
pub struct BootTracer {
    // Uses Windows Event Log: Microsoft-Windows-Diagnostics-Performance/Operational
    // Event ID 100 = Boot duration
    // Event ID 101-110 = Per-component boot times
}

impl BootTracer {
    pub fn last_boot_time(&self) -> Result<Duration, String> {
        // Query Event Log for most recent Event ID 100
        // Returns total boot duration
    }

    pub fn boot_history(&self, count: usize) -> Vec<BootRecord> {
        // Return last N boot times for trend analysis
    }

    pub fn per_item_impact(&self) -> HashMap<String, BootImpact> {
        // Cross-reference ETW boot trace with process creation times
        // Measure CPU/disk/memory per startup item during first 60s
    }
}
```

### 7. UI (Control Center Page)

```
┌──────────────────────────────────────────────────────────────┐
│  Startup Optimizer                                            │
│                                                               │
│  Boot Time: 34s (was 52s before optimization)                │
│  Estimated Savings: 18s | Memory Saved: 412 MB               │
│  Startup Items: 23 total (8 keep, 6 deferred, 7 disabled, 2 removed) │
│                                                               │
│  [Apply All Recommendations]  [Stagger Boot]  [Undo All]    │
│                                                               │
│  ┌─────┬──────────────────┬───────────┬────────┬───────────┐ │
│  │Score│ Name             │ Impact    │ Status │ Action    │ │
│  ├─────┼──────────────────┼───────────┼────────┼───────────┤ │
│  │ 0.95│ Windows Defender  │ Low       │ Keep   │ [Keep]    │ │
│  │ 0.88│ NVIDIA Driver     │ Medium    │ Keep   │ [Keep]    │ │
│  │ 0.72│ VS Code           │ Medium    │ Keep   │ [Defer]   │ │
│  │ 0.61│ Slack             │ High      │ Defer  │ [Disable] │ │
│  │ 0.45│ OneDrive          │ High      │ Defer  │ [Disable] │ │
│  │ 0.32│ Adobe Updater     │ Medium    │ Disabled│ [Enable] │ │
│  │ 0.18│ Cortana           │ Low       │ Disabled│ [Enable] │ │
│  │ 0.05│ HP Telemetry      │ Low       │ Removed│ [Restore] │ │
│  └─────┴──────────────────┴───────────┴────────┴───────────┘ │
│                                                               │
│  Boot Time History (last 7 boots)                            │
│  52s ┤ █                                                     │
│  45s ┤ █ █                                                   │
│  38s ┤ █ █ █                                                 │
│  34s ┤ █ █ █ █ █ █ █                                         │
│      └─┴─┴─┴─┴─┴─┴─┘                                        │
└──────────────────────────────────────────────────────────────┘
```

## Consequences

### Positive
- Measurable boot time improvement (typical 30-50% reduction)
- Memory savings from disabled unnecessary items
- PageRank scoring is more intelligent than Windows' simple High/Medium/Low
- Staggered startup reduces boot-time resource contention
- All changes are reversible

### Negative
- Some startup items are interdependent (disabling A may break B)
- Boot impact measurement requires multiple boots for accuracy
- Task Scheduler manipulation requires admin privileges
- OEM-specific items may have vendor-specific uninstall requirements

### Security Considerations
- **Authenticode verification**: Publisher identity validated via `WinVerifyTrust`, not string matching (spoofable)
- **Security software protection**: Items from verified security vendors (Defender, CrowdStrike, etc.) always scored >= 0.95 and cannot be disabled
- **Unsigned binary warning**: Startup items with no valid Authenticode signature flagged with low trust score and visual warning
- **Group Policy detection**: On domain-joined machines, GP-managed startup items are read-only
- **Audit trail**: All startup modifications logged to persistent undo log with timestamps
- **Tamper detection**: Undo log integrity verified with HMAC; alerts if modified externally

### Risks
- Some games require launchers to be running (Steam, Epic)
- Enterprise-managed startup items should not be modified (mitigated: Group Policy detection)

## Implementation Plan

### Phase 1: Discovery
- [ ] Enumerate all startup sources (Registry, Task Scheduler, Startup folder, Services)
- [ ] Parse and normalize startup items
- [ ] Display in CLI: `ruvector-memopt startup list`

### Phase 2: Scoring
- [ ] Integrate PageRank scoring for startup items
- [ ] Implement system criticality detection
- [ ] Add user interaction tracking (foreground time within 10min of boot)

### Phase 3: Optimization
- [ ] Disable/enable startup items via Registry
- [ ] Create staggered startup via Task Scheduler
- [ ] Measure boot time from Event Log

### Phase 4: UI
- [ ] Startup page in Control Center
- [ ] Boot time history chart
- [ ] One-click optimize and undo

## References

- [Windows Boot Performance Diagnostics](https://learn.microsoft.com/en-us/windows/client-management/boot-performance-diagnostics)
- [Task Scheduler COM API](https://learn.microsoft.com/en-us/windows/win32/taskschd/task-scheduler-start-page)
- [RegEnumValueW](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regenumvaluew)
- [Windows Performance Recorder](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/windows-performance-recorder)
