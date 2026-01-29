# ADR-013: Workspace Profiles & Context Switching

## Status
**Proposed**

## Date
2026-01-28

## Context

Users perform fundamentally different tasks on the same machine: development, gaming, video editing, video calls, web browsing, AI inference. Each workload has different resource requirements:

| Workload | CPU Priority | Memory | GPU | Network | Storage |
|----------|-------------|--------|-----|---------|---------|
| Development | Medium (foreground), High (builds) | High (IDE, Docker, WSL2) | Low | Medium | High (builds) |
| Gaming | High (game process) | Medium | High | Low-Medium | Medium |
| Video Editing | High | Very High | High (decode/encode) | Low | Very High |
| Video Calls | Medium | Low-Medium | Medium (encode) | High | Low |
| AI Inference | High | Very High | Very High (VRAM) | Low | High (model loading) |

Currently, users must manually adjust Windows power plans, close competing apps, tweak settings. RuVector can automate this entirely with named workspace profiles that reconfigure the entire system in one action.

### Requirements
- Named profiles with per-profile configuration
- One-click or hotkey switching
- Auto-detection of context (optional, learned over time)
- Per-profile: process priority rules, suppressed apps, power plan, memory thresholds, GPU allocation
- Profile inheritance (base + overrides)
- Import/export for sharing profiles

## Decision

