# ADR-023: Bloatware & Telemetry Silencer

## Status
**Proposed**

## Date
2026-01-28

## Context

Modern Windows installations ship with dozens of pre-installed applications ("bloatware") and extensive telemetry services that consume RAM, CPU, disk I/O, and network bandwidth - often without the user's knowledge. OEMs add their own layer of preinstalled software on top of Microsoft's. Together, these can consume 500MB-2GB of RAM and generate continuous background I/O and network traffic.

Users who care about performance (gamers, developers, content creators) manually disable these through registry edits, Group Policy, PowerShell scripts, and third-party tools - a tedious, error-prone process that can break Windows Update or other functionality if done incorrectly.

RuVector can safely identify and silence these processes using its existing process monitoring, PageRank scoring, and neural learning infrastructure. Unlike aggressive "debloat" scripts that permanently remove components, RuVector takes a reversible approach: disable, don't delete.

### Existing Infrastructure
- Process monitoring (`sysinfo` crate)
- PageRank-based importance scoring (existing algorithm)
- Named pipe IPC for service-to-tray communication
- TOML configuration system
- Banner notification system (for user approval)

## Decision

### 1. Bloatware Database

```rust
pub struct BloatwareDatabase {
    entries: Vec<BloatwareEntry>,
    user_overrides: HashMap<String, UserDecision>,
    version: String,
}

pub struct BloatwareEntry {
    pub id: String,
    pub name: String,                    // Human-readable name
    pub category: BloatwareCategory,
    pub identifiers: Vec<Identifier>,    // How to find it
    pub impact: ResourceImpact,
    pub safety: SafetyLevel,
    pub action: RecommendedAction,
    pub description: String,
    pub reversible: bool,                // Can we undo this?
}

pub enum BloatwareCategory {
    OemPreinstall,          // HP Support Assistant, Dell SupportAssist, Lenovo Vantage
    MicrosoftBloat,         // Candy Crush, TikTok, Spotify (pre-installed)
    Telemetry,              // DiagTrack, Connected User Experiences, CEIP
    BackgroundService,      // Cortana, Xbox Game Bar (when not gaming)
    StartupJunk,            // Updaters, tray icons, "helper" processes
    ScheduledTask,          // Recurring background tasks
    BrowserExtension,       // Pre-installed browser extensions
}

pub enum Identifier {
    ProcessName(String),          // "HpTouchpointAnalyticsClient.exe"
    ServiceName(String),          // "DiagTrack"
    ScheduledTaskPath(String),    // "\Microsoft\Windows\Application Experience\..."
    AppxPackage(String),          // "Microsoft.BingWeather_8wekyb3d8bbwe"
    RegistryKey(String),          // "HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection"
    StartupEntry(String),         // Registry Run key or Startup folder
}

pub struct ResourceImpact {
    pub ram_mb: f64,              // Typical RAM usage
    pub cpu_percent: f64,         // Average CPU usage
    pub disk_io_mb_per_hour: f64, // Disk I/O generated
    pub network_mb_per_hour: f64, // Network traffic
    pub startup_delay_ms: u64,    // Added boot time
}

pub enum SafetyLevel {
    Safe,           // No system impact, purely cosmetic/telemetry
    Moderate,       // Some features may stop working (e.g., Cortana)
    Caution,        // Could affect Windows Update or Store
    Expert,         // Only for advanced users who understand consequences
}
```

### 2. Telemetry Silencer

