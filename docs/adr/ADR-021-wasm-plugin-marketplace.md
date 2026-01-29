# ADR-021: WASM Plugin Marketplace

## Status
**Proposed**

## Date
2026-01-28

## Context

RuVector already has Wasmer (`wasmer = "4.3"`, `wasmer-compiler-singlepass = "4.3"`) compiled into the Windows binary. This runtime can execute WebAssembly modules, enabling a plugin system where anyone can write custom optimization logic in any language that compiles to WASM (Rust, C, C++, Go, AssemblyScript, Zig).

A plugin marketplace transforms RuVector from a closed tool into a platform. Community members can create and share:
- Custom process rules (e.g., "auto-kill Adobe updater when Photoshop closes")
- Hardware-specific optimizations (e.g., "Lenovo ThinkPad thermal tweaks")
- Workflow automations (e.g., "optimize for OBS streaming")
- Integration plugins (e.g., "sync with Home Assistant for smart fan control")

### Existing Infrastructure
- Wasmer 4.3 runtime (Singlepass compiler for fast startup)
- TOML configuration system
- IPC framework (named pipes)
- Neural learning engine (plugins can register training data)

## Decision

### 1. Plugin Interface (WASM ABI)

```rust
/// Host functions exposed to WASM plugins
pub trait PluginHost {
    // System queries
    fn get_memory_status() -> MemoryStatus;
    fn get_cpu_usage() -> f64;
    fn get_process_list() -> Vec<ProcessInfo>;
    fn get_process_memory(pid: u32) -> u64;
    fn get_temperature() -> ThermalSnapshot;
    fn get_gpu_status() -> GpuStatus;

    // Actions (sandboxed, require capability)
    fn set_process_priority(pid: u32, priority: u8) -> Result<(), Error>;
    fn trim_process_memory(pid: u32) -> Result<u64, Error>;
    fn show_notification(title: &str, message: &str);
    fn log(level: u8, message: &str);

    // Configuration
    fn get_config(key: &str) -> Option<String>;
    fn set_config(key: &str, value: &str);

    // Neural integration
    fn report_metric(name: &str, value: f64);
    fn get_learned_pattern(name: &str) -> Option<Vec<f64>>;
}

/// Plugin-implemented functions
pub trait Plugin {
    fn name() -> &'static str;
    fn version() -> &'static str;
    fn capabilities() -> Vec<Capability>;

    fn on_init();
    fn on_tick(interval_ms: u32);           // Called every N ms
    fn on_memory_pressure(level: u8);       // 0=normal, 1=high, 2=critical
    fn on_process_launch(name: &str, pid: u32);
    fn on_process_exit(name: &str, pid: u32);
    fn on_profile_switch(profile: &str);
    fn on_cleanup();
}
```

### 2. Capability System (Sandboxing)

```rust
pub enum Capability {
    ReadSystemInfo,          // Read CPU, memory, temperature
    ReadProcessList,         // See running processes
    ModifyProcessPriority,   // Change process priority (requires approval)
    TrimProcessMemory,       // Trim working sets (requires approval)
    KillProcess,             // Terminate processes (requires explicit approval)
    ShowNotifications,       // Display notifications
    NetworkAccess,           // Make HTTP requests (requires approval)
    FileSystemRead(PathBuf), // Read specific paths
    FileSystemWrite(PathBuf),// Write specific paths
    RegistryRead,            // Read Windows Registry
    RegistryWrite,           // Write Registry (requires explicit approval)
    RunCommand,              // Execute shell commands (requires explicit approval)
}

pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub homepage: Option<String>,
    pub license: String,
    pub capabilities: Vec<Capability>,
    pub min_ruvector_version: String,
    pub platforms: Vec<String>,        // ["windows", "macos"]
    pub wasm_file: String,             // "plugin.wasm"
    pub config_schema: Option<String>, // JSON Schema for plugin config
    pub signature: Option<String>,     // Code signing (optional)
}
```

### 3. Plugin Lifecycle

