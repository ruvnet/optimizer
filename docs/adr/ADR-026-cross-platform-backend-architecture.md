# ADR-026: Cross-Platform Backend Handler Architecture

## Status
**Accepted**

## Date
2026-01-29

## Context

The Control Center UI (WebView2, documented in ADR-012) has 13 feature pages (ADR-013 through ADR-025) with complete frontend JavaScript components that send IPC messages to the Rust backend. However, **zero backend handlers currently exist**. The project was initially developed for Windows, but macOS support (ADR-011) and future Linux/Unix support require a cross-platform backend architecture.

### Current State
- **Frontend**: Complete JavaScript IPC message senders for all 13 features
- **Backend**: Empty stub in `control_center.rs` that returns "not implemented" for all IPC messages
- **Platform support**: Windows-only implementations exist for core memory optimization
- **Feature coverage**: 0 of 13 features have working backend handlers

### Requirements
1. **Cross-platform support**: Windows (primary), macOS (ADR-011), Linux/Unix (future)
2. **Feature parity**: All 13 features must work where technically feasible
3. **Graceful degradation**: Platform-unsupported features return clear error messages
4. **Maintainability**: Clean module separation per feature
5. **IPC integration**: All handlers must integrate with existing `control_center.rs` IPC dispatch
6. **Data persistence**: Per-feature configuration must persist across restarts

### Feature Matrix by Platform

| Feature (ADR) | Windows | macOS | Linux | Notes |
|---------------|---------|-------|-------|-------|
| 013 Workspace Profiles | ✓ | ✓ | ✓ | Process management varies by platform |
| 014 System Health Score | ✓ | ✓ | ✓ | `sysinfo` provides cross-platform metrics |
| 015 Startup Optimizer | ✓ | ✓ | ✓ | Different boot item locations per OS |
| 016 WSL2 Governor | ✓ | ✗ | ✗ | Windows-only (WSL2 is a Windows feature) |
| 017 Build Environment | ✓ | ✓ | ✓ | Tool detection varies by platform |
| 018 Spectral Leak Detector | ✓ | ✓ | ✓ | Uses existing `SpectralAnalyzer` |
| 019 Predictive Prefetcher | ✓ | ✓ | ✓ | Markov chain logic is platform-agnostic |
| 020 Thermal Scheduler | ✓ | ✓ | ✓ | Temperature sensors vary by platform |
| 021 WASM Plugins | ✓ | ✓ | ✓ | `wasmer` provides cross-platform WASM |
| 022 GPU Optimizer | ✓ (NVML) | ✓ (Metal) | ✓ (NVML) | GPU APIs differ significantly |
| 023 Bloatware Silencer | ✓ | ✓ | ✓ | Known bloatware DB per platform |
| 024 Time Travel State | ✓ | ✓ | ✓ | Snapshot format is platform-specific |
| 025 Desktop Automation | ✓ | ✓ | ✓ | Accessibility APIs vary by platform |

## Decision

### 1. Module Structure

Create a `src/features/` directory tree with one module per feature:

```
src/
  features/
    mod.rs              # Module declarations + FeatureRegistry
    profiles.rs         # ADR-013: Workspace profile CRUD, TOML persistence
    health.rs           # ADR-014: Composite health score (memory + CPU + disk + network)
    startup.rs          # ADR-015: Boot item enumeration, timing, enable/disable
    wsl2.rs             # ADR-016: WSL2 memory governance (.wslconfig, reclaim)
    build.rs            # ADR-017: Build tool detection, cache management
    leaks.rs            # ADR-018: Spectral analysis leak detection
    prefetch.rs         # ADR-019: Markov chain app launch prediction
    thermal.rs          # ADR-020: CPU temperature, throttle detection, scheduling
    plugins.rs          # ADR-021: WASM plugin loading via wasmer
    gpu.rs              # ADR-022: GPU/VRAM monitoring (NVML, Metal, OpenCL)
    bloatware.rs        # ADR-023: Known bloatware database, telemetry blocking
    timeline.rs         # ADR-024: System state snapshots, diff, rollback
    agent.rs            # ADR-025: Desktop automation agent, trajectory recording
```

Each feature module is **self-contained** and exports a handler function:

