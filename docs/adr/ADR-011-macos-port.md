# ADR-011: macOS Platform Port

## Status
**Accepted**

## Date
2026-01-28

## Context

RuVector Memory Optimizer is currently Windows-only, utilizing Windows-specific APIs for:
- Memory status via `MEMORYSTATUSEX`
- Process working set trimming via `SetProcessWorkingSetSizeEx`
- Admin privilege detection via `TokenElevation`
- System tray via Windows-specific `tray-icon` implementation

We need to support macOS (specifically Apple Silicon M4) to:
1. Enable development/testing on Mac Mini (100.123.117.38 via Tailscale)
2. Provide memory optimization for macOS users
3. Leverage Apple Silicon's unified memory architecture
4. Support the growing macOS developer market

## Decision

### 1. Platform Abstraction Layer

Create a unified `Platform` trait that abstracts OS-specific operations:

```rust
pub trait MemoryOptimizer: Send + Sync {
    fn get_memory_status(&self) -> Result<MemoryStatus, String>;
    fn optimize(&self, aggressive: bool) -> Result<OptimizationResult, String>;
    fn has_admin_privileges(&self) -> bool;
}
```

### 2. macOS Implementation Strategy

| Windows Feature | macOS Equivalent |
|-----------------|------------------|
| `MEMORYSTATUSEX` | `host_statistics64()` via Mach API |
| `SetProcessWorkingSetSizeEx` | `madvise(MADV_PAGEOUT)` / `sudo purge` |
| Admin check | `geteuid() == 0` |
| Working set trim | Memory pressure notifications + `jetsam` hints |
| System tray | Native macOS menu bar via `objc` bindings |

### 3. Module Structure

```
src/
├── platform/
│   ├── mod.rs          # Platform trait + factory
│   ├── windows.rs      # Windows implementation (current code)
│   └── macos.rs        # macOS implementation
├── macos/
│   ├── mod.rs          # macOS-specific exports
│   ├── memory.rs       # Mach VM APIs
│   ├── process.rs      # BSD process APIs
│   └── safety.rs       # macOS protected processes
└── ...
```

### 4. macOS Memory Optimization Techniques

1. **Memory Pressure Simulation**
   - Use `memory_pressure` CLI tool
   - Trigger system memory cleanup

2. **Process Memory Hints**
   - `madvise(MADV_DONTNEED)` - hint pages not needed
   - `madvise(MADV_FREE)` - mark pages as freeable

3. **Unified Memory Considerations**
   - Apple Silicon shares RAM between CPU/GPU
   - Monitor Metal memory usage via `MTLDevice`
   - Respect GPU workloads (especially for Ollama)

4. **Jetsam Integration**
   - macOS uses `jetsam` for memory pressure management
   - Respect system priorities
   - Don't interfere with system decisions

### 5. Cargo.toml Changes

```toml
[target.'cfg(target_os = "macos")'.dependencies]
mach2 = "0.4"           # Mach kernel APIs
core-foundation = "0.9" # CF types
objc = "0.2"            # Objective-C runtime
cocoa = "0.25"          # macOS UI bindings

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [...] }
winapi = { version = "0.3", features = [...] }
```

### 6. Protected Processes (macOS)

```rust
const PROTECTED_PROCESSES: &[&str] = &[
    "kernel_task",
    "launchd",
    "WindowServer",
    "loginwindow",
    "Finder",
    "Dock",
    "SystemUIServer",
    "coreaudiod",
    "bluetoothd",
    "mds_stores",
    // Apple Silicon specific
    "AMPDeviceDiscoveryAgent",
    "gpu_driver_spawn",
];
```

### 7. Feature Flags

```toml
[features]
default = []
macos-menubar = ["cocoa", "objc"]      # Menu bar icon
macos-metal = ["metal"]                 # GPU memory tracking
apple-silicon = ["macos-metal"]         # M-series optimizations
```

## Consequences

### Positive
- Cross-platform support increases user base
- Development can occur on Mac Mini (M4)
- Better testing coverage across platforms
- Unified memory model on Apple Silicon is simpler
- Can leverage Metal Performance Shaders for neural ops

### Negative
- Increased maintenance burden (2 platforms)
- macOS memory management is less aggressive by design
- Some Windows features have no direct equivalent
- Code signing required for distribution
- Apple's restrictions on memory manipulation

### Neutral
- Different performance characteristics expected
- macOS users may see less dramatic improvements
- Need separate release binaries

## Implementation Plan

### Phase 1: Core Port (Week 1)
- [ ] Create `src/macos/` module structure
- [ ] Implement `MacMemoryOptimizer` using sysinfo crate
- [ ] Add platform detection in main.rs
- [ ] Update Cargo.toml for conditional compilation

### Phase 2: Memory APIs (Week 1-2)
- [ ] Implement Mach VM statistics gathering
- [ ] Add `madvise` hints for process memory
- [ ] Create memory pressure monitoring

### Phase 3: Testing (Week 2)
- [ ] Deploy to Mac Mini via Tailscale
- [ ] Run benchmarks comparing platforms
- [ ] Validate safety mechanisms

### Phase 4: Polish (Week 2-3)
- [ ] Menu bar icon (optional)
- [ ] Apple Silicon optimizations
- [ ] Documentation updates

## Test Plan

1. **Remote Testing via Tailscale**
   ```bash
   # From Windows
   ssh cohen@100.123.117.38
   cd ~/workspace/ruvector-memopt
   cargo build --release
   ./target/release/ruvector-memopt status
   ```

2. **Benchmark Suite**
   ```bash
   ./target/release/ruvector-memopt bench --iterations 1000
   ./target/release/ruvector-memopt bench --advanced
   ```

3. **Memory Pressure Test**
   ```bash
   # Create memory pressure
   stress --vm 2 --vm-bytes 4G &
   ./target/release/ruvector-memopt optimize
   ```

## References

- [Mach VM APIs](https://developer.apple.com/documentation/kernel/mach_vm)
- [madvise(2)](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/madvise.2.html)
- [Memory Pressure](https://developer.apple.com/documentation/os/memorymanagement)
- [Jetsam](https://apple.stackexchange.com/questions/356105/what-is-jetsam-and-what-can-i-do-about-it)

## Notes

The macOS implementation will be more conservative than Windows because:
1. macOS already has excellent memory management
2. Apple restricts low-level memory manipulation
3. The unified memory architecture is self-optimizing

Focus should be on:
- Monitoring and reporting
- Gentle hints via madvise
- Application-level cleanup
- Respecting system decisions