```rust
pub struct TelemetrySilencer {
    database: BloatwareDatabase,
    firewall: Option<FirewallController>,
    hosts_file: Option<HostsFileManager>,
}

pub struct TelemetryEndpoint {
    pub domain: String,
    pub purpose: String,
    pub data_collected: Vec<String>,
    pub block_safe: bool,
}

impl TelemetrySilencer {
    /// Known Microsoft telemetry endpoints
    fn telemetry_domains() -> Vec<TelemetryEndpoint> {
        vec![
            TelemetryEndpoint {
                domain: "vortex.data.microsoft.com".into(),
                purpose: "Windows telemetry data collection".into(),
                data_collected: vec!["App usage", "Crash reports", "Hardware info"],
                block_safe: true,
            },
            TelemetryEndpoint {
                domain: "settings-win.data.microsoft.com".into(),
                purpose: "Telemetry settings sync".into(),
                data_collected: vec!["Device settings", "Feature flags"],
                block_safe: true,
            },
            TelemetryEndpoint {
                domain: "watson.telemetry.microsoft.com".into(),
                purpose: "Error reporting (Watson)".into(),
                data_collected: vec!["Crash dumps", "Error logs"],
                block_safe: true,
            },
            // ... more endpoints
        ]
    }

    /// Silence telemetry via multiple methods (defense in depth)
    pub fn silence_telemetry(&self, level: TelemetryLevel) -> Vec<SilenceAction> {
        let mut actions = vec![];

        match level {
            TelemetryLevel::Minimal => {
                // Registry: Set telemetry to Security level (0)
                actions.push(SilenceAction::SetRegistry {
                    key: r"HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection".into(),
                    value: "AllowTelemetry".into(),
                    data: RegistryData::Dword(0),
                });
                // Disable Connected User Experiences service
                actions.push(SilenceAction::DisableService {
                    name: "DiagTrack".into(),
                    display_name: "Connected User Experiences and Telemetry".into(),
                });
                // Disable dmwappushservice
                actions.push(SilenceAction::DisableService {
                    name: "dmwappushservice".into(),
                    display_name: "Device Management WAP Push".into(),
                });
            }
            TelemetryLevel::Aggressive => {
                // All of Minimal, plus:
                // Block telemetry domains via Windows Firewall
                for endpoint in Self::telemetry_domains() {
                    if endpoint.block_safe {
                        actions.push(SilenceAction::BlockDomain {
                            domain: endpoint.domain,
                            method: BlockMethod::WindowsFirewall,
                        });
                    }
                }
                // Disable scheduled telemetry tasks
                actions.push(SilenceAction::DisableScheduledTask {
                    path: r"\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser".into(),
                });
                actions.push(SilenceAction::DisableScheduledTask {
                    path: r"\Microsoft\Windows\Application Experience\ProgramDataUpdater".into(),
                });
                actions.push(SilenceAction::DisableScheduledTask {
                    path: r"\Microsoft\Windows\Customer Experience Improvement Program\Consolidator".into(),
                });
            }
            TelemetryLevel::Paranoid => {
                // All of Aggressive, plus:
                // Hosts file blocking
                for endpoint in Self::telemetry_domains() {
                    actions.push(SilenceAction::BlockDomain {
                        domain: endpoint.domain,
                        method: BlockMethod::HostsFile,
                    });
                }
                // Disable additional background services
                actions.push(SilenceAction::DisableService {
                    name: "WerSvc".into(),
                    display_name: "Windows Error Reporting".into(),
                });
            }
        }

        actions
    }
}

pub enum TelemetryLevel {
    Minimal,     // Registry-only changes
    Aggressive,  // Registry + Firewall + Scheduled tasks
    Paranoid,    // All methods including hosts file
}
```

### 3. Bloatware Removal Engine