```rust
// src/features/mod.rs
pub mod profiles;
pub mod health;
pub mod startup;
pub mod wsl2;
pub mod build;
pub mod leaks;
pub mod prefetch;
pub mod thermal;
pub mod plugins;
pub mod gpu;
pub mod bloatware;
pub mod timeline;
pub mod agent;

/// Dispatch IPC message to the appropriate feature handler
pub async fn handle_feature_ipc(msg_type: &str, value: &serde_json::Value) -> String {
    match msg_type {
        s if s.starts_with("profiles:") => profiles::handle_ipc(msg_type, value).await,
        s if s.starts_with("health:") => health::handle_ipc(msg_type, value).await,
        s if s.starts_with("startup:") => startup::handle_ipc(msg_type, value).await,
        s if s.starts_with("wsl2:") => wsl2::handle_ipc(msg_type, value).await,
        s if s.starts_with("build:") => build::handle_ipc(msg_type, value).await,
        s if s.starts_with("leaks:") => leaks::handle_ipc(msg_type, value).await,
        s if s.starts_with("prefetch:") => prefetch::handle_ipc(msg_type, value).await,
        s if s.starts_with("thermal:") => thermal::handle_ipc(msg_type, value).await,
        s if s.starts_with("plugins:") => plugins::handle_ipc(msg_type, value).await,
        s if s.starts_with("gpu:") => gpu::handle_ipc(msg_type, value).await,
        s if s.starts_with("bloatware:") => bloatware::handle_ipc(msg_type, value).await,
        s if s.starts_with("timeline:") => timeline::handle_ipc(msg_type, value).await,
        s if s.starts_with("agent:") => agent::handle_ipc(msg_type, value).await,
        _ => json!({"error": "Unknown feature"}).to_string(),
    }
}
```

### 2. Feature Handler Pattern

Each feature module follows this pattern:

```rust
// src/features/example.rs
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Feature-specific state (if needed)
pub struct ExampleManager {
    config: ExampleConfig,
    state: ExampleState,
}

#[derive(Serialize, Deserialize)]
struct ExampleConfig {
    // Feature configuration
}

/// IPC message dispatcher for this feature
pub async fn handle_ipc(msg_type: &str, value: &Value) -> String {
    match msg_type {
        "example:get_config" => get_config(value).await,
        "example:set_config" => set_config(value).await,
        "example:perform_action" => perform_action(value).await,
        _ => json!({"error": "Unknown message type"}).to_string(),
    }
}

#[cfg(target_os = "windows")]
async fn get_config(value: &Value) -> String {
    // Windows implementation
}

#[cfg(target_os = "macos")]
async fn get_config(value: &Value) -> String {
    // macOS implementation
}

#[cfg(target_os = "linux")]
async fn get_config(value: &Value) -> String {
    // Linux implementation
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
async fn get_config(value: &Value) -> String {
    json!({"error": "Not supported on this platform"}).to_string()
}
```

### 3. Cross-Platform Strategy

#### 3.1 Shared Dependencies

```toml
[dependencies]
# Cross-platform system info
sysinfo = "0.32"              # CPU, memory, disk, network, process info

# File I/O and paths
dirs = "5.0"                  # Standard config/data directories

# WASM runtime (cross-platform)
wasmer = "4.2"                # WASM plugin execution

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Platform-specific (feature-gated)
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.52", features = [
    "Win32_System_ProcessStatus",
    "Win32_System_Threading",
    "Win32_System_Power",
    "Win32_Storage_FileSystem",
] }

[target.'cfg(target_os = "macos")'.dependencies]
core-foundation = "0.9"
core-foundation-sys = "0.8"
```

#### 3.2 Platform-Specific Code Structure

Use conditional compilation for platform-specific implementations:

```rust
// profiles.rs - Example of cross-platform process management
#[cfg(target_os = "windows")]
pub fn set_process_priority(pid: u32, priority: ProcessPriority) -> Result<(), String> {
    use windows::Win32::System::Threading::*;
    // Windows-specific SetPriorityClass implementation
}

#[cfg(target_os = "macos")]
pub fn set_process_priority(pid: u32, priority: ProcessPriority) -> Result<(), String> {
    // macOS-specific setpriority() implementation via libc
}

#[cfg(target_os = "linux")]
pub fn set_process_priority(pid: u32, priority: ProcessPriority) -> Result<(), String> {
    // Linux-specific setpriority() implementation via libc
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn set_process_priority(pid: u32, priority: ProcessPriority) -> Result<(), String> {
    Err("Process priority not supported on this platform".to_string())
}
```

#### 3.3 Configuration Directory Standards

Use `dirs` crate for platform-appropriate config paths:

