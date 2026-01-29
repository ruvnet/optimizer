# ADR-016: WSL2 Memory Governor

## Status
**Proposed**

## Date
2026-01-28

## Context

WSL2 runs a full Linux kernel inside a Hyper-V lightweight VM (`Vmmem` process). By default, WSL2 can consume up to 50-80% of total physical RAM and rarely releases it back to Windows, even when Linux processes are idle. This is the #1 complaint of WSL2 users on Windows.

Microsoft provides `.wslconfig` for static limits, but this is a blunt instrument - a fixed cap means either wasted RAM when WSL is active or OOM kills when the cap is too low. RuVector can dynamically manage WSL2 memory based on actual Linux workload demand and Windows memory pressure.

### Current Pain Points
- `Vmmem` grows to 8-16GB and never shrinks
- `.wslconfig` `memory=4GB` cap causes Linux OOM during builds
- No real-time visibility into what Linux is actually using vs. cached
- Docker Desktop on WSL2 compounds the problem
- Users must restart WSL2 to reclaim memory (`wsl --shutdown`)

### RuVector Advantage
- Already monitors Windows memory pressure
- Already has neural learning for optimization decisions
- Already has process management infrastructure
- Can bridge Windows memory state with Linux memory state via `/proc/meminfo`

## Decision

### 1. Architecture

```
┌──────────────────────────────────────────────┐
│                  Windows Host                 │
│                                               │
│  ┌─────────────────────┐  ┌───────────────┐  │
│  │  RuVector Governor   │  │  Vmmem (WSL2) │  │
│  │  ┌───────────────┐  │  │  ┌──────────┐ │  │
│  │  │ Memory Monitor │──┼──┼─>│/proc/    │ │  │
│  │  │ Neural Engine  │  │  │  │meminfo   │ │  │
│  │  │ Policy Engine  │  │  │  │          │ │  │
│  │  │ Pressure Valve │──┼──┼─>│drop_caches│ │  │
│  │  └───────────────┘  │  │  └──────────┘ │  │
│  └─────────────────────┘  └───────────────┘  │
│                                               │
│  Memory Pool: [Windows ←→ WSL2 ←→ Docker]    │
└──────────────────────────────────────────────┘
```

### 2. Memory State Bridge

Read Linux memory state from within WSL2:

```rust
pub struct WslMemoryState {
    pub total_mb: f64,
    pub used_mb: f64,
    pub cached_mb: f64,       // Linux page cache (reclaimable)
    pub buffers_mb: f64,      // Buffer cache (reclaimable)
    pub available_mb: f64,    // Actually available to Linux apps
    pub swap_used_mb: f64,
    pub active_mb: f64,       // Actively used pages
    pub inactive_mb: f64,     // Inactive (candidates for reclaim)
    pub slab_reclaimable_mb: f64,
}

impl WslMemoryState {
    pub fn read() -> Result<Self, String> {
        // Execute: wsl -e cat /proc/meminfo
        // Parse key-value pairs
        let output = Command::new("wsl")
            .args(["-e", "cat", "/proc/meminfo"])
            .output()?;
        Self::parse_meminfo(&String::from_utf8_lossy(&output.stdout))
    }

    /// Memory that can be reclaimed without killing processes
    pub fn reclaimable_mb(&self) -> f64 {
        self.cached_mb + self.buffers_mb + self.slab_reclaimable_mb
    }

    /// Memory that Linux is actually using for applications
    pub fn actual_usage_mb(&self) -> f64 {
        self.used_mb - self.cached_mb - self.buffers_mb
    }
}
```

### 3. Governor Policies

```rust
pub enum GovernorPolicy {
    /// Dynamic: Adjust WSL2 memory based on both Windows and Linux pressure
    Dynamic {
        windows_target_available_percent: u32,  // Keep this much free for Windows
        linux_min_mb: u64,                       // Never shrink below this
        linux_max_mb: u64,                       // Never grow above this
        reclaim_interval_secs: u32,              // How often to check
    },

    /// Aggressive: Actively reclaim WSL2 cached memory when Windows is pressured
    Aggressive {
        windows_pressure_threshold: u32,  // Trigger at this % used
        target_reclaim_mb: u64,           // Try to free this much
    },

    /// Conservative: Only act when Windows is critically low
    Conservative {
        windows_critical_threshold: u32,  // Only act at 90%+ pressure
    },

    /// Scheduled: Compact WSL2 at specific times
    Scheduled {
        compact_times: Vec<String>,  // e.g., ["03:00", "12:00"]
    },
}
```

### 4. Reclamation Engine