```rust
pub struct BloatwareRemover {
    database: BloatwareDatabase,
    undo_log: Vec<UndoEntry>,
}

pub enum SilenceAction {
    // Process-level
    StopProcess { name: String },
    DisableStartupEntry { name: String, location: StartupLocation },

    // Service-level
    DisableService { name: String, display_name: String },
    SetServiceManual { name: String, display_name: String },

    // Package-level
    RemoveAppxPackage { package: String, name: String },
    DeprovisionAppx { package: String },  // Prevent reinstall

    // Task Scheduler
    DisableScheduledTask { path: String },

    // Registry
    SetRegistry { key: String, value: String, data: RegistryData },

    // Network
    BlockDomain { domain: String, method: BlockMethod },

    // Notification
    NotifyUser { title: String, message: String },
}

pub struct UndoEntry {
    pub action: SilenceAction,
    pub original_state: OriginalState,
    pub timestamp: DateTime<Local>,
}

impl BloatwareRemover {
    /// Remove bloatware with full undo capability
    pub fn remove(&mut self, entry: &BloatwareEntry) -> Result<Vec<SilenceAction>, String> {
        let mut actions = vec![];

        for identifier in &entry.identifiers {
            match identifier {
                Identifier::AppxPackage(pkg) => {
                    // Save current state for undo
                    self.undo_log.push(UndoEntry {
                        action: SilenceAction::RemoveAppxPackage {
                            package: pkg.clone(),
                            name: entry.name.clone(),
                        },
                        original_state: OriginalState::AppxInstalled(pkg.clone()),
                        timestamp: Local::now(),
                    });
                    actions.push(SilenceAction::RemoveAppxPackage {
                        package: pkg.clone(),
                        name: entry.name.clone(),
                    });
                }
                Identifier::ServiceName(svc) => {
                    let current = self.get_service_start_type(svc);
                    self.undo_log.push(UndoEntry {
                        action: SilenceAction::DisableService {
                            name: svc.clone(),
                            display_name: entry.name.clone(),
                        },
                        original_state: OriginalState::ServiceStartType(svc.clone(), current),
                        timestamp: Local::now(),
                    });
                    actions.push(SilenceAction::DisableService {
                        name: svc.clone(),
                        display_name: entry.name.clone(),
                    });
                }
                Identifier::ScheduledTaskPath(path) => {
                    actions.push(SilenceAction::DisableScheduledTask {
                        path: path.clone(),
                    });
                }
                Identifier::StartupEntry(name) => {
                    actions.push(SilenceAction::DisableStartupEntry {
                        name: name.clone(),
                        location: self.find_startup_location(name),
                    });
                }
                _ => {}
            }
        }

        Ok(actions)
    }

    /// Undo all changes for a specific entry
    pub fn undo(&mut self, entry_id: &str) -> Result<(), String> {
        let entries: Vec<UndoEntry> = self.undo_log.iter()
            .filter(|e| /* matches entry_id */)
            .cloned()
            .collect();

        for undo in entries.iter().rev() {
            match &undo.original_state {
                OriginalState::ServiceStartType(name, start_type) => {
                    self.restore_service(name, start_type)?;
                }
                OriginalState::AppxInstalled(pkg) => {
                    // Can reinstall from Store
                    self.reinstall_appx(pkg)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}
```

### 4. Scan & Report UI

```
+---------------------------------------------------------+
|  Bloatware & Telemetry Scanner                           |
|                                                          |
|  System Impact: 1.4 GB RAM · 3.2% CPU · 840 MB/day net |
|                                                          |
|  Category              Items  RAM     Action             |
|  ─────────────────────────────────────────────────       |
|  OEM Preinstalls        8     312 MB  [Remove All]       |
|    HP Support Asst.           142 MB  [Remove] [Keep]    |
|    HP Audio Switch             48 MB  [Remove] [Keep]    |
|    HP System Event Util        38 MB  [Remove] [Keep]    |
|    ...                                                   |
|                                                          |
|  Microsoft Bloatware    12    198 MB  [Remove All]       |
|    Candy Crush Saga            0 MB   [Remove] [Keep]    |
|    Clipchamp                   0 MB   [Remove] [Keep]    |
|    Microsoft News             42 MB   [Remove] [Keep]    |
|    ...                                                   |
|                                                          |
|  Telemetry Services     6     84 MB   [Silence All]      |
|    DiagTrack                  28 MB   [Disable] [Keep]   |
|    dmwappushservice           12 MB   [Disable] [Keep]   |
|    ...                                                   |
|                                                          |
|  Background Junk        15   412 MB   [Clean All]        |
|    Windows Search            248 MB   [Disable] [Keep]   |
|    Cortana                    86 MB   [Disable] [Keep]   |
|    ...                                                   |
|                                                          |
|  Telemetry Level: [Minimal ▾]                            |
|                                                          |
|  [Scan Again]  [Apply Selected]  [Undo All Changes]     |
+---------------------------------------------------------+
```

### 5. Configuration

```toml
[bloatware]
enabled = true
scan_on_startup = true
auto_silence = false          # Require user approval

[bloatware.telemetry]
level = "minimal"             # minimal | aggressive | paranoid
block_method = "firewall"     # firewall | hosts | both

[bloatware.categories]
oem_preinstalls = "prompt"    # remove | prompt | ignore
microsoft_bloat = "prompt"
telemetry = "silence"
background_junk = "prompt"
startup_junk = "prompt"

[bloatware.whitelist]
# Never touch these even if detected as bloatware
processes = ["OneDrive.exe"]
services = ["WSearch"]        # Keep Windows Search
packages = []

[bloatware.blacklist]
# Always remove these on scan
packages = ["Microsoft.BingWeather", "Microsoft.GetHelp"]
```

## Consequences

