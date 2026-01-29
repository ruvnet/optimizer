# ADR-020: Thermal-Aware Process Scheduler

## Status
**Proposed**

## Date
2026-01-28

## Context

Modern CPUs throttle when they exceed thermal limits (TjMax). Windows responds reactively - after throttling begins. By then, the user experiences frame drops, build slowdowns, or audio glitches. RuVector can monitor thermal trajectory and proactively reduce non-critical workloads before throttling occurs, maintaining peak performance for the foreground application.

Additionally, fan noise is a significant quality-of-life issue. Laptops and small desktops become loud under sustained load. A "silent mode" that trades some background performance for silence is highly desirable.

### Data Sources
- **CPU temperature**: WMI `MSAcpi_ThermalZoneTemperature`, Intel PCH sensors, AMD SMU
- **GPU temperature**: NVML (`nvml-wrapper` already in Cargo.toml), AMD ADL, Intel IGCL
- **Fan RPM**: WMI `Win32_Fan`, vendor-specific (Dell: `BIOS_Fan`, Lenovo: `IMC`)
- **Power draw**: Intel RAPL MSRs, battery API `GetSystemPowerStatus`
- **Throttle status**: `IsProcessorFeaturePresent`, HWiNFO shared memory

## Decision

### 1. Thermal Monitor

```rust
pub struct ThermalMonitor {
    cpu_sensors: Vec<ThermalSensor>,
    gpu_sensors: Vec<ThermalSensor>,
    fans: Vec<FanSensor>,
    history: VecDeque<ThermalSnapshot>,
    tjmax: f64,                        // CPU thermal limit (typically 100°C)
    throttle_margin: f64,              // Start acting at TjMax - margin (default 15°C)
}

pub struct ThermalSnapshot {
    pub timestamp: Instant,
    pub cpu_temp_c: f64,
    pub gpu_temp_c: f64,
    pub cpu_package_watts: f64,
    pub fan_rpm: Vec<u32>,
    pub is_throttling: bool,
    pub predicted_throttle_secs: Option<f64>,
}

impl ThermalMonitor {
    /// Predict seconds until thermal throttling based on current trajectory
    pub fn predict_throttle_time(&self) -> Option<f64> {
        if self.history.len() < 6 { return None; }  // Need 30s of data

        let temps: Vec<f64> = self.history.iter()
            .rev().take(12)  // Last 60 seconds
            .map(|s| s.cpu_temp_c)
            .collect();

        // Linear regression on temperature trend
        let (slope_per_sec, _) = linear_regression_time(&temps, 5.0); // 5s samples

        if slope_per_sec <= 0.0 {
            return None;  // Cooling or stable
        }

        let current = temps[0];
        let headroom = self.tjmax - self.throttle_margin - current;

        if headroom <= 0.0 {
            Some(0.0)  // Already in danger zone
        } else {
            Some(headroom / slope_per_sec)
        }
    }
}
```

### 2. Proactive Throttling Engine