```rust
pub struct PluginManager {
    runtime: wasmer::Store,
    plugins: HashMap<String, LoadedPlugin>,
    registry: PluginRegistry,
}

pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub instance: wasmer::Instance,
    pub enabled: bool,
    pub approved_capabilities: Vec<Capability>,
    pub metrics: PluginMetrics,
}

impl PluginManager {
    pub fn install(&mut self, wasm_bytes: &[u8], manifest: PluginManifest) -> Result<(), String> {
        // 1. Validate WASM module
        let module = wasmer::Module::new(&self.runtime, wasm_bytes)?;

        // 2. Check capabilities - prompt user for dangerous ones
        let dangerous: Vec<_> = manifest.capabilities.iter()
            .filter(|c| c.is_dangerous())
            .collect();
        if !dangerous.is_empty() {
            // Show approval dialog
            // "Plugin 'X' requests: Kill processes, Write registry. Allow?"
        }

        // 3. Create sandboxed instance with only approved host functions
        let imports = self.create_imports(&manifest.capabilities);
        let instance = wasmer::Instance::new(&module, &imports)?;

        // 4. Call on_init
        let init = instance.exports.get_function("on_init")?;
        init.call(&[])?;

        self.plugins.insert(manifest.name.clone(), LoadedPlugin {
            manifest,
            instance,
            enabled: true,
            approved_capabilities: /* filtered */,
            metrics: PluginMetrics::new(),
        });

        Ok(())
    }

    pub fn tick(&mut self) {
        for (name, plugin) in &self.plugins {
            if !plugin.enabled { continue; }

            let start = Instant::now();
            if let Ok(tick_fn) = plugin.instance.exports.get_function("on_tick") {
                let _ = tick_fn.call(&[wasmer::Value::I32(5000)]); // 5s interval
            }
            plugin.metrics.record_tick(start.elapsed());

            // Kill plugin if it exceeds time limit (100ms per tick)
            if start.elapsed() > Duration::from_millis(100) {
                tracing::warn!("Plugin '{}' exceeded tick time limit", name);
                plugin.enabled = false;
            }
        }
    }
}
```

### 4. Plugin Registry / Marketplace

```rust
pub struct PluginRegistry {
    pub remote_url: String,      // "https://plugins.ruvector.dev/api/v1"
    pub local_cache: PathBuf,    // "~/.ruvector/plugins/"
    pub installed: Vec<InstalledPlugin>,
}

pub struct RegistryEntry {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub downloads: u64,
    pub rating: f64,              // 0-5 stars
    pub verified: bool,           // RuVector team verified
    pub categories: Vec<String>,  // ["gaming", "thermal", "developer"]
    pub size_bytes: u64,
    pub wasm_url: String,
    pub manifest_url: String,
    pub sha256: String,
}

// Example plugins:
// - "obs-optimizer" - Auto-configure system for OBS streaming
// - "thinkpad-thermal" - Lenovo ThinkPad-specific fan curves
// - "docker-cleaner" - Remove unused Docker images/volumes
// - "chrome-tab-limiter" - Suspend inactive Chrome tabs
// - "meeting-mode" - Detect video calls and boost network/audio
// - "nvidia-oc" - GPU overclocking profiles per application
// - "battery-guard" - Limit charge to 80% for battery longevity
// - "home-assistant" - Smart home integration for power/thermal
```

### 5. Example Plugin (Rust -> WASM)

```rust
// plugins/chrome-tab-limiter/src/lib.rs
use ruvector_plugin_sdk::*;

static mut MAX_TABS: u32 = 30;
static mut SUSPENDED_TABS: u32 = 0;

#[export_name = "name"]
pub extern "C" fn name() -> *const u8 { b"Chrome Tab Limiter\0".as_ptr() }

#[export_name = "version"]
pub extern "C" fn version() -> *const u8 { b"1.0.0\0".as_ptr() }

#[export_name = "on_init"]
pub extern "C" fn on_init() {
    if let Some(val) = get_config("max_tabs") {
        unsafe { MAX_TABS = val.parse().unwrap_or(30); }
    }
    log_info("Chrome Tab Limiter initialized");
}

#[export_name = "on_tick"]
pub extern "C" fn on_tick(_interval_ms: u32) {
    let processes = get_process_list();
    let chrome_count = processes.iter()
        .filter(|p| p.name == "chrome.exe")
        .count() as u32;

    unsafe {
        if chrome_count > MAX_TABS {
            // Find least-recently-used Chrome processes and suspend them
            let excess = chrome_count - MAX_TABS;
            show_notification(
                "Tab Limiter",
                &format!("Suspended {} inactive Chrome tabs", excess),
            );
            SUSPENDED_TABS += excess;
            report_metric("tabs_suspended", SUSPENDED_TABS as f64);
        }
    }
}
```

### 6. Configuration

```toml
[plugins]
enabled = true
plugin_dir = "~/.ruvector/plugins"
registry_url = "https://plugins.ruvector.dev/api/v1"
auto_update = true
max_tick_ms = 100         # Kill plugins exceeding this per tick
max_memory_mb = 64        # WASM memory limit per plugin

[plugins.installed.chrome-tab-limiter]
enabled = true
version = "1.0.0"
config = { max_tabs = 30 }

[plugins.installed.thinkpad-thermal]
enabled = true
version = "2.1.0"
config = { fan_curve = "quiet" }
```

