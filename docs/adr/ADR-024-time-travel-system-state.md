# ADR-024: Time-Travel System State

## Status
**Proposed**

## Date
2026-01-28

## Context

System state is ephemeral. When a system was "running perfectly yesterday" but is sluggish today, there's no way for a typical user to understand what changed. Windows System Restore captures a partial snapshot (registry, drivers, system files) but doesn't capture process configurations, memory layouts, service states, or performance characteristics.

RuVector can implement a lightweight "time-travel" system that continuously snapshots the system's operational state - not files, but the running configuration - allowing users to:
- Compare "now" vs "when it was fast"
- Identify exactly what changed (new process, service, driver, startup item)
- Rollback specific changes without a full system restore
- Create named checkpoints before risky operations

### Key Differentiator
Windows System Restore operates at the filesystem/registry level. RuVector Time-Travel operates at the **operational** level: what's running, how it's configured, and how it's performing. These are complementary, not competing.

### Existing Infrastructure
- Process monitoring with memory tracking (sysinfo)
- System Health Score (ADR-014) provides quantified state
- Startup item discovery (ADR-015)
- TOML configuration persistence
- Named pipe IPC

## Decision

### 1. System State Snapshot

```rust
pub struct SystemSnapshot {
    pub id: String,                        // UUID
    pub timestamp: DateTime<Local>,
    pub label: Option<String>,             // User-defined name
    pub trigger: SnapshotTrigger,
    pub health_score: u32,                 // 0-100 at time of snapshot

    // Process state
    pub processes: Vec<ProcessSnapshot>,
    pub services: Vec<ServiceSnapshot>,
    pub startup_items: Vec<StartupSnapshot>,

    // System configuration
    pub power_plan: String,
    pub virtual_memory_mb: u64,
    pub page_file_size_mb: u64,
    pub visual_effects: VisualEffectsState,

    // Performance baseline
    pub performance: PerformanceBaseline,

    // Hardware state
    pub memory_total_mb: u64,
    pub memory_available_mb: u64,
    pub disk_free_gb: HashMap<String, f64>,  // Drive letter -> free GB
    pub cpu_temp_c: Option<f64>,

    // Network
    pub network_adapters: Vec<NetworkAdapterState>,
    pub dns_servers: Vec<String>,
    pub proxy_settings: Option<ProxyConfig>,

    // Environment
    pub environment_variables: HashMap<String, String>,
    pub path_entries: Vec<String>,
    pub installed_updates: Vec<WindowsUpdate>,
}

pub struct ProcessSnapshot {
    pub pid: u32,
    pub name: String,
    pub path: Option<PathBuf>,
    pub memory_mb: f64,
    pub cpu_percent: f64,
    pub priority: u32,
    pub start_time: DateTime<Local>,
    pub command_line: Option<String>,
    pub parent_pid: Option<u32>,
}

pub struct ServiceSnapshot {
    pub name: String,
    pub display_name: String,
    pub status: ServiceStatus,
    pub start_type: ServiceStartType,
    pub memory_mb: f64,
    pub pid: Option<u32>,
}

pub struct StartupSnapshot {
    pub name: String,
    pub path: String,
    pub location: StartupLocation,
    pub enabled: bool,
}

pub struct PerformanceBaseline {
    pub boot_time_seconds: f64,
    pub idle_cpu_percent: f64,
    pub idle_memory_percent: f64,
    pub disk_read_mb_per_sec: f64,
    pub disk_write_mb_per_sec: f64,
    pub avg_process_count: u32,
    pub avg_handle_count: u64,
}

pub enum SnapshotTrigger {
    Scheduled,                   // Automatic periodic snapshot
    UserManual,                  // User clicked "Create Checkpoint"
    PreOperation(String),        // Before a RuVector operation
    HealthScoreDrop(u32, u32),   // From score X to Y
    SystemEvent(String),         // After Windows Update, driver install, etc.
    ProfileSwitch(String),       // Before workspace profile change
}
```

### 2. Snapshot Engine