```rust
pub struct ThermalScheduler {
    monitor: ThermalMonitor,
    mode: ThermalMode,
    original_affinities: HashMap<u32, u64>,
    throttled_processes: Vec<u32>,
}

pub enum ThermalMode {
    /// Maximum performance, accept fan noise and heat
    Performance,
    /// Balanced - throttle non-critical when approaching limit
    Balanced {
        throttle_margin_c: f64,  // Start throttling at TjMax - margin
    },
    /// Silent - keep fans below threshold RPM
    Silent {
        max_fan_rpm: u32,        // Target: fans below this RPM
        max_cpu_temp_c: f64,     // Cap temp to keep fans quiet
    },
    /// Custom - user-defined thresholds
    Custom {
        cpu_target_c: f64,
        gpu_target_c: f64,
        max_fan_rpm: Option<u32>,
    },
}

impl ThermalScheduler {
    pub fn tick(&mut self) -> Vec<ThermalAction> {
        let snapshot = self.monitor.sample();
        let mut actions = vec![];

        match &self.mode {
            ThermalMode::Balanced { throttle_margin_c } => {
                let danger_temp = self.monitor.tjmax - throttle_margin_c;

                if snapshot.cpu_temp_c > danger_temp {
                    // Approaching throttle - reduce background load
                    actions.extend(self.throttle_background_processes());
                } else if snapshot.cpu_temp_c < danger_temp - 10.0 {
                    // Safe zone - restore throttled processes
                    actions.extend(self.restore_throttled_processes());
                }
            }

            ThermalMode::Silent { max_fan_rpm, max_cpu_temp_c } => {
                let current_rpm = snapshot.fan_rpm.iter().max().unwrap_or(&0);

                if *current_rpm > *max_fan_rpm || snapshot.cpu_temp_c > *max_cpu_temp_c {
                    // Too loud or too hot - progressive throttling
                    let severity = self.compute_throttle_severity(
                        *current_rpm, *max_fan_rpm,
                        snapshot.cpu_temp_c, *max_cpu_temp_c,
                    );
                    actions.extend(self.apply_throttle(severity));
                } else if *current_rpm < max_fan_rpm - 200 {
                    // Quiet enough - can release some throttling
                    actions.extend(self.release_throttle_step());
                }
            }
            _ => {}
        }

        actions
    }

    fn throttle_background_processes(&mut self) -> Vec<ThermalAction> {
        let mut actions = vec![];

        // Use PageRank to identify lowest-importance background processes
        let candidates = self.get_background_processes_by_cpu_usage();

        for proc in candidates {
            if proc.is_foreground { continue; }  // Never throttle foreground

            // Save original affinity
            self.original_affinities.entry(proc.pid)
                .or_insert(proc.affinity_mask);

            // Restrict to efficiency cores (or half the cores)
            let throttled_mask = self.get_efficiency_core_mask();
            actions.push(ThermalAction::SetAffinity {
                pid: proc.pid,
                name: proc.name.clone(),
                mask: throttled_mask,
            });

            // Lower priority
            actions.push(ThermalAction::SetPriority {
                pid: proc.pid,
                name: proc.name.clone(),
                priority: ProcessPriority::BelowNormal,
            });

            self.throttled_processes.push(proc.pid);
        }

        actions
    }
}
```

### 3. Silent Mode

```rust
pub struct SilentMode {
    target_noise_level: NoiseLevel,
    fan_controller: Option<FanController>,
}

pub enum NoiseLevel {
    Silent,     // <1000 RPM - whisper quiet
    Quiet,      // <1500 RPM - barely audible
    Normal,     // <2500 RPM - standard operation
    Unlimited,  // No fan limits
}

impl SilentMode {
    /// Compute maximum allowed CPU power to keep fans quiet
    pub fn compute_power_budget(&self, ambient_temp_c: f64) -> PowerBudget {
        // Thermal model: P_dissipated = (T_cpu - T_ambient) / R_thermal
        // Where R_thermal depends on fan speed
        // Lower fan speed -> higher thermal resistance -> lower power budget

        match self.target_noise_level {
            NoiseLevel::Silent => PowerBudget {
                cpu_tdp_watts: 15.0,    // Limit to 15W (passively cooled)
                gpu_tdp_watts: 25.0,
            },
            NoiseLevel::Quiet => PowerBudget {
                cpu_tdp_watts: 35.0,    // Limit to 35W
                gpu_tdp_watts: 75.0,
            },
            NoiseLevel::Normal => PowerBudget {
                cpu_tdp_watts: 65.0,
                gpu_tdp_watts: 150.0,
            },
            NoiseLevel::Unlimited => PowerBudget::unlimited(),
        }
    }
}
```

### 4. Configuration