## Consequences

### Positive
- Transforms RuVector into an extensible platform
- Community can add hardware-specific optimizations
- WASM sandboxing provides security isolation
- Any language can target WASM (Rust, C, Go, AssemblyScript)
- Wasmer already compiled in - minimal additional code
- Plugin marketplace creates ecosystem and community

### Negative
- Plugin API must be stable (breaking changes affect community)
- WASM has overhead vs native code (~2-5x slower)
- Plugin quality varies (need review/rating system)
- Capability approval UX must be clear and non-annoying
- Registry infrastructure requires hosting

### Security Considerations

#### Mandatory Code Signing
```rust
pub struct PluginSecurity {
    /// Ed25519 signature verification for all marketplace plugins
    pub require_signature: bool,
    /// Plugins requesting dangerous capabilities MUST be code-signed
    pub dangerous_requires_signing: bool,
    /// RuVector team verification for top-tier trust
    pub verified_publisher_keys: Vec<Ed25519PublicKey>,
}

// Trust levels:
// 1. Verified (RuVector team signed) - all capabilities available
// 2. Signed (community, valid signature) - standard capabilities
// 3. Unsigned (local development only) - read-only capabilities only
```

#### Wasmer Fuel Metering (CPU Limits)
```rust
// Use Wasmer's deterministic fuel metering instead of post-hoc timing
let mut store = wasmer::Store::new_with_config(
    wasmer::CompilerConfig::new()
        .middleware(wasmer_middlewares::Metering::new(
            10_000_000, // 10M fuel units per tick (~100ms equivalent)
            cost_fn,    // Per-instruction cost function
        ))
);
// Plugin execution deterministically halts when fuel exhausted
// No timing-based race conditions
```

#### AIDefence Integration
```rust
// All plugin outputs scanned through AIDefence WASM gateway
impl PluginManager {
    fn execute_plugin_action(&self, action: PluginAction) -> Result<(), String> {
        // 1. Validate action against approved capabilities
        // 2. Scan action parameters through AIDefence for injection
        // 3. Execute only if both checks pass
        let scan = self.aidefence.validate_action(&action);
        if !scan.is_safe {
            tracing::warn!("Plugin action blocked by AIDefence: {:?}", scan.threats_detected);
            return Err("Action blocked by security scan".into());
        }
        self.execute_action_inner(action)
    }
}
```

#### Additional Plugin Security Measures
- **Memory isolation**: Each plugin runs in a separate Wasmer `Store` with 64MB memory limit
- **No network by default**: `NetworkAccess` capability requires code signing + explicit user approval + AIDefence URL scanning
- **Registry write protection**: `RegistryWrite` only available to Verified (RuVector-signed) plugins
- **No `RunCommand`**: `RunCommand` capability removed entirely. Plugins must use the structured host API.
- **WASM module integrity**: SHA-256 hash verified on load; hash pinned in manifest
- **Plugin isolation**: Plugins cannot communicate with each other; no shared memory
- **Automatic disable**: Plugins exceeding resource limits 3 times are auto-disabled with user notification

### Risks
- Malicious plugins despite sandboxing (mitigated: capability system + code signing + AIDefence)
- Plugins causing system instability (mitigated: fuel metering + memory limits + auto-disable)
- API surface area increases attack surface (mitigated: minimal host API, closed enum actions)
- Community management overhead

## Implementation Plan

### Phase 1: Runtime
- [ ] WASM plugin loading and execution
- [ ] Host function ABI (system queries)
- [ ] Capability-based sandboxing
- [ ] Plugin timeout and memory limits

### Phase 2: SDK
- [ ] Rust plugin SDK crate (`ruvector-plugin-sdk`)
- [ ] AssemblyScript plugin template
- [ ] Plugin build tooling (`cargo ruvector-plugin build`)

### Phase 3: Marketplace
- [ ] Local plugin installation from .wasm files
- [ ] Plugin registry API
- [ ] Plugin browser in Control Center
- [ ] Rating and review system

### Phase 4: Community
- [ ] Plugin submission workflow
- [ ] Code signing for verified plugins
- [ ] Plugin documentation generator
- [ ] Example plugins repository

## References

- [Wasmer Runtime](https://wasmer.io/)
- [WASM Component Model](https://component-model.bytecodealliance.org/)
- [Plugin security best practices](https://webassembly.org/docs/security/)
- [VS Code Extension Marketplace](https://marketplace.visualstudio.com/) (inspiration)