```rust
pub struct TimeTravelEngine {
    snapshots: VecDeque<SystemSnapshot>,
    max_snapshots: usize,           // 168 = 7 days at hourly
    storage_path: PathBuf,          // ~/.ruvector/snapshots/
    auto_interval_minutes: u32,     // Default: 60
}

impl TimeTravelEngine {
    /// Take a full system snapshot
    pub fn snapshot(&mut self, trigger: SnapshotTrigger) -> SystemSnapshot {
        let snapshot = SystemSnapshot {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Local::now(),
            label: None,
            trigger,
            health_score: self.compute_health_score(),
            processes: self.capture_processes(),
            services: self.capture_services(),
            startup_items: self.capture_startup_items(),
            power_plan: self.get_power_plan(),
            virtual_memory_mb: self.get_virtual_memory(),
            page_file_size_mb: self.get_page_file_size(),
            visual_effects: self.get_visual_effects(),
            performance: self.measure_performance_baseline(),
            memory_total_mb: self.get_total_memory(),
            memory_available_mb: self.get_available_memory(),
            disk_free_gb: self.get_disk_free(),
            cpu_temp_c: self.get_cpu_temp(),
            network_adapters: self.get_network_state(),
            dns_servers: self.get_dns_servers(),
            proxy_settings: self.get_proxy(),
            environment_variables: self.get_env_vars(),
            path_entries: self.get_path_entries(),
            installed_updates: self.get_recent_updates(),
        };

        // Auto-compress old snapshots (keep only diffs after 24h)
        self.compress_old_snapshots();

        self.snapshots.push_back(snapshot.clone());
        self.persist_snapshot(&snapshot);
        snapshot
    }

    /// Label a snapshot for easy reference
    pub fn label(&mut self, snapshot_id: &str, label: &str) {
        if let Some(s) = self.snapshots.iter_mut().find(|s| s.id == snapshot_id) {
            s.label = Some(label.to_string());
        }
    }

    /// Find snapshot closest to a given time
    pub fn at(&self, target: DateTime<Local>) -> Option<&SystemSnapshot> {
        self.snapshots.iter()
            .min_by_key(|s| (s.timestamp - target).num_seconds().abs())
    }

    /// Find snapshot with highest health score in a time range
    pub fn best_in_range(
        &self,
        start: DateTime<Local>,
        end: DateTime<Local>,
    ) -> Option<&SystemSnapshot> {
        self.snapshots.iter()
            .filter(|s| s.timestamp >= start && s.timestamp <= end)
            .max_by_key(|s| s.health_score)
    }
}
```

### 3. Diff Engine

```rust
pub struct StateDiff {
    pub from: DateTime<Local>,
    pub to: DateTime<Local>,
    pub health_score_change: i32,         // e.g., -15

    pub processes_added: Vec<ProcessSnapshot>,
    pub processes_removed: Vec<ProcessSnapshot>,
    pub processes_memory_changed: Vec<MemoryChange>,

    pub services_started: Vec<ServiceSnapshot>,
    pub services_stopped: Vec<ServiceSnapshot>,
    pub services_start_type_changed: Vec<ServiceChange>,

    pub startup_items_added: Vec<StartupSnapshot>,
    pub startup_items_removed: Vec<StartupSnapshot>,

    pub updates_installed: Vec<WindowsUpdate>,
    pub env_vars_changed: Vec<EnvVarChange>,
    pub path_entries_changed: Vec<PathChange>,

    pub memory_available_change_mb: i64,
    pub disk_free_change_gb: HashMap<String, f64>,
}

pub struct MemoryChange {
    pub name: String,
    pub pid: u32,
    pub before_mb: f64,
    pub after_mb: f64,
    pub change_mb: f64,
    pub change_percent: f64,
}

pub struct ServiceChange {
    pub name: String,
    pub display_name: String,
    pub before: ServiceStartType,
    pub after: ServiceStartType,
}

impl TimeTravelEngine {
    /// Compare two snapshots and produce a human-readable diff
    pub fn diff(&self, from: &SystemSnapshot, to: &SystemSnapshot) -> StateDiff {
        let from_procs: HashSet<&str> = from.processes.iter()
            .map(|p| p.name.as_str()).collect();
        let to_procs: HashSet<&str> = to.processes.iter()
            .map(|p| p.name.as_str()).collect();

        StateDiff {
            from: from.timestamp,
            to: to.timestamp,
            health_score_change: to.health_score as i32 - from.health_score as i32,

            processes_added: to.processes.iter()
                .filter(|p| !from_procs.contains(p.name.as_str()))
                .cloned().collect(),
            processes_removed: from.processes.iter()
                .filter(|p| !to_procs.contains(p.name.as_str()))
                .cloned().collect(),
            processes_memory_changed: self.compute_memory_changes(from, to),

            services_started: self.diff_services_started(from, to),
            services_stopped: self.diff_services_stopped(from, to),
            services_start_type_changed: self.diff_service_types(from, to),

            startup_items_added: self.diff_startup_added(from, to),
            startup_items_removed: self.diff_startup_removed(from, to),

            updates_installed: self.diff_updates(from, to),
            env_vars_changed: self.diff_env_vars(from, to),
            path_entries_changed: self.diff_path(from, to),

            memory_available_change_mb:
                to.memory_available_mb as i64 - from.memory_available_mb as i64,
            disk_free_change_gb: self.diff_disk_free(from, to),
        }
    }

    /// Identify the most likely cause of performance degradation
    pub fn diagnose_degradation(
        &self,
        good: &SystemSnapshot,
        bad: &SystemSnapshot,
    ) -> Vec<DegradationCause> {
        let diff = self.diff(good, bad);
        let mut causes = vec![];

        // New processes consuming significant resources
        for proc in &diff.processes_added {
            if proc.memory_mb > 100.0 || proc.cpu_percent > 5.0 {
                causes.push(DegradationCause::NewProcess {
                    name: proc.name.clone(),
                    memory_mb: proc.memory_mb,
                    cpu_percent: proc.cpu_percent,
                    suggestion: format!("New process '{}' is using {:.0} MB RAM", proc.name, proc.memory_mb),
                });
            }
        }

        // Services that changed to auto-start
        for svc in &diff.services_start_type_changed {
            if matches!(svc.after, ServiceStartType::Automatic) {
                causes.push(DegradationCause::ServiceChanged {
                    name: svc.display_name.clone(),
                    change: format!("{:?} -> {:?}", svc.before, svc.after),
                    suggestion: format!("Service '{}' was changed to auto-start", svc.display_name),
                });
            }
        }

        // Windows Updates
        for update in &diff.updates_installed {
            causes.push(DegradationCause::UpdateInstalled {
                name: update.title.clone(),
                kb: update.kb_number.clone(),
                suggestion: format!("Update {} was installed", update.title),
            });
        }

        // New startup items
        for item in &diff.startup_items_added {
            causes.push(DegradationCause::NewStartupItem {
                name: item.name.clone(),
                path: item.path.clone(),
                suggestion: format!("'{}' was added to startup", item.name),
            });
        }

        // Sort by likely impact
        causes.sort_by(|a, b| b.impact_score().partial_cmp(&a.impact_score()).unwrap());
        causes
    }
}

pub enum DegradationCause {
    NewProcess { name: String, memory_mb: f64, cpu_percent: f64, suggestion: String },
    ServiceChanged { name: String, change: String, suggestion: String },
    UpdateInstalled { name: String, kb: String, suggestion: String },
    NewStartupItem { name: String, path: String, suggestion: String },
    MemoryLeak { name: String, growth_mb: f64, suggestion: String },
    DiskSpaceLow { drive: String, free_gb: f64, suggestion: String },
}
```