```toml
[thermal]
mode = "balanced"         # performance | balanced | silent | custom
throttle_margin_c = 15.0  # Start acting 15°C before TjMax
sample_interval_secs = 5

[thermal.silent]
max_fan_rpm = 1200
max_cpu_temp_c = 75.0

[thermal.custom]
cpu_target_c = 80.0
gpu_target_c = 75.0

[thermal.protected_processes]
# Never throttle these even in thermal emergency
processes = ["audiodg.exe", "svchost.exe"]
```

### 5. UI Widget

```
┌──────────────────────────────────────────┐
│  Thermal                                  │
│                                           │
│  CPU: 72°C  ████████████░░░░░░ 72/100°C  │
│  GPU: 58°C  █████████░░░░░░░░░ 58/85°C   │
│  Fan: 1450 RPM                            │
│                                           │
│  Mode: [Balanced ▾]                       │
│                                           │
│  Throttle Prediction: Safe (>30 min)      │
│  Throttled Processes: 0                   │
│                                           │
│  Temperature History (1 hour)             │
│  100°C ┤                                  │
│   80°C ┤     ╭───╮                        │
│   60°C ┤─────╯   ╰──────────────          │
│   40°C ┤                                  │
│        └────────────────────────          │
└──────────────────────────────────────────┘
```

## Consequences

### Positive
- Prevents thermal throttling before it impacts foreground performance
- Silent mode enables comfortable use in quiet environments
- Proactive management is superior to reactive Windows thermal throttling
- Neural learning optimizes throttle timing over time

### Negative
- Background process throttling may slow background tasks (intentional tradeoff)
- Fan RPM reading unavailable on some hardware
- CPU temperature accuracy varies by sensor and motherboard
- Intel hybrid CPU detection requires CPUID parsing

### Security Considerations
- **Hardware protection hard limits**: Regardless of user configuration, thermal scheduler enforces absolute maximums: CPU never above TjMax-5°C, fan never below 500 RPM. These cannot be overridden.
- **Protected process list**: `audiodg.exe`, `svchost.exe`, `MsMpEng.exe` (Defender), `csrss.exe`, and all SYSTEM-owned processes are never throttled, even in thermal emergency
- **No direct fan control by default**: Fan speed is managed indirectly through CPU power limits, not direct PWM control. Direct fan control opt-in only with explicit warning.
- **Sensor validation**: Temperature readings cross-checked between WMI and vendor APIs; readings outside plausible range (0-120°C) are discarded
- **Rollback safety**: All throttling actions automatically reversed if RuVector crashes (watchdog timer restores original affinities/priorities)

### Risks
- Over-aggressive throttling could prevent important background tasks (mitigated: protected process list)
- Fan control (if implemented) could damage hardware (mitigated: indirect control by default, hard floor limits)
- Temperature readings may be inaccurate on some hardware (mitigated: cross-validation, plausible range check)

## Implementation Plan

### Phase 1: Monitoring
- [ ] CPU temperature reading via WMI
- [ ] GPU temperature via NVML/sysinfo
- [ ] Fan RPM detection
- [ ] Throttle trajectory prediction

### Phase 2: Balanced Mode
- [ ] Background process identification
- [ ] Priority and affinity adjustment
- [ ] Foreground protection
- [ ] Restore on cooldown

### Phase 3: Silent Mode
- [ ] Fan RPM-based power budgeting
- [ ] Progressive throttling
- [ ] Noise level presets

### Phase 4: UI
- [ ] Temperature gauges in Control Center
- [ ] Mode selector
- [ ] Temperature history chart
- [ ] Throttle event log

## References

- [WMI Thermal Zone](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-thermalzonetemperature)
- [NVML Temperature API](https://docs.nvidia.com/deploy/nvml-api/group__nvmlDeviceQueries.html)
- [Intel RAPL](https://www.kernel.org/doc/html/latest/power/powercap/powercap.html)
- [Process Priority Classes](https://learn.microsoft.com/en-us/windows/win32/procthread/scheduling-priorities)