### Positive
- Reclaims 500MB-2GB RAM and reduces background CPU usage
- Reduces telemetry data transmission (privacy benefit)
- Faster boot times by removing startup bloatware
- Fully reversible - undo log preserves original state
- Database approach is safer than generic "debloat" scripts
- OEM-specific bloatware detection covers major vendors

### Negative
- Bloatware database requires ongoing maintenance
- Some users want features we classify as bloatware (Cortana, Xbox)
- Telemetry blocking may violate corporate policies
- AppX removal may require re-provisioning to restore
- Different OEM builds have different preinstalled software

### Security Considerations

#### Corporate/Enterprise Detection
```rust
pub struct EnvironmentDetector {
    pub is_domain_joined: bool,         // Part of Active Directory
    pub is_azure_ad_joined: bool,       // Azure AD managed
    pub has_mdm: bool,                  // Mobile Device Management (Intune)
    pub edition: WindowsEdition,        // Home, Pro, Enterprise, Education
    pub group_policy_active: bool,      // GP objects applied
}

impl EnvironmentDetector {
    /// On managed machines, restrict bloatware/telemetry actions
    pub fn get_restrictions(&self) -> BloatwareRestrictions {
        if self.is_domain_joined || self.has_mdm {
            BloatwareRestrictions {
                can_modify_services: false,       // IT manages services
                can_modify_telemetry: false,      // Compliance requirement
                can_remove_appx: true,            // User apps only
                can_modify_firewall: false,       // Corp firewall policies
                can_modify_hosts: false,          // DNS managed centrally
                warning: Some("Corporate-managed device detected. Some actions restricted."),
            }
        } else {
            BloatwareRestrictions::unrestricted()
        }
    }
}
```

- **Security software protection**: Windows Defender (`WinDefend`, `MsMpEng.exe`), Windows Firewall (`mpssvc`), and Windows Update (`wuauserv`) are hardcoded in a never-touch list; cannot be added to bloatware database even by user override
- **Corporate compliance**: Domain-joined and MDM-managed machines automatically restrict telemetry and service modifications
- **Hosts file integrity**: Hosts file modifications are atomic (write to temp, rename) with rollback; file permission changes are not made
- **Firewall rule namespacing**: All firewall rules created by RuVector use a `RuVector-` prefix for easy identification and bulk removal
- **Undo log integrity**: Undo log entries signed with HMAC-SHA256; tampered entries flagged on restore
- **Windows Update protection**: Actions that could break Windows Update (disabling `wuauserv`, blocking `*.update.microsoft.com`) are in `Expert` safety level only, with explicit double-confirmation

### Risks
- Overly aggressive removal could break Windows Update (mitigated: `wuauserv` is in never-touch list)
- Disabling wrong services could cause boot issues (mitigated: safety levels + never-touch list)
- Corporate/education editions may have group policy conflicts (mitigated: automatic corporate detection)
- Microsoft may change telemetry endpoints between Windows updates
- Legal considerations in some jurisdictions regarding telemetry blocking

## Implementation Plan

### Phase 1: Database
- [ ] Curate bloatware database (top 50 entries)
- [ ] OEM detection (HP, Dell, Lenovo, ASUS, Acer)
- [ ] Resource impact measurement per entry
- [ ] Safety classification

### Phase 2: Scanner
- [ ] Process, service, and AppX enumeration
- [ ] Startup entry discovery
- [ ] Telemetry endpoint identification
- [ ] Impact calculation

### Phase 3: Removal
- [ ] Service disable/enable with undo log
- [ ] AppX removal with deprovision
- [ ] Scheduled task disable
- [ ] Registry modification with backup
- [ ] Firewall rule management

### Phase 4: UI
- [ ] Scan results page in Control Center
- [ ] Category grouping with bulk actions
- [ ] Undo history viewer
- [ ] Telemetry level selector

## References

- [Windows Telemetry Endpoints](https://learn.microsoft.com/en-us/windows/privacy/manage-connections-from-windows-operating-system-components-to-microsoft-services)
- [AppX Package Management](https://learn.microsoft.com/en-us/powershell/module/appx/)
- [Service Control Manager](https://learn.microsoft.com/en-us/windows/win32/services/service-control-manager)
- [Task Scheduler API](https://learn.microsoft.com/en-us/windows/win32/taskschd/task-scheduler-start-page)