### 4. Rollback Engine

```rust
pub struct RollbackEngine {
    engine: TimeTravelEngine,
}

impl RollbackEngine {
    /// Rollback specific changes to match a previous snapshot
    pub fn rollback_to(
        &self,
        target: &SystemSnapshot,
        options: RollbackOptions,
    ) -> Vec<RollbackAction> {
        let current = self.engine.snapshot(SnapshotTrigger::PreOperation("rollback".into()));
        let diff = self.engine.diff(target, &current);
        let mut actions = vec![];

        if options.rollback_services {
            for svc in &diff.services_start_type_changed {
                actions.push(RollbackAction::RestoreServiceType {
                    name: svc.name.clone(),
                    target_type: svc.before.clone(),
                });
            }
        }

        if options.rollback_startup {
            for item in &diff.startup_items_added {
                actions.push(RollbackAction::RemoveStartupItem {
                    name: item.name.clone(),
                    location: item.location.clone(),
                });
            }
        }

        if options.kill_new_processes {
            for proc in &diff.processes_added {
                if !proc.is_system_critical() {
                    actions.push(RollbackAction::StopProcess {
                        name: proc.name.clone(),
                        pid: proc.pid,
                    });
                }
            }
        }

        if options.rollback_power_plan {
            if current.power_plan != target.power_plan {
                actions.push(RollbackAction::SetPowerPlan {
                    plan: target.power_plan.clone(),
                });
            }
        }

        actions
    }
}

pub struct RollbackOptions {
    pub rollback_services: bool,
    pub rollback_startup: bool,
    pub kill_new_processes: bool,
    pub rollback_power_plan: bool,
    pub rollback_visual_effects: bool,
    pub rollback_env_vars: bool,
}
```

### 5. UI Widget

```
+---------------------------------------------------------+
|  Time Travel                                             |
|                                                          |
|  Current: Score 72  |  Best Today: Score 91 (9:15 AM)  |
|                                                          |
|  Timeline (24 hours)                                     |
|  Score                                                   |
|  100 ┤                                                   |
|   80 ┤──────╮    ╭────────╮                              |
|   60 ┤      ╰────╯        ╰────── ● NOW (72)            |
|   40 ┤                                                   |
|       └──────────────────────────────                    |
|       9AM   12PM   3PM   6PM   9PM                       |
|                                                          |
|  Checkpoints                                             |
|  ● "Before Windows Update"  - Jan 27, Score 91          |
|  ● "Clean boot"             - Jan 25, Score 95          |
|  ○ Auto-snapshot             - 1 hour ago, Score 74     |
|                                                          |
|  [Compare to Best] [Create Checkpoint] [Diagnose Drop]  |
|                                                          |
|  Recent Changes (since best score):                      |
|  ▼ Score dropped 91 → 72 (-19 points)                   |
|    + SearchIndexer.exe added (+248 MB)                   |
|    + WindowsUpdate service → Automatic                   |
|    + KB5034441 installed                                 |
|    - 2.1 GB available memory lost                        |
|                                                          |
|  [Rollback Selected]  [Rollback All]  [Ignore]          |
+---------------------------------------------------------+
```

