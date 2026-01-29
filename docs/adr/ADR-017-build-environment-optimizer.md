# ADR-017: Build Environment Optimizer

## Status
**Proposed**

## Date
2026-01-28

## Context

Software builds (cargo, webpack, msbuild, gcc, gradle) are resource-intensive burst workloads. During builds, the system should prioritize the build process with maximum CPU, memory, and I/O bandwidth. After the build, resources should be returned to normal. Currently, users either suffer slow builds due to background processes or manually adjust settings.

RuVector can detect build activity in real-time and automatically reconfigure the system for maximum build throughput, then restore normal operation when the build completes.

### Build Detection Signals
- Process names: `cargo.exe`, `rustc.exe`, `cl.exe`, `node.exe` (with webpack/vite args), `gradle`, `javac`, `gcc`, `g++`, `make`, `ninja`, `msbuild.exe`
- High CPU + high disk I/O pattern sustained >5 seconds
- File system watching on known build output directories
- Compiler-specific I/O patterns (many small reads, sequential large writes)

## Decision

### 1. Build Detection Engine

```rust
pub struct BuildDetector {
    known_compilers: HashSet<String>,
    active_builds: HashMap<u32, BuildSession>,  // pid -> session
    neural: AttentionScorer,                     // From existing neural engine
}

pub struct BuildSession {
    pub pid: u32,
    pub tool: BuildTool,
    pub started: Instant,
    pub project_dir: Option<PathBuf>,
    pub cpu_seconds: f64,
    pub peak_memory_mb: f64,
    pub disk_read_mb: f64,
    pub disk_write_mb: f64,
}

pub enum BuildTool {
    Cargo,          // Rust
    Webpack,        // JavaScript bundler
    Vite,           // JavaScript bundler
    MSBuild,        // .NET / C++
    Gradle,         // Java / Kotlin
    CMake,          // C / C++
    Make,           // C / C++
    Go,             // Go compiler
    Zig,            // Zig compiler
    Turborepo,      // Monorepo builds
    Bazel,          // Google build system
    Unknown(String),
}

impl BuildDetector {
    pub fn detect(&mut self, processes: &[ProcessInfo]) -> Vec<BuildEvent> {
        let mut events = vec![];

        for proc in processes {
            let name = proc.name.to_lowercase();

            // Check if this is a known compiler/build tool
            if let Some(tool) = self.identify_build_tool(&name, &proc.cmd_line) {
                if !self.active_builds.contains_key(&proc.pid) {
                    // New build detected
                    let session = BuildSession {
                        pid: proc.pid,
                        tool,
                        started: Instant::now(),
                        project_dir: self.detect_project_dir(&proc.cmd_line),
                        cpu_seconds: 0.0,
                        peak_memory_mb: 0.0,
                        disk_read_mb: 0.0,
                        disk_write_mb: 0.0,
                    };
                    self.active_builds.insert(proc.pid, session);
                    events.push(BuildEvent::Started(proc.pid));
                }
            }
        }

        // Check for completed builds
        let completed: Vec<u32> = self.active_builds.keys()
            .filter(|pid| !processes.iter().any(|p| p.pid == **pid))
            .cloned()
            .collect();

        for pid in completed {
            if let Some(session) = self.active_builds.remove(&pid) {
                events.push(BuildEvent::Completed(session));
            }
        }

        events
    }
}
```

### 2. Build Boost Actions

```rust
pub struct BuildBooster {
    original_state: Option<SystemSnapshot>,
    active: bool,
}

impl BuildBooster {
    pub async fn boost(&mut self, build: &BuildSession) -> BoostReport {
        // Save current state for restoration
        self.original_state = Some(SystemSnapshot::capture().await);

        let mut report = BoostReport::new();

        // 1. Set build process to High priority
        report.add(self.set_priority(build.pid, ProcessPriority::High));

        // 2. Set build process affinity to all cores
        report.add(self.set_affinity(build.pid, AffinityMask::All));

        // 3. Flush standby memory to give build maximum RAM
        report.add(self.flush_standby_memory());

        // 4. Reduce background process priorities
        report.add(self.throttle_background_processes());

        // 5. Disable Windows Search indexing on build directories
        if let Some(ref dir) = build.project_dir {
            report.add(self.disable_indexing(dir));
        }

        // 6. Set power plan to High Performance
        report.add(self.set_power_plan(PowerPlan::HighPerformance));

        // 7. Pin build to performance cores (Intel hybrid CPUs)
        if self.has_hybrid_cpu() {
            report.add(self.pin_to_p_cores(build.pid));
        }

        // 8. Increase I/O priority
        report.add(self.set_io_priority(build.pid, IoPriority::High));

        self.active = true;
        report
    }

    pub async fn unboost(&mut self) -> Result<(), String> {
        if let Some(snapshot) = self.original_state.take() {
            snapshot.restore().await?;
        }
        self.active = false;
        Ok(())
    }
}
```

### 3. Build Metrics & Learning