```rust
use dirs::config_dir;
use std::path::PathBuf;

pub fn get_ruvector_config_dir() -> PathBuf {
    let base = config_dir().expect("Failed to get config directory");
    // Windows: %APPDATA%\RuVector
    // macOS:   ~/Library/Application Support/RuVector
    // Linux:   ~/.config/ruvector
    base.join(if cfg!(target_os = "macos") { "RuVector" } else { "ruvector" })
}

pub fn get_feature_config_path(feature_name: &str) -> PathBuf {
    get_ruvector_config_dir().join(format!("{}.toml", feature_name))
}
```

### 4. Data Persistence

Each feature manages its own configuration file:

```rust
// Example: profiles.rs
pub async fn save_profile(profile: &WorkspaceProfile) -> Result<(), String> {
    let config_path = get_feature_config_path("profiles");
    let toml_str = toml::to_string_pretty(profile)
        .map_err(|e| format!("Serialization error: {}", e))?;

    tokio::fs::create_dir_all(config_path.parent().unwrap()).await
        .map_err(|e| format!("Failed to create config dir: {}", e))?;

    tokio::fs::write(&config_path, toml_str).await
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(())
}

pub async fn load_profiles() -> Result<Vec<WorkspaceProfile>, String> {
    let config_path = get_feature_config_path("profiles");

    if !config_path.exists() {
        return Ok(get_default_profiles()); // Built-in defaults
    }

    let toml_str = tokio::fs::read_to_string(&config_path).await
        .map_err(|e| format!("Failed to read config: {}", e))?;

    toml::from_str(&toml_str)
        .map_err(|e| format!("Failed to parse config: {}", e))
}
```

Configuration file locations:

| Platform | Base Directory | Example |
|----------|----------------|---------|
| Windows | `%APPDATA%\RuVector` | `C:\Users\ruv\AppData\Roaming\RuVector\profiles.toml` |
| macOS | `~/Library/Application Support/RuVector` | `/Users/ruv/Library/Application Support/RuVector/profiles.toml` |
| Linux | `~/.config/ruvector` | `/home/ruv/.config/ruvector/profiles.toml` |

### 5. IPC Integration

Modify `src/control_center.rs` to dispatch to feature handlers:

```rust
// src/control_center.rs
use crate::features;

impl ControlCenterView {
    async fn handle_ipc(&self, msg: &str, value: serde_json::Value) -> String {
        // Extract feature prefix
        if let Some((feature, _)) = msg.split_once(':') {
            // Dispatch to feature module
            features::handle_feature_ipc(msg, &value).await
        } else {
            // Legacy messages (ui:, theme:, etc.) handled here
            self.handle_legacy_ipc(msg, &value).await
        }
    }
}
```

### 6. Feature-Specific Platform Notes

#### 6.1 WSL2 Governor (ADR-016)
```rust
// src/features/wsl2.rs
#[cfg(target_os = "windows")]
pub async fn handle_ipc(msg_type: &str, value: &Value) -> String {
    // Read/write %USERPROFILE%\.wslconfig
    // Call wsl.exe --shutdown, wsl.exe --list, etc.
}

#[cfg(not(target_os = "windows"))]
pub async fn handle_ipc(msg_type: &str, value: &Value) -> String {
    json!({"error": "WSL2 is only available on Windows"}).to_string()
}
```

#### 6.2 GPU Optimizer (ADR-022)
```rust
// src/features/gpu.rs
#[cfg(all(target_os = "windows", feature = "nvml"))]
pub async fn get_gpu_info() -> Result<GpuInfo, String> {
    // Use NVML (nvidia-ml-sys) for NVIDIA GPUs
}

#[cfg(target_os = "macos")]
pub async fn get_gpu_info() -> Result<GpuInfo, String> {
    // Use Metal framework for Apple Silicon GPU
}

#[cfg(all(target_os = "linux", feature = "nvml"))]
pub async fn get_gpu_info() -> Result<GpuInfo, String> {
    // Use NVML on Linux for NVIDIA GPUs
}
```

#### 6.3 Startup Optimizer (ADR-015)
```rust
// src/features/startup.rs
#[cfg(target_os = "windows")]
async fn list_startup_items() -> Vec<StartupItem> {
    // Read registry: HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run
    // Read folder: %APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup
}

#[cfg(target_os = "macos")]
async fn list_startup_items() -> Vec<StartupItem> {
    // Read: ~/Library/LaunchAgents/*.plist
    // Read: /Library/LaunchAgents/*.plist
    // Read: /Library/LaunchDaemons/*.plist
}

#[cfg(target_os = "linux")]
async fn list_startup_items() -> Vec<StartupItem> {
    // Read: ~/.config/autostart/*.desktop
    // Read: /etc/xdg/autostart/*.desktop
    // Read systemd: systemctl list-unit-files --type=service --state=enabled
}
```