### 6. Configuration

```toml
[time_travel]
enabled = true
auto_snapshot_interval_minutes = 60
max_snapshots = 168                    # 7 days at hourly
storage_path = "~/.ruvector/snapshots"
compress_after_hours = 24              # Diff-only after 24h

[time_travel.triggers]
on_health_score_drop = 10              # Snapshot if score drops >10
on_windows_update = true
on_driver_install = true
on_profile_switch = true
before_optimization = true

[time_travel.snapshot]
capture_processes = true
capture_services = true
capture_startup = true
capture_env_vars = true
capture_network = true
capture_performance = true
```

## Consequences

### Positive
- Users can pinpoint exactly when and why their system slowed down
- Named checkpoints provide confidence before risky operations
- Automated diagnosis replaces guesswork with data
- Selective rollback avoids the heavy-handedness of System Restore
- Health score timeline provides long-term system health visibility
- Auto-trigger on health score drops catches problems immediately

### Negative
- Snapshot storage requires disk space (~5-20 MB per snapshot)
- Performance baseline measurement adds brief I/O during snapshot
- Some state changes are not easily reversible (driver installs)
- Full process enumeration takes 100-500ms per snapshot
- Environmental changes (Windows Update) may not be safely reversible

### Security Considerations
- **Snapshot encryption at rest**: All snapshot files encrypted with AES-256-GCM using DPAPI-derived key (Windows user-bound); prevents offline extraction of system state data
- **Sensitive data filtering**: Environment variables matching patterns (`*KEY*`, `*SECRET*`, `*TOKEN*`, `*PASSWORD*`) are redacted before snapshot storage
- **Rollback safety checks**: Before restoring a snapshot, verify it would not:
  - Disable Windows Defender or firewall
  - Re-enable a service known to be a security risk
  - Restore an older, vulnerable driver version
  - Revert a security update
  If any check fails, warn user and require explicit confirmation.
- **Snapshot integrity**: Each snapshot includes a SHA-256 digest; tampered snapshots are rejected on load
- **Access control**: Snapshot storage directory has ACL restricting access to current user + SYSTEM only
- **Automatic expiry**: Snapshots older than 30 days auto-deleted; no unlimited history accumulation
- **No credential capture**: Snapshot explicitly excludes: running process command-line arguments (may contain tokens), environment variable values for sensitive keys, browser profile data

### Risks
- Rolling back services could break dependent functionality
- Snapshot data contains process names and paths (mitigated: encryption + sensitive data filtering)
- Very frequent snapshots could impact low-end disk performance
- Users may expect "full" undo capability beyond what's possible
- Some degradations have external causes (failing hardware) that can't be rolled back
- Rollback could restore a less-secure state (mitigated: security rollback checks)

## Implementation Plan

### Phase 1: Snapshot Engine
- [ ] Process, service, and startup enumeration
- [ ] Performance baseline measurement
- [ ] Snapshot persistence (TOML or bincode serialization)
- [ ] Auto-snapshot on timer

### Phase 2: Diff Engine
- [ ] State comparison algorithm
- [ ] Memory change tracking
- [ ] Service state change detection
- [ ] Human-readable diff output

### Phase 3: Diagnosis
- [ ] Degradation cause identification
- [ ] Impact scoring for each cause
- [ ] Health score correlation
- [ ] Windows Update detection

### Phase 4: Rollback
- [ ] Service start type restoration
- [ ] Startup item removal
- [ ] Power plan restoration
- [ ] Selective rollback UI

### Phase 5: UI
- [ ] Health score timeline chart
- [ ] Checkpoint management
- [ ] Comparison view (side-by-side)
- [ ] Rollback confirmation dialog

## References

- [Windows Management Instrumentation (WMI)](https://learn.microsoft.com/en-us/windows/win32/wmisdk/wmi-start-page)
- [System Restore API](https://learn.microsoft.com/en-us/windows/win32/sr/system-restore-reference)
- [Windows Update API](https://learn.microsoft.com/en-us/windows/win32/wua_sdk/windows-update-agent--wua--api-reference)
- [Service Control Manager](https://learn.microsoft.com/en-us/windows/win32/services/service-control-manager)
