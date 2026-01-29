# ADR-012: Control Center UI Framework

## Status
**Proposed**

## Date
2026-01-28

## Context

RuVector MemOpt currently operates through two interfaces:
1. **CLI** (`ruvector-memopt optimize`, `status`, `bench`) - power users
2. **System tray** (`ruvector-memopt-tray`) - background monitoring with basic menu

Neither provides a comprehensive configuration experience. Users cannot visualize system health, tweak optimization parameters, manage profiles, or access advanced features without reading documentation. PowerToys by Microsoft demonstrated that a unified settings GUI dramatically increases adoption of system optimization tools.

RuVector already has macOS-style custom Win32 dialog rendering (`src/tray/dialog.rs`) using GDI, DWM rounded corners, and Segoe UI typography. This proves custom Win32 UI is viable without heavyweight frameworks. The question is how to scale this to a full configuration application.

### Requirements
- Single-window settings application with sidebar navigation
- Real-time system monitoring dashboard
- Configuration persistence (TOML-based, existing `TraySettings` pattern)
- Cross-platform (Windows primary, macOS secondary)
- Sub-50MB binary (no Electron, no web runtime)
- Native look: macOS-inspired clean design on both platforms
- Launchable from tray menu or standalone
- IPC with running tray/service for live data

## Decision

### 1. UI Technology: Custom Win32 + GDI on Windows, AppKit on macOS

Reject: Electron (100MB+), Tauri (WASM complexity), egui/iced (immature, non-native feel).

Accept: Extend the existing `dialog.rs` pattern into a full windowed application. This gives us:
- Zero additional dependencies on Windows (already have `windows` crate)
- Native rendering performance
- Complete control over the visual language
- Sub-5MB addition to binary size

On macOS, use `objc2` + AppKit bindings for native Cocoa UI.

### 2. Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   RuVector Control Center                    │
├──────────┬──────────────────────────────────────────────────┤
│          │                                                  │
│ Sidebar  │              Content Area                        │
│ (Nav)    │  ┌──────────────────────────────────────────┐   │
│          │  │  Active Page                              │   │
│ ● Home   │  │                                          │   │
│ ◆ Memory │  │  (Rendered by page module)               │   │
│ ◆ CPU    │  │                                          │   │
│ ◆ Disk   │  │                                          │   │
│ ◆ Network│  └──────────────────────────────────────────┘   │
│ ◆ GPU    │                                                  │
│          │                                                  │
│ ○ Profiles│                                                 │
│ ○ Focus  │                                                  │
│ ○ Startup│                                                  │
│ ○ Plugins│                                                  │
│          │                                                  │
│ ◇ Timeline│                                                 │
│ ◇ Costs  │                                                  │
│          │                                                  │
│ ⚙ Settings│                                                │
└──────────┴──────────────────────────────────────────────────┘
```

### 3. Module Structure

```
src/
├── ui/
│   ├── mod.rs              # UI engine, message loop, window management
│   ├── controls.rs         # Reusable control library (buttons, sliders, toggles, charts)
│   ├── theme.rs            # Color palette, font sizes, spacing constants
│   ├── layout.rs           # Layout engine (flex-like row/column)
│   ├── sidebar.rs          # Navigation sidebar component
│   ├── pages/
│   │   ├── mod.rs          # Page trait + registry
│   │   ├── home.rs         # Dashboard / health score
│   │   ├── memory.rs       # Memory monitoring + optimization
│   │   ├── cpu.rs          # CPU / thermal / process management
│   │   ├── disk.rs         # Disk I/O monitoring
│   │   ├── network.rs      # Network traffic shaping
│   │   ├── gpu.rs          # GPU / VRAM management
│   │   ├── profiles.rs     # Workspace profile management
│   │   ├── focus.rs        # Focus mode configuration
│   │   ├── startup.rs      # Startup optimizer
│   │   ├── plugins.rs      # WASM plugin marketplace
│   │   ├── timeline.rs     # Time-travel system state
│   │   ├── costs.rs        # Power cost calculator
│   │   └── settings.rs     # App settings
│   └── charts/
│       ├── mod.rs          # Chart rendering engine
│       ├── line.rs         # Time-series line charts
│       ├── bar.rs          # Bar charts
│       ├── treemap.rs      # Memory treemap
│       └── gauge.rs        # Circular gauge (health score)
```

### 4. Page Trait

```rust
pub trait Page: Send {
    fn id(&self) -> &str;
    fn title(&self) -> &str;
    fn icon(&self) -> PageIcon;
    fn category(&self) -> PageCategory;

    /// Paint the page content into the given rect
    fn paint(&self, hdc: HDC, rect: &RECT, state: &AppState);