```rust
pub struct WslGovernor {
    policy: GovernorPolicy,
    neural: NeuralDecisionEngine,
    history: VecDeque<GovernorAction>,
}

impl WslGovernor {
    pub async fn tick(&mut self) -> Option<GovernorAction> {
        let win_mem = WindowsMemoryOptimizer::new().get_memory_status()?;
        let wsl_mem = WslMemoryState::read().ok()?;
        let vmmem_mb = self.get_vmmem_memory_mb();

        // Calculate waste: memory Vmmem holds but Linux doesn't actively use
        let waste_mb = vmmem_mb - wsl_mem.actual_usage_mb();

        match &self.policy {
            GovernorPolicy::Dynamic { windows_target_available_percent, .. } => {
                let win_available = win_mem.available_physical_mb / win_mem.total_physical_mb * 100.0;

                if win_available < *windows_target_available_percent as f64 && waste_mb > 500.0 {
                    // Windows needs memory, WSL2 has reclaimable cache
                    let reclaim_target = (waste_mb * 0.7).min(
                        (win_mem.total_physical_mb * (*windows_target_available_percent as f64 / 100.0))
                        - win_mem.available_physical_mb
                    );
                    Some(self.reclaim(reclaim_target as u64).await)
                } else {
                    None
                }
            }
            // ... other policies
        }
    }

    async fn reclaim(&self, target_mb: u64) -> GovernorAction {
        let before = self.get_vmmem_memory_mb();

        // SECURITY: All WSL commands use fixed scripts, never string interpolation.
        // target_mb is validated as u64, then passed via a pre-written script
        // to avoid command injection through format strings.

        // Step 1: Drop Linux page cache (fixed command, no user input)
        let _ = Command::new("wsl")
            .args(["-u", "root", "-e", "sh", "-c",
                   "echo 3 > /proc/sys/vm/drop_caches"])
            .output();

        // Step 2: Compact WSL2 VM memory (Windows 11 22H2+)
        let _ = Command::new("wsl")
            .args(["--manage", "--memory", "compact"])
            .output();

        // Step 3: Trigger memory reclaim via cgroup
        // Validate target_mb is within safe bounds before passing
        let safe_bytes = target_mb.min(65536) * 1024 * 1024; // Cap at 64GB
        let reclaim_cmd = format!("{}", safe_bytes); // u64 -> decimal string, no injection
        let _ = Command::new("wsl")
            .args(["-u", "root", "-e", "sh", "-c",
                   &format!("echo {} > /sys/fs/cgroup/memory.reclaim", reclaim_cmd)])
            .output();

        let after = self.get_vmmem_memory_mb();
        let freed = (before - after).max(0.0);

        GovernorAction {
            timestamp: chrono::Local::now(),
            target_mb,
            freed_mb: freed,
            method: "drop_caches + compact + cgroup_reclaim",
        }
    }
}
```

### 5. Docker Desktop Integration

Docker Desktop on WSL2 adds another layer of memory management:

```rust
pub struct DockerMemoryState {
    pub container_count: u32,
    pub running_count: u32,
    pub total_memory_mb: f64,
    pub idle_containers: Vec<ContainerInfo>,
}

impl DockerMemoryState {
    pub fn read() -> Result<Self, String> {
        let output = Command::new("wsl")
            .args(["-e", "docker", "stats", "--no-stream", "--format",
                "{{.Name}}\t{{.MemUsage}}\t{{.CPUPerc}}"])
            .output()?;
        // Parse docker stats output
    }

    /// Pause idle containers to free memory
    pub fn pause_idle(&self, idle_threshold_minutes: u32) -> Vec<String> {
        // Pause containers with <1% CPU for N minutes
    }
}
```

### 6. Neural Learning Integration

The governor learns optimal reclamation timing:

```rust
impl WslGovernor {
    fn learn_from_action(&mut self, action: &GovernorAction, user_complaint: bool) {
        // Features: time_of_day, windows_pressure, wsl_usage, docker_containers
        // Positive: freed memory, no user complaint
        // Negative: user complained about WSL slowdown, or Linux OOM within 5min
        self.neural.learn_from_result(/* ... */);
    }
}
```

### 7. Configuration

```toml
[wsl_governor]
enabled = true
policy = "dynamic"
check_interval_secs = 30
windows_target_available_percent = 25
linux_min_mb = 2048
linux_max_mb = 16384
auto_pause_idle_containers = true
idle_container_threshold_minutes = 30

# Emergency: if Windows <5% free, force WSL compact
emergency_threshold_percent = 5
```

## Consequences

### Positive
- Solves the #1 WSL2 pain point (memory not released)
- Dynamic adjustment is superior to static `.wslconfig` cap
- Docker container pausing saves significant memory
- No WSL2 restart required
- Neural learning improves over time

### Negative
- `drop_caches` can slow down subsequent Linux disk operations
- Requires WSL2 command execution which adds latency
- `sudo` in WSL may require password unless configured
- Some operations need Windows 11 22H2+ (`wsl --manage`)

### Security Considerations
- **Command injection prevention**: WSL commands use fixed scripts with validated numeric parameters only; no string interpolation from user or config input
- **Privilege isolation**: `wsl -u root` used instead of `sudo tee` (avoids password prompt and sudo config dependency)
- **Input bounds validation**: `target_mb` capped at 64GB to prevent overflow; only `u64` decimal values passed to shell
- **WSL distribution validation**: Only operate on configured default distribution; verify WSL state before executing commands
- **Docker API security**: Docker stats queried read-only; container pause requires user confirmation
- **No arbitrary command execution**: All WSL interactions use a fixed set of predefined commands

### Risks
- Over-aggressive reclamation could cause Linux OOM
- Docker container pausing may break dependent services
- WSL2 command execution could fail if WSL is in a bad state
- Multiple WSL distributions complicate management

## Implementation Plan

### Phase 1: Monitoring
- [ ] Read `/proc/meminfo` via `wsl -e`
- [ ] Track Vmmem process memory
- [ ] Display WSL2 memory breakdown in tray tooltip

### Phase 2: Basic Governor
- [ ] Implement `drop_caches` reclamation
- [ ] Conservative policy (act only on critical pressure)
- [ ] Reclamation history logging

### Phase 3: Advanced Governor
- [ ] Dynamic policy with neural learning
- [ ] Docker container detection and pausing
- [ ] `wsl --manage --memory compact` integration

### Phase 4: UI Integration
- [ ] WSL2 section in Memory page (Control Center)
- [ ] Governor policy configuration UI
- [ ] Reclamation history chart

## References

- [WSL2 memory configuration](https://learn.microsoft.com/en-us/windows/wsl/wsl-config)
- [Linux memory management](https://www.kernel.org/doc/html/latest/admin-guide/mm/concepts.html)
- [cgroup memory.reclaim](https://docs.kernel.org/admin-guide/cgroup-v2.html)
- [Docker memory management](https://docs.docker.com/config/containers/resource_constraints/)