#### 6.4 Thermal Scheduler (ADR-020)
```rust
// src/features/thermal.rs
#[cfg(target_os = "windows")]
async fn get_cpu_temperature() -> Option<f32> {
    // Use WMI: Win32_TemperatureProbe or OpenHardwareMonitor
}

#[cfg(target_os = "macos")]
async fn get_cpu_temperature() -> Option<f32> {
    // Use SMC (System Management Controller) keys via IOKit
    // Or: sysctl -a | grep machdep.xcpm.cpu_thermal_level
}

#[cfg(target_os = "linux")]
async fn get_cpu_temperature() -> Option<f32> {
    // Read: /sys/class/thermal/thermal_zone*/temp
    // Or: sensors command from lm-sensors package
}
```

### 7. Error Handling and Graceful Degradation

All platform-unsupported operations return structured errors:

```rust
#[derive(Serialize)]
struct FeatureError {
    error: String,
    platform: String,
    feature: String,
    supported_platforms: Vec<String>,
}

#[cfg(not(target_os = "windows"))]
async fn windows_only_feature() -> String {
    let err = FeatureError {
        error: "This feature is only available on Windows".to_string(),
        platform: std::env::consts::OS.to_string(),
        feature: "wsl2_governor".to_string(),
        supported_platforms: vec!["windows".to_string()],
    };
    serde_json::to_string(&err).unwrap()
}
```

Frontend can display user-friendly messages:

```javascript
// Control Center UI
async function callFeature(msgType, data) {
    const response = await window.ipc(msgType, data);
    const parsed = JSON.parse(response);

    if (parsed.error && parsed.supported_platforms) {
        showUnsupportedFeature(parsed);
        return null;
    }

    return parsed;
}

function showUnsupportedFeature(error) {
    alert(`${error.feature} is not available on ${error.platform}.\n` +
          `Supported platforms: ${error.supported_platforms.join(", ")}`);
}
```

## Consequences

### Positive
1. **Clean separation**: One module per feature = easy to navigate and maintain
2. **Cross-platform by design**: `#[cfg(target_os = "...")]` makes platform support explicit
3. **Graceful degradation**: Unsupported features return clear errors instead of crashing
4. **Testability**: Each feature module can be tested independently
5. **Incremental implementation**: Features can be implemented one at a time
6. **Existing code reuse**: Spectral leak detector (ADR-018) reuses existing `SpectralAnalyzer`
7. **Standard directories**: Uses OS-appropriate config paths (`dirs` crate)
8. **WASM portability**: `wasmer` provides cross-platform plugin execution (ADR-021)

### Negative
1. **Platform fragmentation**: Some features require 3 separate implementations (Windows/macOS/Linux)
2. **Testing burden**: Must test on all 3 platforms for full coverage
3. **API differences**: GPU APIs (NVML vs Metal vs OpenCL) are fundamentally different
4. **Privilege requirements**: Some features require elevated privileges (process priority, service control)
5. **Maintenance**: Platform-specific code paths increase maintenance burden
6. **Feature disparity**: WSL2 governor will never work on macOS/Linux (inherent limitation)
7. **Dependency bloat**: Platform-specific dependencies increase binary size

### Security Considerations
1. **IPC validation**: All incoming IPC messages must validate JSON schema
2. **Path traversal**: Configuration file paths must be sanitized (no `../` escapes)
3. **Process killing**: Only allow terminating processes owned by the current user
4. **Service control**: Maintain allow-list of safe services; block critical system services
5. **WASM sandboxing**: `wasmer` provides memory-safe WASM execution (ADR-021)
6. **Registry access**: Windows registry writes require schema validation
7. **File permissions**: Configuration files must have restricted permissions (0600 on Unix)

### Risks
1. **Windows registry corruption**: Startup optimizer (ADR-015) modifying wrong keys
   - *Mitigation*: Dry-run mode, backup registry keys before modification
2. **Process termination**: Bloatware silencer (ADR-023) killing wrong processes
   - *Mitigation*: Full path + signature verification, user confirmation for system processes
3. **GPU instability**: GPU optimizer (ADR-022) setting invalid clocks/power limits
   - *Mitigation*: Whitelist of known-safe settings, gradual adjustment with stability monitoring
4. **macOS SIP restrictions**: System Integrity Protection may block some operations
   - *Mitigation*: Detect SIP status, gracefully disable unsupported features