### 1. Profile Data Model

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct WorkspaceProfile {
    pub name: String,
    pub icon: String,                    // Emoji or icon identifier
    pub description: String,
    pub inherits: Option<String>,        // Base profile name

    // Process management
    pub priority_processes: Vec<ProcessRule>,
    pub suppress_processes: Vec<String>,  // Kill or minimize on activation
    pub launch_processes: Vec<LaunchRule>, // Start on activation

    // System configuration
    pub power_plan: PowerPlan,
    pub memory_threshold: u32,           // Auto-optimize trigger (%)
    pub optimization_aggressiveness: AggressivenessLevel,

    // GPU
    pub gpu_priority_app: Option<String>, // Gets GPU priority
    pub vram_reservation_mb: Option<u64>, // Reserve VRAM

    // Network
    pub bandwidth_rules: Vec<BandwidthRule>,

    // Services
    pub disable_services: Vec<String>,    // Windows services to stop
    pub enable_services: Vec<String>,     // Windows services to start
    pub disable_indexing: bool,           // Windows Search indexing
    // SECURITY: disable_defender_realtime removed.
    // Disabling the primary AV engine is too dangerous even temporarily.
    // Users who want this must do it manually outside RuVector.

    // Display
    pub refresh_rate: Option<u32>,        // Hz, None = don't change
    pub resolution: Option<(u32, u32)>,   // None = don't change

    // Focus
    pub focus_mode: bool,                 // Enable focus mode (ADR-005)
    pub notification_policy: NotificationPolicy,

    // Schedule
    pub auto_activate: Option<ActivationTrigger>,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum ProcessRule {
    SetPriority { name: String, priority: ProcessPriority },
    SetAffinity { name: String, cores: Vec<usize> },       // Pin to specific cores
    SetMemoryLimit { name: String, max_mb: u64 },          // Job object limit
}

#[derive(Serialize, Deserialize, Clone)]
pub enum ActivationTrigger {
    TimeRange { start: String, end: String, days: Vec<String> },
    ProcessLaunched(String),     // e.g., "steam.exe" triggers Gaming profile
    NetworkConnected(String),    // e.g., "CorporateWiFi" triggers Work profile
    DisplayConnected(u32),       // External monitor count
    Manual,                      // Only via hotkey/UI
}

#[derive(Serialize, Deserialize, Clone)]
pub enum PowerPlan {
    Balanced,
    HighPerformance,
    Ultimate,
    PowerSaver,
    Custom(String),  // GUID of custom plan
}
```

### 2. Built-in Profiles (Shipping Defaults)

```toml
[profiles.development]
icon = "D"
description = "Optimized for software development"
priority_processes = [
    { name = "code.exe", priority = "AboveNormal" },
    { name = "cargo.exe", priority = "High" },
    { name = "node.exe", priority = "AboveNormal" },
    { name = "rust-analyzer.exe", priority = "AboveNormal" },
]
suppress_processes = ["OneDrive.exe", "YourPhone.exe", "GameBar.exe"]
power_plan = "HighPerformance"
memory_threshold = 80
disable_indexing = true
focus_mode = true
notification_policy = "minimal"

[profiles.gaming]
icon = "G"
description = "Maximum performance for gaming"
suppress_processes = [
    "code.exe", "OneDrive.exe", "Teams.exe", "Slack.exe",
    "Discord.exe", "Spotify.exe", "node.exe",
]
power_plan = "Ultimate"
memory_threshold = 90
optimization_aggressiveness = "aggressive"
disable_indexing = true
# disable_defender_realtime removed for security - see Security Considerations
refresh_rate = 165
focus_mode = true
notification_policy = "none"
auto_activate = { ProcessLaunched = "steam.exe" }

[profiles.video_call]
icon = "V"
description = "Prioritize audio/video for calls"
priority_processes = [
    { name = "Teams.exe", priority = "High" },
    { name = "zoom.exe", priority = "High" },
    { name = "slack.exe", priority = "AboveNormal" },
]
suppress_processes = ["OneDrive.exe", "WindowsUpdate"]
bandwidth_rules = [
    { name = "Teams.exe", priority = "critical" },
    { name = "zoom.exe", priority = "critical" },
    { name = "*", priority = "low" },
]
notification_policy = "urgent_only"

[profiles.ai_inference]
icon = "A"
description = "Optimized for local AI model inference"
priority_processes = [
    { name = "ollama.exe", priority = "High" },
    { name = "python.exe", priority = "AboveNormal" },
]
suppress_processes = ["OneDrive.exe", "Teams.exe", "GameBar.exe"]
power_plan = "HighPerformance"
memory_threshold = 70
vram_reservation_mb = 0  # All available
optimization_aggressiveness = "aggressive"
disable_indexing = true
```

### 3. Profile Switching Engine

```rust
pub struct ProfileEngine {
    active_profile: Option<String>,
    profiles: HashMap<String, WorkspaceProfile>,
    undo_stack: Vec<SystemSnapshot>,  // For rollback on deactivate
}

impl ProfileEngine {
    /// Activate a profile, saving current state for rollback
    pub async fn activate(&mut self, name: &str) -> Result<ActivationReport, String> {
        // 1. Snapshot current state (for undo)
        let snapshot = SystemSnapshot::capture().await?;
        self.undo_stack.push(snapshot);

        let profile = self.profiles.get(name).ok_or("Profile not found")?;

        // 2. Apply profile in order
        let mut report = ActivationReport::new(name);

        // 2a. Stop suppressed processes
        for proc in &profile.suppress_processes {
            report.add(self.suppress_process(proc).await);
        }

        // 2b. Set process priorities
        for rule in &profile.priority_processes {
            report.add(self.apply_process_rule(rule).await);
        }

        // 2c. Set power plan
        report.add(self.set_power_plan(&profile.power_plan).await);

        // 2d. Disable services
        for service in &profile.disable_services {
            report.add(self.stop_service(service).await);
        }

        // 2e. Configure GPU
        if let Some(ref app) = profile.gpu_priority_app {
            report.add(self.set_gpu_priority(app).await);
        }

        // 2f. Memory optimization
        if profile.optimization_aggressiveness != AggressivenessLevel::None {
            report.add(self.optimize_memory(profile.memory_threshold).await);
        }

        // 2g. Launch apps
        for launch in &profile.launch_processes {
            report.add(self.launch_process(launch).await);
        }

        // 2h. Focus mode
        if profile.focus_mode {
            report.add(self.enable_focus_mode(&profile.notification_policy).await);
        }

        self.active_profile = Some(name.to_string());
        Ok(report)
    }

    /// Deactivate profile and restore previous state
    pub async fn deactivate(&mut self) -> Result<(), String> {
        if let Some(snapshot) = self.undo_stack.pop() {
            snapshot.restore().await?;
        }
        self.active_profile = None;
        Ok(())
    }
}
```

### 4. Auto-Detection (Neural Engine Integration)

The neural engine (existing `NeuralDecisionEngine`) learns which profile matches current activity:

```rust
pub struct ProfilePredictor {
    engine: NeuralDecisionEngine,
    observation_window: Duration, // 5 minutes
}

impl ProfilePredictor {
    /// Observe current system state and suggest a profile
    pub fn suggest(&self, state: &SystemState) -> Option<(String, f32)> {
        let features = vec![
            state.foreground_app_category as f32,
            state.cpu_usage_percent,
            state.gpu_usage_percent,
            state.network_usage_mbps,
            state.active_process_count as f32,
            state.time_of_day_normalized(),
            state.day_of_week_normalized(),
        ];
        // Returns (profile_name, confidence)
        // Only auto-switch if confidence > 0.85
    }
}
```

### 5. Hotkey System

```rust
pub struct HotkeyManager {
    bindings: HashMap<Hotkey, String>, // Hotkey -> profile name
}

// Default bindings (configurable)
// Ctrl+Alt+1 = Development
// Ctrl+Alt+2 = Gaming
// Ctrl+Alt+3 = Video Call
// Ctrl+Alt+4 = AI Inference
// Ctrl+Alt+0 = Deactivate (restore defaults)
```

### 6. Profile Transition Animation

When switching profiles, the macOS-style banner (ADR-012/dialog.rs) shows:

```
┌─────────────────────────────────────────┐
│ ▎ Switching to Development              │
│ ▎ Stopped 3 apps · Boosted 4 · 342MB freed │
└─────────────────────────────────────────┘
```

## Consequences

### Positive
- One-click system reconfiguration saves minutes per context switch
- Neural auto-detection eliminates even the one click
- Undo/rollback prevents permanent system changes
- Sharable profiles build community
- Measurable performance improvement per workload

### Negative
- Killing processes may lose unsaved work (mitigated by suppress = minimize first, ask before kill)
- Power plan changes require admin on some configurations
- Service control requires elevated privileges
- Auto-detection needs training period (1-2 weeks of usage)

### Security Considerations
- **No AV disable**: `disable_defender_realtime` removed - RuVector must never disable the primary security tool
- **Profile import validation**: Imported profiles are schema-validated and sanitized; `suppress_processes` cannot target security software (`MsMpEng.exe`, `MBAMService.exe`, etc.)
- **Service control allow-list**: Only known-safe services can be stopped; critical services (`wuauserv`, `WinDefend`, `mpssvc`) are protected
- **Process name + path verification**: Process rules match on full path + Authenticode publisher, not name alone (prevents `cargo.exe` in a malicious directory)
- **Profile signing**: Shared profiles include an HMAC signature; tampered profiles warn the user before activation
- **Privilege escalation audit**: All admin-required operations logged to Windows Event Log
- **Corporate detection**: On domain-joined machines, profiles cannot modify Group Policy-controlled settings

### Risks
- Aggressive profiles could suppress important processes (mitigated: security software allow-list)
- Process name matching may hit wrong processes (mitigated: full path + Authenticode publisher checks)
- Multiple profile activations without deactivation could stack side effects

## Implementation Plan

### Phase 1: Core Engine
- [ ] Profile data model and TOML serialization
- [ ] Profile switching engine with undo/rollback
- [ ] Built-in default profiles
- [ ] CLI: `ruvector-memopt profile activate <name>`

### Phase 2: Process Management
- [ ] Process priority adjustment via `SetPriorityClass`
- [ ] Process affinity via `SetProcessAffinityMask`
- [ ] Graceful process suppression (minimize first, then terminate)
- [ ] Process launch with environment configuration

### Phase 3: System Integration
- [ ] Power plan switching via `powercfg`
- [ ] Windows Search indexing control
- [ ] Service start/stop via Service Control Manager
- [ ] GPU priority via NVIDIA/AMD APIs

### Phase 4: Intelligence
- [ ] Neural profile suggestion
- [ ] Auto-activation triggers (process launch, network, time)
- [ ] Training data collection
- [ ] Confidence threshold tuning

### Phase 5: UI Integration
- [ ] Profile page in Control Center (ADR-012)
- [ ] Tray menu profile switcher
- [ ] Global hotkey registration
- [ ] Profile editor (create/edit/delete/import/export)

## References

- [SetPriorityClass](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setpriorityclass)
- [SetProcessAffinityMask](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-setprocessaffinitymask)
- [powercfg](https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/powercfg-command-line-options)
- [Windows Focus Assist API](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setprocessdpiawarenesscontext)