    /// Handle mouse click at (x, y) relative to content area
    fn on_click(&mut self, x: i32, y: i32, state: &mut AppState) -> PageAction;

    /// Handle mouse move for hover effects
    fn on_hover(&mut self, x: i32, y: i32) -> bool; // true = needs repaint

    /// Called every tick (1s) to update data
    fn tick(&mut self, state: &AppState);
}

pub enum PageCategory {
    Monitor,    // ◆ Real-time monitoring
    Tools,      // ○ Configuration tools
    Advanced,   // ◇ Advanced features
    System,     // ⚙ System settings
}
```

### 5. IPC with Tray/Service

The Control Center communicates with the running tray app and/or Windows service via named pipes (Windows) or Unix domain sockets (macOS):

```rust
pub enum IpcMessage {
    // Queries
    GetMemoryStatus,
    GetProcessList,
    GetOptimizationHistory,
    GetHealthScore,

    // Commands
    OptimizeNow { aggressive: bool },
    SetProfile { name: String },
    ToggleFocusMode { enabled: bool },

    // Responses
    MemoryStatus(MemoryStatus),
    ProcessList(Vec<ProcessInfo>),
    HealthScore(u32),
}
```

The existing `interprocess` crate (already in Cargo.toml for Windows) handles this.

#### IPC Security

```rust
/// Named pipe security: restrict connections to same-user sessions
pub struct SecureIpcServer {
    /// Named pipe with DACL restricting to current user SID
    pipe_name: String,
    /// HMAC-SHA256 challenge-response on connection
    auth_secret: [u8; 32],
}

impl SecureIpcServer {
    pub fn new() -> Self {
        // 1. Create named pipe with security descriptor:
        //    - DACL allows only current user SID (no other users/processes)
        //    - Deny NETWORK access (local-only)
        // 2. Generate per-session auth secret stored in memory
        // 3. Validate all incoming IpcMessages against schema
        //    (reject malformed/oversized payloads)
    }
}
```

- **Pipe ACL**: Named pipe DACL restricts connections to the current user SID only
- **Auth handshake**: HMAC-SHA256 challenge-response prevents rogue process injection
- **Input validation**: All `IpcMessage` variants validated against size/type constraints before dispatch
- **No arbitrary execution**: `IpcMessage` is a closed enum - no dynamic command strings

### 6. Configuration Persistence

Extend existing TOML-based `TraySettings`:

```toml
[control_center]
window_x = 100
window_y = 100
window_width = 1000
window_height = 700
last_page = "home"
sidebar_width = 200
theme = "light"  # light | dark | system

[profiles]
active = "development"

[profiles.development]
priority_apps = ["code.exe", "cargo.exe", "node.exe"]
suppress_apps = ["OneDrive.exe", "Teams.exe"]
power_plan = "high_performance"
memory_threshold = 80

[profiles.gaming]
priority_apps = ["steam.exe"]
suppress_apps = ["code.exe", "OneDrive.exe", "Teams.exe", "Slack.exe"]
power_plan = "ultimate"
memory_threshold = 90
disable_indexing = true
```

### 7. Chart Rendering

Custom GDI chart rendering for real-time data visualization:

```rust
pub struct LineChart {
    data: VecDeque<f64>,     // Ring buffer of values
    max_points: usize,       // Visible window size
    y_min: f64,
    y_max: f64,
    color: COLORREF,
    fill_alpha: u8,          // Area fill (0 = line only)
    label: String,
}