## Implementation Plan

### Phase 1: Infrastructure (Week 1)
- [ ] Create `src/features/mod.rs` with dispatcher
- [ ] Implement `get_ruvector_config_dir()` with platform detection
- [ ] Set up `sysinfo` integration for cross-platform metrics
- [ ] Modify `control_center.rs` to dispatch to feature handlers
- [ ] Add error handling for unsupported platforms

### Phase 2: Core Features (Weeks 2-3)
- [ ] **profiles.rs** (ADR-013): TOML persistence, process priority, power plans
- [ ] **health.rs** (ADR-014): Composite score using `sysinfo` metrics
- [ ] **startup.rs** (ADR-015): Boot item enumeration (registry/plist/systemd)
- [ ] **leaks.rs** (ADR-018): Hook into existing `SpectralAnalyzer`

### Phase 3: Platform-Specific Features (Weeks 4-5)
- [ ] **wsl2.rs** (ADR-016): Windows-only `.wslconfig` management
- [ ] **build.rs** (ADR-017): Tool detection (cargo/npm/docker cache paths)
- [ ] **thermal.rs** (ADR-020): Temperature sensors (WMI/SMC/thermal_zone)
- [ ] **gpu.rs** (ADR-022): GPU info (NVML/Metal/OpenCL)

### Phase 4: Advanced Features (Weeks 6-7)
- [ ] **prefetch.rs** (ADR-019): Markov chain prediction engine
- [ ] **plugins.rs** (ADR-021): `wasmer` WASM loader with sandboxing
- [ ] **bloatware.rs** (ADR-023): Known bloatware database, process blocking
- [ ] **timeline.rs** (ADR-024): System snapshot format, diff engine

### Phase 5: Automation (Week 8)
- [ ] **agent.rs** (ADR-025): Desktop automation API integration
- [ ] Cross-platform accessibility API wrappers (UI Automation/AccessibilityKit/AT-SPI)
- [ ] Trajectory recording and playback
- [ ] Neural learning integration with existing `NeuralDecisionEngine`

### Phase 6: Testing & Documentation (Week 9)
- [ ] Unit tests for each feature module
- [ ] Integration tests with IPC simulation
- [ ] Cross-platform CI (Windows/macOS/Linux runners)
- [ ] Per-feature documentation in `docs/features/`
- [ ] User-facing feature availability matrix

## Alternatives Considered

### Alternative 1: Monolithic Backend (Rejected)
Put all feature logic in `control_center.rs` with a giant `match` statement.

**Rejected because:**
- Impossible to maintain (thousands of lines in one file)
- No separation of concerns
- Testing becomes a nightmare
- Cross-platform code would be interleaved

### Alternative 2: Separate Binaries (Rejected)
Ship separate binaries for each feature (e.g., `ruvector-profiles.exe`, `ruvector-health.exe`).

**Rejected because:**
- Distribution complexity (13+ binaries)
- IPC overhead between processes
- Shared state management becomes complex
- Startup cost (launching 13 processes)

### Alternative 3: Dynamic Plugin Loading (Considered)
Load each feature as a dynamically linked library (.dll/.dylib/.so).

**Rejected because:**
- Significantly increases complexity
- Plugin interface versioning is hard
- Worse startup performance (loading 13+ dynamic libraries)
- Better suited for third-party extensions (already covered by ADR-021 WASM plugins)

### Alternative 4: JavaScript Backend (Rejected)
Implement backend handlers in JavaScript using Node.js or Deno.

**Rejected because:**
- Performance penalty for system-level operations
- Loses Rust's memory safety guarantees
- Harder to call platform-specific APIs (Windows registry, etc.)
- Larger binary size (bundling JS runtime)

## References

- [sysinfo crate](https://docs.rs/sysinfo) - Cross-platform system information
- [dirs crate](https://docs.rs/dirs) - Platform-appropriate directories
- [wasmer](https://wasmer.io/) - Cross-platform WASM runtime
- [Windows API (windows crate)](https://microsoft.github.io/windows-docs-rs/)
- [macOS IOKit](https://developer.apple.com/documentation/iokit)
- [Linux sysfs](https://www.kernel.org/doc/Documentation/filesystems/sysfs.txt)
- [ADR-011: macOS Port](./ADR-011-macos-port.md) - macOS-specific considerations
- [ADR-012: Control Center UI](./ADR-012-control-center-ui.md) - IPC integration
- [ADR-013 through ADR-025](./README.md) - Individual feature specifications