```rust
pub struct BuildMetrics {
    pub tool: BuildTool,
    pub duration: Duration,
    pub cpu_seconds: f64,
    pub peak_memory_mb: f64,
    pub disk_io_mb: f64,
    pub boosted: bool,
    pub boost_speedup: Option<f64>,  // Compared to unboosted baseline
}

impl BuildOptimizer {
    /// Learn which boost strategies work best for each build tool
    pub fn learn(&mut self, metrics: &BuildMetrics) {
        // Store: cargo builds benefit most from memory flush + IO priority
        // Store: webpack benefits most from high CPU priority
        // Store: msbuild benefits most from disabling indexing
        self.neural.learn_from_result(/* build metrics as pattern */);
    }

    /// Predict build duration and resource needs
    pub fn predict(&self, tool: &BuildTool, project_dir: &Path) -> BuildPrediction {
        // Based on historical builds of this project
        BuildPrediction {
            estimated_duration: Duration::from_secs(120),
            peak_memory_mb: 4096.0,
            recommended_boosts: vec![
                Boost::FlushStandby,
                Boost::HighPriority,
                Boost::DisableIndexing,
            ],
        }
    }
}
```

### 4. RAM Disk for Build Caches

```rust
pub struct RamDiskManager {
    active_disks: Vec<RamDisk>,
}

pub struct RamDisk {
    pub drive_letter: char,
    pub size_mb: u64,
    pub purpose: String,
    pub path_mappings: Vec<(PathBuf, PathBuf)>,  // original -> ramdisk
}

impl RamDiskManager {
    /// Create a RAM disk for build cache and symlink common cache directories
    pub fn create_build_cache(&mut self, tool: &BuildTool) -> Result<RamDisk, String> {
        let (size, mappings) = match tool {
            BuildTool::Cargo => (2048, vec![
                (dirs::home_dir().unwrap().join(".cargo/registry"),
                 PathBuf::from("R:\\cargo-registry")),
            ]),
            BuildTool::Webpack => (1024, vec![
                (PathBuf::from("node_modules/.cache"),
                 PathBuf::from("R:\\webpack-cache")),
            ]),
            _ => (1024, vec![]),
        };

        // Create RAM disk using Windows ImDisk or built-in API
        // Symlink cache directories
        // Return RamDisk handle for cleanup
    }
}
```

### 5. Notifications

```
Build Started (banner):
┌──────────────────────────────────────────┐
│ ▎ Build Mode Activated                   │
│ ▎ cargo build --release · Boosting...    │
└──────────────────────────────────────────┘

Build Completed (banner):
┌──────────────────────────────────────────┐
│ ▎ Build Complete (2m 14s)                │
│ ▎ 23% faster than average · Restored    │
└──────────────────────────────────────────┘
```

## Consequences

### Positive
- Measurable build time improvement (typical 15-30%)
- Automatic - no user intervention needed
- Neural learning adapts to specific build tools and projects
- RAM disk dramatically speeds up I/O-bound builds
- System restored to normal after build completes

### Negative
- Build detection has false positive risk (process name matching)
- RAM disk reduces available system memory
- Background process throttling may affect user experience during builds
- Intel hybrid CPU detection requires CPUID instruction parsing

### Security Considerations
- **Build tool verification**: Process identification uses full executable path + Authenticode signature, not just name (prevents `cargo.exe` in a malicious directory from triggering boost)
- **Path validation**: `project_dir` validated against user-accessible directories; no boost applied if path is outside user profile or known workspace roots
- **RAM disk cleanup**: RAM disk automatically destroyed on RuVector exit via `Drop` trait; OS handles cleanup on crash (temp filesystem)
- **RAM disk encryption**: Optional BitLocker encryption for RAM disks containing source code (default off, configurable)
- **Process tree tracking**: Build boost follows the process tree (parent + children), not just the detected compiler PID
- **No privilege escalation**: RAM disk creation uses user-space ImDisk API; admin-only operations (power plan, Search indexing) require explicit elevation

### Risks
- Over-aggressive throttling could affect foreground applications
- RAM disk data loss on crash (mitigated: caches are regenerable, `Drop` cleanup)
- Some builds spawn hundreds of processes (mitigated: process tree tracking)

## Implementation Plan

### Phase 1: Detection
- [ ] Build tool process detection
- [ ] Command-line argument parsing for tool identification
- [ ] Build session tracking (start/end)

### Phase 2: Boosting
- [ ] Process priority and affinity adjustment
- [ ] Standby memory flush on build start
- [ ] Background process throttling
- [ ] Power plan switching

### Phase 3: Learning
- [ ] Build duration tracking and history
- [ ] Per-project baseline establishment
- [ ] Speedup measurement (boosted vs. unboosted)

### Phase 4: Advanced
- [ ] RAM disk creation for build caches
- [ ] Windows Search indexing control
- [ ] Intel hybrid CPU P-core/E-core pinning

## References

- [SetPriorityClass](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setpriorityclass)
- [SetProcessAffinityMask](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-setprocessaffinitymask)
- [Intel Thread Director](https://www.intel.com/content/www/us/en/developer/articles/technical/hybrid-architecture.html)
- [ImDisk RAM disk](https://github.com/ArsenalRecon/Arsenal-Image-Mounter)