impl LineChart {
    pub fn paint(&self, hdc: HDC, rect: &RECT) {
        // Anti-aliased line rendering via GDI+
        // Grid lines, axis labels, hover tooltips
    }
}
```

### 8. Keyboard Navigation

Full keyboard support following Windows accessibility guidelines:
- `Tab` / `Shift+Tab` - navigate between sidebar items
- `Enter` - activate selected item
- `Ctrl+1..9` - jump to page by index
- `F5` - force refresh
- `Escape` - close / back
- `Ctrl+O` - optimize now
- `Ctrl+P` - switch profile

## Consequences

### Positive
- Professional UI increases user trust and adoption
- All features discoverable in one place
- Real-time visualization makes optimization tangible
- Configuration UI eliminates need to edit TOML manually
- Zero additional runtime dependencies
- Native performance, instant startup

### Negative
- Significant development effort for custom UI toolkit
- GDI text rendering lacks sub-pixel antialiasing (mitigated by GDI+ for charts)
- Manual layout management vs. auto-layout frameworks
- Two separate UI codebases (Win32 + AppKit)
- Accessibility requires manual implementation (screen reader support)

### Security Considerations
- **IPC authentication**: Named pipe ACL restricts to current user SID; HMAC-SHA256 handshake prevents impersonation
- **Input validation**: All IPC messages validated against closed enum schema; oversized payloads rejected
- **No arbitrary execution**: IPC commands are a fixed enum, not dynamic strings
- **GDI content sanitization**: Rendered text from process names/paths is length-limited and filtered for control characters
- **Memory safety**: All Win32 handle resources wrapped in RAII (`OwnedHandle`) to prevent leaks

### Risks
- Custom UI can feel non-standard if not carefully designed
- DPI scaling requires explicit handling
- Multiple monitor support needs testing
- Dark mode detection requires Windows 10 1809+ registry check

## Implementation Plan

### Phase 1: Window Shell
- [ ] Create `src/ui/mod.rs` with main window, sidebar, content area
- [ ] Implement page navigation (sidebar click -> content swap)
- [ ] DPI-aware rendering with `GetDpiForWindow`
- [ ] Add "Control Center" menu item to tray

### Phase 2: Home Dashboard
- [ ] Health score gauge (circular)
- [ ] Quick action buttons
- [ ] Recent activity feed
- [ ] Memory/CPU mini-charts

### Phase 3: Monitor Pages
- [ ] Memory page with treemap
- [ ] CPU page with per-core graph
- [ ] Process list with PageRank scores

### Phase 4: Tool Pages
- [ ] Profiles page
- [ ] Startup optimizer
- [ ] Focus mode configuration

### Phase 5: Advanced Pages
- [ ] Timeline / time-travel
- [ ] Plugin marketplace
- [ ] Settings / about

## Design Language

| Element | Specification |
|---------|--------------|
| Background | `#FFFFFF` (light) / `#1E1E1E` (dark) |
| Sidebar | `#F5F5F5` / `#252526` |
| Accent | `#2090FF` (blue) |
| Success | `#00C850` (green) |
| Warning | `#FFA500` (orange) |
| Error | `#FF4444` (red) |
| Font | Segoe UI (Win) / SF Pro (macOS) |
| Title | 20px Semibold |
| Body | 14px Regular |
| Caption | 12px Regular |
| Corner radius | 8px (Win11), 0px (Win10) |
| Spacing unit | 8px grid |
| Sidebar width | 200px |
| Min window | 800 x 600 |

## References

- [Microsoft PowerToys](https://github.com/microsoft/PowerToys)
- [Windows UI Library design guidelines](https://learn.microsoft.com/en-us/windows/apps/design/)
- [Win32 GDI custom drawing](https://learn.microsoft.com/en-us/windows/win32/gdi/painting-and-drawing)
- [DWM window attributes](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)


# design 
Design Style Prompt

Design a technical infographic that explains a living, self maintaining AI world model. The visual language should feel structural, alive, and restrained. This is not a dashboard and not a poster. It should feel like a system diagram that quietly thinks.

Visual tone
Calm, deliberate, intelligent. No sci fi tropes. No faces. No mascots. Avoid artificial glow effects. Prefer subtle gradients and physical metaphors such as pressure, flow, and balance.

Color and texture
Primary palette uses deep navy, graphite, and charcoal. Secondary accents use muted cyan and desaturated amber. Background is dark, matte, and lightly textured like fine paper or anodized metal. Contrast comes from spacing, hierarchy, and geometry rather than brightness.

Overall layout
The composition is radial but asymmetrical, suggesting motion without chaos. At the center is a quiet core labeled World Model. Around it are four concentric but slightly offset layers labeled:

Vectors
Graphs
Attention
Coherence

Each layer partially overlaps the next, implying interaction rather than isolation.

Visual metaphors by layer

Vectors
Shown as sparse points in a curved coordinate field. Distances are visible. Some points drift slowly to suggest learning and adaptation.

Graphs
Overlaid node edge structures with varying edge thickness and tension. Some edges fade or subtly rewire over time.

Attention
Represented as directional flow lines that brighten and dim as they route energy or focus across the graph. Flow feels intentional and selective.

Coherence
Shown as structural stress lines or cut boundaries. Use soft heat shading or fracture like patterns to show disagreement building and resolving.

Runtime loop
At the base, include a thin circular loop:
Ingest → Embed → Link → Measure → Gate → Act → Settle
The loop should feel like a heartbeat or control cycle, not a linear pipeline.

Typography
Modern sans serif. Technical but human. Headers are medium weight. Labels are light. Text is minimal and precise. No paragraphs inside the graphic.

Motion guidance
If animated, motion is slow and intentional. Gentle drift, fades, and flow emphasis only. Nothing pulses aggressively or draws attention to itself.

Overall feeling
This should look like an engineering whitepaper that learned to breathe. Structure over spectacle. Meaning over decoration. A world model that remains intact while everything within it moves.