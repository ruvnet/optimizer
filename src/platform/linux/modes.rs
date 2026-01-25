//! Linux Mode Detection - Game Mode and Focus Mode
//!
//! Detects running games and focus-worthy applications on Linux systems.
//! Uses /proc filesystem for process detection with optional X11 window focus support.

use std::collections::HashSet;
use std::fs;

// ============================================================================
// Process Lists - Const arrays for efficiency
// ============================================================================

/// Steam and Valve-related processes
const STEAM_PROCESSES: &[&str] = &[
    "steam",
    "steamwebhelper",
    "steam-runtime",
    "steam-runtime-l",
    "steamclient",
    "gameoverlayui",
];

/// Proton/Wine game compatibility layer processes
const WINE_PROCESSES: &[&str] = &[
    "wine",
    "wine64",
    "wine-preloader",
    "wine64-preloader",
    "proton",
    "pressure-vessel",
    "pressure-vessel-wrap",
    "wineserver",
    "winedevice.exe",
    "plugplay.exe",
    "explorer.exe",
    "services.exe",
    "wineboot.exe",
];

/// Native Linux games (popular titles)
const NATIVE_GAMES: &[&str] = &[
    // Minecraft variants
    "minecraft-launcher",
    "java", // Often Minecraft
    "javaw",
    "minecraft",
    "prismlauncher",
    "multimc",
    "atlauncher",
    // Strategy games
    "factorio",
    "rimworld",
    "paradox launcher",
    "stellaris",
    "eu4",
    "hoi4",
    "ck3",
    "victoria3",
    // Valve games
    "dota2",
    "csgo",
    "cs2",
    "hl2_linux",
    "portal2_linux",
    "left4dead2",
    "tf_linux",
    // Other popular native games
    "terraria",
    "stardewvalley",
    "hollowknight",
    "celeste",
    "deadcells",
    "hadesii",
    "hades",
    "slay-the-spire",
    "inscryption",
    "balatro",
    "among-us",
    "valheim",
    "subnautica",
    "rocketleague",
    "psychonauts2",
    "disco-elysium",
    "divinity-original-sin-2",
    "baldursgate3",
    "bg3",
    "cyberpunk2077",
    "witcher3",
];

/// Game launchers and platforms
const GAME_LAUNCHERS: &[&str] = &[
    "lutris",
    "heroic",
    "bottles",
    "gamehub",
    "itch",
    "legendary",
    "minigalaxy",
    "gog-galaxy",
    "pegasus-fe",
    "retroarch",
    "gamescope",
    "mangohud",
];

/// Emulators
const EMULATORS: &[&str] = &[
    // RetroArch / libretro
    "retroarch",
    // Nintendo
    "dolphin-emu",
    "yuzu",
    "ryujinx",
    "citra",
    "cemu",
    "mupen64plus",
    "snes9x",
    "bsnes",
    "mgba",
    "desmume",
    "melonds",
    // Sony
    "pcsx2",
    "rpcs3",
    "ppsspp",
    "duckstation",
    // Other
    "xemu",
    "redream",
    "flycast",
    "mame",
    "dosbox",
    "dosbox-staging",
    "scummvm",
    "fs-uae",
];

/// Video call and conferencing applications
const VIDEO_CALL_APPS: &[&str] = &[
    "zoom",
    "zoom.real",
    "zoomwebviewhost",
    "teams",
    "ms-teams",
    "teams-for-linux",
    "slack",
    "discord",
    "discord-ptb",
    "discord-canary",
    "webcord",
    "skype",
    "skypeforlinux",
    "webex",
    "webexmeetings",
    "webexstart",
    "jitsi",
    "jitsi-meet",
    "signal-desktop",
    "telegram-desktop",
    "element-desktop",
    "wire-desktop",
    "meet",
    "google-meet",
];

/// Web browsers (focus mode when in meetings)
const BROWSERS: &[&str] = &[
    "firefox",
    "firefox-esr",
    "firefox-developer-edition",
    "chromium",
    "chromium-browser",
    "google-chrome",
    "google-chrome-stable",
    "chrome",
    "brave",
    "brave-browser",
    "vivaldi",
    "vivaldi-stable",
    "opera",
    "microsoft-edge",
    "edge",
    "librewolf",
    "waterfox",
    "epiphany",
    "qutebrowser",
    "min",
    "midori",
];

/// IDE and code editors
const IDE_EDITORS: &[&str] = &[
    // VS Code variants
    "code",
    "codium",
    "code-oss",
    "vscodium",
    // JetBrains IDEs
    "idea",
    "idea.sh",
    "intellij-idea",
    "pycharm",
    "pycharm.sh",
    "webstorm",
    "webstorm.sh",
    "phpstorm",
    "clion",
    "goland",
    "rider",
    "rustrover",
    "datagrip",
    "android-studio",
    "studio.sh",
    // Other editors
    "sublime_text",
    "subl",
    "atom",
    "gedit",
    "kate",
    "kwrite",
    "pluma",
    "xed",
    "mousepad",
    "geany",
    "neovim",
    "nvim",
    "vim",
    "gvim",
    "emacs",
    "emacs-gtk",
    "emacs-nox",
    // Specialized
    "zed",
    "helix",
    "lapce",
    "gnome-builder",
    "kdevelop",
    "qtcreator",
];

/// Creative applications
const CREATIVE_APPS: &[&str] = &[
    // Image editing
    "gimp",
    "gimp-2.10",
    "krita",
    "inkscape",
    "darktable",
    "rawtherapee",
    "digikam",
    "hugin",
    "photopea",
    // Video editing
    "kdenlive",
    "shotcut",
    "openshot",
    "pitivi",
    "olive-editor",
    "davinci-resolve",
    "resolve",
    "flowblade",
    "cinelerra",
    "natron",
    // Audio
    "audacity",
    "ardour",
    "lmms",
    "bitwig-studio",
    "reaper",
    "mixxx",
    "hydrogen",
    "musescore",
    "rosegarden",
    // 3D / CAD
    "blender",
    "freecad",
    "openscad",
    "fusion360",
    "cura",
    "prusa-slicer",
    "solvespace",
    // Design
    "figma-linux",
    "penpot",
];

/// Office and productivity applications
const PRODUCTIVITY_APPS: &[&str] = &[
    // LibreOffice
    "soffice.bin",
    "libreoffice",
    "lowriter",
    "localc",
    "loimpress",
    "lodraw",
    // Other office
    "onlyoffice",
    "wps",
    "calligra",
    // Note taking
    "obsidian",
    "logseq",
    "joplin",
    "notion",
    "notion-app",
    "standard-notes",
    "simplenote",
    "zettlr",
    "typora",
    "marktext",
    // PDF
    "evince",
    "okular",
    "zathura",
    "mupdf",
    "xreader",
    "qpdfview",
];

// ============================================================================
// LinuxModeDetector Implementation
// ============================================================================

/// Linux-specific game and focus mode detector
pub struct LinuxModeDetector {
    /// Set of known game processes (lowercase)
    game_processes: HashSet<String>,
    /// Set of known focus app processes (lowercase)
    focus_processes: HashSet<String>,
    /// Cache of running processes (updated on each check)
    process_cache: Vec<ProcessInfo>,
    /// Last cache update timestamp
    cache_timestamp: std::time::Instant,
    /// Cache validity duration
    cache_duration: std::time::Duration,
}

/// Information about a running process
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Process name (comm)
    pub name: String,
    /// Command line (for more detailed matching)
    pub cmdline: String,
    /// Parent process ID
    pub ppid: Option<u32>,
}

impl LinuxModeDetector {
    /// Create a new Linux mode detector
    pub fn new() -> Self {
        let mut game_processes = HashSet::new();
        let mut focus_processes = HashSet::new();

        // Populate game processes
        for list in [
            STEAM_PROCESSES,
            WINE_PROCESSES,
            NATIVE_GAMES,
            GAME_LAUNCHERS,
            EMULATORS,
        ] {
            for process in list {
                game_processes.insert(process.to_lowercase());
            }
        }

        // Populate focus processes
        for list in [
            VIDEO_CALL_APPS,
            BROWSERS,
            IDE_EDITORS,
            CREATIVE_APPS,
            PRODUCTIVITY_APPS,
        ] {
            for process in list {
                focus_processes.insert(process.to_lowercase());
            }
        }

        Self {
            game_processes,
            focus_processes,
            process_cache: Vec::new(),
            cache_timestamp: std::time::Instant::now(),
            cache_duration: std::time::Duration::from_secs(2),
        }
    }

    /// Refresh the process cache if needed
    fn refresh_cache(&mut self) {
        if self.cache_timestamp.elapsed() >= self.cache_duration {
            self.process_cache = self.enumerate_processes();
            self.cache_timestamp = std::time::Instant::now();
        }
    }

    /// Enumerate all running processes from /proc
    fn enumerate_processes(&self) -> Vec<ProcessInfo> {
        let mut processes = Vec::new();

        let proc_dir = match fs::read_dir("/proc") {
            Ok(dir) => dir,
            Err(_) => return processes,
        };

        for entry in proc_dir.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();

            // Only process numeric directory names (PIDs)
            if let Ok(pid) = name_str.parse::<u32>() {
                if let Some(info) = self.read_process_info(pid) {
                    processes.push(info);
                }
            }
        }

        processes
    }

    /// Read process information from /proc/[pid]
    fn read_process_info(&self, pid: u32) -> Option<ProcessInfo> {
        let proc_path = format!("/proc/{}", pid);

        // Read process name from /proc/[pid]/comm
        let comm_path = format!("{}/comm", proc_path);
        let name = fs::read_to_string(&comm_path)
            .ok()?
            .trim()
            .to_string();

        // Read command line from /proc/[pid]/cmdline
        let cmdline_path = format!("{}/cmdline", proc_path);
        let cmdline = fs::read_to_string(&cmdline_path)
            .unwrap_or_default()
            .replace('\0', " ")
            .trim()
            .to_string();

        // Read parent PID from /proc/[pid]/stat
        let stat_path = format!("{}/stat", proc_path);
        let ppid = fs::read_to_string(&stat_path)
            .ok()
            .and_then(|stat| {
                // Format: pid (comm) state ppid ...
                // Find the closing paren and parse the 4th field
                let after_comm = stat.rfind(')')? + 2;
                let fields: Vec<&str> = stat[after_comm..].split_whitespace().collect();
                fields.get(1)?.parse().ok()
            });

        Some(ProcessInfo {
            pid,
            name,
            cmdline,
            ppid,
        })
    }

    /// Check if a game is currently running
    /// Returns the name of the first detected game, if any
    pub fn is_game_running(&mut self) -> Option<String> {
        self.refresh_cache();

        for process in &self.process_cache {
            let name_lower = process.name.to_lowercase();

            // Direct match
            if self.game_processes.contains(&name_lower) {
                return Some(process.name.clone());
            }

            // Heuristic matching for games
            if self.looks_like_game(&name_lower, &process.cmdline.to_lowercase()) {
                return Some(process.name.clone());
            }
        }

        None
    }

    /// Check if a focus-worthy application is running
    /// Returns the name of the first detected focus app, if any
    pub fn is_focus_app_running(&mut self) -> Option<String> {
        self.refresh_cache();

        for process in &self.process_cache {
            let name_lower = process.name.to_lowercase();

            // Direct match
            if self.focus_processes.contains(&name_lower) {
                return Some(process.name.clone());
            }

            // Check cmdline for app indicators
            if self.looks_like_focus_app(&name_lower, &process.cmdline.to_lowercase()) {
                return Some(process.name.clone());
            }
        }

        None
    }

    /// Get the active window name using X11
    /// Falls back to None if X11 is not available
    pub fn get_active_window_name(&self) -> Option<String> {
        // Try using xdotool if available
        self.get_active_window_xdotool()
            .or_else(|| self.get_active_window_xprop())
    }

    /// Get active window name using xdotool
    fn get_active_window_xdotool(&self) -> Option<String> {
        use std::process::Command;

        let output = Command::new("xdotool")
            .args(["getactivewindow", "getwindowname"])
            .output()
            .ok()?;

        if output.status.success() {
            let name = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }

        None
    }

    /// Get active window name using xprop
    fn get_active_window_xprop(&self) -> Option<String> {
        use std::process::Command;

        // Get active window ID
        let root_output = Command::new("xprop")
            .args(["-root", "_NET_ACTIVE_WINDOW"])
            .output()
            .ok()?;

        if !root_output.status.success() {
            return None;
        }

        let root_str = String::from_utf8_lossy(&root_output.stdout);
        let window_id = root_str
            .split_whitespace()
            .last()?;

        // Get window name
        let name_output = Command::new("xprop")
            .args(["-id", window_id, "WM_NAME"])
            .output()
            .ok()?;

        if name_output.status.success() {
            let name_str = String::from_utf8_lossy(&name_output.stdout);
            // Parse: WM_NAME(STRING) = "Window Title"
            if let Some(start) = name_str.find('"') {
                if let Some(end) = name_str.rfind('"') {
                    if start < end {
                        return Some(name_str[start + 1..end].to_string());
                    }
                }
            }
        }

        None
    }

    /// Get all currently running games
    pub fn get_running_games(&mut self) -> Vec<String> {
        self.refresh_cache();
        let mut games = Vec::new();

        for process in &self.process_cache {
            let name_lower = process.name.to_lowercase();

            if self.game_processes.contains(&name_lower)
                || self.looks_like_game(&name_lower, &process.cmdline.to_lowercase())
            {
                // Avoid duplicates
                if !games.contains(&process.name) {
                    games.push(process.name.clone());
                }
            }
        }

        games
    }

    /// Get all currently running focus-worthy applications
    pub fn get_running_focus_apps(&mut self) -> Vec<String> {
        self.refresh_cache();
        let mut apps = Vec::new();

        for process in &self.process_cache {
            let name_lower = process.name.to_lowercase();

            if self.focus_processes.contains(&name_lower)
                || self.looks_like_focus_app(&name_lower, &process.cmdline.to_lowercase())
            {
                // Avoid duplicates
                if !apps.contains(&process.name) {
                    apps.push(process.name.clone());
                }
            }
        }

        apps
    }

    /// Heuristic to detect games not in the known list
    fn looks_like_game(&self, name: &str, cmdline: &str) -> bool {
        // Common game binary patterns
        let game_patterns = [
            "game",
            "unity",
            "unreal",
            "godot",
            "love", // LOVE2D
            "-vulkan",
            "-opengl",
            "-gl",
            "-sdl",
        ];

        for pattern in game_patterns {
            if name.contains(pattern) || cmdline.contains(pattern) {
                return true;
            }
        }

        // Check for Wine/Proton game executables
        if cmdline.contains(".exe") && (cmdline.contains("wine") || cmdline.contains("proton")) {
            return true;
        }

        // Check for Steam game paths
        if cmdline.contains("steamapps/common/") || cmdline.contains("compatdata/") {
            return true;
        }

        // Check for Lutris game paths
        if cmdline.contains("/games/") && cmdline.contains("lutris") {
            return true;
        }

        false
    }

    /// Heuristic to detect focus apps not in the known list
    fn looks_like_focus_app(&self, name: &str, cmdline: &str) -> bool {
        // Electron apps running video calls
        let electron_indicators = [
            "--type=renderer",
            "electron",
        ];

        let video_indicators = [
            "meet.google.com",
            "teams.microsoft.com",
            "zoom.us",
            "webex.com",
            "whereby.com",
            "jitsi",
            "discord.com/channels",
        ];

        // Check for browser with video call URL
        for browser in BROWSERS {
            if name.contains(browser) {
                for indicator in video_indicators {
                    if cmdline.contains(indicator) {
                        return true;
                    }
                }
            }
        }

        // Check for Electron-based communication apps
        for indicator in electron_indicators {
            if cmdline.contains(indicator) {
                for video in video_indicators {
                    if cmdline.contains(video) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Check if a specific process is running
    pub fn is_process_running(&mut self, process_name: &str) -> bool {
        self.refresh_cache();
        let name_lower = process_name.to_lowercase();

        self.process_cache
            .iter()
            .any(|p| p.name.to_lowercase() == name_lower)
    }

    /// Get process info by name
    pub fn get_process_by_name(&mut self, process_name: &str) -> Option<ProcessInfo> {
        self.refresh_cache();
        let name_lower = process_name.to_lowercase();

        self.process_cache
            .iter()
            .find(|p| p.name.to_lowercase() == name_lower)
            .cloned()
    }

    /// Add a custom game process to the detection list
    pub fn add_game_process(&mut self, name: &str) {
        self.game_processes.insert(name.to_lowercase());
    }

    /// Add a custom focus app process to the detection list
    pub fn add_focus_process(&mut self, name: &str) {
        self.focus_processes.insert(name.to_lowercase());
    }

    /// Remove a process from the game detection list
    pub fn remove_game_process(&mut self, name: &str) {
        self.game_processes.remove(&name.to_lowercase());
    }

    /// Remove a process from the focus app detection list
    pub fn remove_focus_process(&mut self, name: &str) {
        self.focus_processes.remove(&name.to_lowercase());
    }

    /// Get count of known game processes
    pub fn game_process_count(&self) -> usize {
        self.game_processes.len()
    }

    /// Get count of known focus app processes
    pub fn focus_process_count(&self) -> usize {
        self.focus_processes.len()
    }

    /// Set cache duration for process enumeration
    pub fn set_cache_duration(&mut self, duration: std::time::Duration) {
        self.cache_duration = duration;
    }

    /// Force refresh the process cache
    pub fn force_refresh(&mut self) {
        self.process_cache = self.enumerate_processes();
        self.cache_timestamp = std::time::Instant::now();
    }

    /// Get detailed game detection result with category
    pub fn detect_game_with_category(&mut self) -> Option<GameDetection> {
        self.refresh_cache();

        for process in &self.process_cache {
            let name_lower = process.name.to_lowercase();

            // Check each category
            for steam in STEAM_PROCESSES {
                if name_lower == steam.to_lowercase() {
                    return Some(GameDetection {
                        name: process.name.clone(),
                        category: GameCategory::Steam,
                        pid: process.pid,
                    });
                }
            }

            for wine in WINE_PROCESSES {
                if name_lower == wine.to_lowercase() {
                    return Some(GameDetection {
                        name: process.name.clone(),
                        category: GameCategory::WineProton,
                        pid: process.pid,
                    });
                }
            }

            for native in NATIVE_GAMES {
                if name_lower == native.to_lowercase() {
                    return Some(GameDetection {
                        name: process.name.clone(),
                        category: GameCategory::NativeGame,
                        pid: process.pid,
                    });
                }
            }

            for launcher in GAME_LAUNCHERS {
                if name_lower == launcher.to_lowercase() {
                    return Some(GameDetection {
                        name: process.name.clone(),
                        category: GameCategory::Launcher,
                        pid: process.pid,
                    });
                }
            }

            for emu in EMULATORS {
                if name_lower == emu.to_lowercase() {
                    return Some(GameDetection {
                        name: process.name.clone(),
                        category: GameCategory::Emulator,
                        pid: process.pid,
                    });
                }
            }
        }

        None
    }

    /// Get detailed focus app detection result with category
    pub fn detect_focus_app_with_category(&mut self) -> Option<FocusAppDetection> {
        self.refresh_cache();

        for process in &self.process_cache {
            let name_lower = process.name.to_lowercase();

            for video in VIDEO_CALL_APPS {
                if name_lower == video.to_lowercase() {
                    return Some(FocusAppDetection {
                        name: process.name.clone(),
                        category: FocusCategory::VideoCall,
                        pid: process.pid,
                    });
                }
            }

            for browser in BROWSERS {
                if name_lower == browser.to_lowercase() {
                    return Some(FocusAppDetection {
                        name: process.name.clone(),
                        category: FocusCategory::Browser,
                        pid: process.pid,
                    });
                }
            }

            for ide in IDE_EDITORS {
                if name_lower == ide.to_lowercase() {
                    return Some(FocusAppDetection {
                        name: process.name.clone(),
                        category: FocusCategory::IdeEditor,
                        pid: process.pid,
                    });
                }
            }

            for creative in CREATIVE_APPS {
                if name_lower == creative.to_lowercase() {
                    return Some(FocusAppDetection {
                        name: process.name.clone(),
                        category: FocusCategory::Creative,
                        pid: process.pid,
                    });
                }
            }

            for prod in PRODUCTIVITY_APPS {
                if name_lower == prod.to_lowercase() {
                    return Some(FocusAppDetection {
                        name: process.name.clone(),
                        category: FocusCategory::Productivity,
                        pid: process.pid,
                    });
                }
            }
        }

        None
    }
}

impl Default for LinuxModeDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Detection Result Types
// ============================================================================

/// Category of detected game
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameCategory {
    /// Steam/Valve processes
    Steam,
    /// Wine/Proton compatibility layer
    WineProton,
    /// Native Linux game
    NativeGame,
    /// Game launcher (Lutris, Heroic, etc.)
    Launcher,
    /// Emulator (RetroArch, Dolphin, etc.)
    Emulator,
    /// Unknown/heuristic match
    Unknown,
}

impl std::fmt::Display for GameCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameCategory::Steam => write!(f, "Steam"),
            GameCategory::WineProton => write!(f, "Wine/Proton"),
            GameCategory::NativeGame => write!(f, "Native Game"),
            GameCategory::Launcher => write!(f, "Game Launcher"),
            GameCategory::Emulator => write!(f, "Emulator"),
            GameCategory::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Category of detected focus application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusCategory {
    /// Video call / conferencing
    VideoCall,
    /// Web browser
    Browser,
    /// IDE or code editor
    IdeEditor,
    /// Creative application
    Creative,
    /// Productivity / office
    Productivity,
    /// Unknown/heuristic match
    Unknown,
}

impl std::fmt::Display for FocusCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FocusCategory::VideoCall => write!(f, "Video Call"),
            FocusCategory::Browser => write!(f, "Browser"),
            FocusCategory::IdeEditor => write!(f, "IDE/Editor"),
            FocusCategory::Creative => write!(f, "Creative"),
            FocusCategory::Productivity => write!(f, "Productivity"),
            FocusCategory::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Detailed game detection result
#[derive(Debug, Clone)]
pub struct GameDetection {
    /// Process name
    pub name: String,
    /// Category of game
    pub category: GameCategory,
    /// Process ID
    pub pid: u32,
}

/// Detailed focus app detection result
#[derive(Debug, Clone)]
pub struct FocusAppDetection {
    /// Process name
    pub name: String,
    /// Category of focus app
    pub category: FocusCategory,
    /// Process ID
    pub pid: u32,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_creation() {
        let detector = LinuxModeDetector::new();
        assert!(detector.game_process_count() > 50);
        assert!(detector.focus_process_count() > 50);
    }

    #[test]
    fn test_game_processes_populated() {
        let detector = LinuxModeDetector::new();
        assert!(detector.game_processes.contains("steam"));
        assert!(detector.game_processes.contains("wine"));
        assert!(detector.game_processes.contains("retroarch"));
        assert!(detector.game_processes.contains("lutris"));
        assert!(detector.game_processes.contains("factorio"));
    }

    #[test]
    fn test_focus_processes_populated() {
        let detector = LinuxModeDetector::new();
        assert!(detector.focus_processes.contains("zoom"));
        assert!(detector.focus_processes.contains("discord"));
        assert!(detector.focus_processes.contains("code"));
        assert!(detector.focus_processes.contains("gimp"));
        assert!(detector.focus_processes.contains("blender"));
    }

    #[test]
    fn test_add_custom_process() {
        let mut detector = LinuxModeDetector::new();
        let initial_count = detector.game_process_count();

        detector.add_game_process("my-custom-game");
        assert_eq!(detector.game_process_count(), initial_count + 1);
        assert!(detector.game_processes.contains("my-custom-game"));
    }

    #[test]
    fn test_remove_process() {
        let mut detector = LinuxModeDetector::new();
        detector.add_game_process("test-game");
        assert!(detector.game_processes.contains("test-game"));

        detector.remove_game_process("test-game");
        assert!(!detector.game_processes.contains("test-game"));
    }

    #[test]
    fn test_looks_like_game() {
        let detector = LinuxModeDetector::new();

        // Should detect Unity games
        assert!(detector.looks_like_game("unity-player", ""));

        // Should detect games run through Wine
        assert!(detector.looks_like_game("some-game", "wine /path/to/game.exe"));

        // Should detect Steam games by path
        assert!(detector.looks_like_game("myapp", "/home/user/.steam/steamapps/common/MyGame/game"));

        // Should not match random processes
        assert!(!detector.looks_like_game("bash", "/bin/bash"));
    }

    #[test]
    fn test_looks_like_focus_app() {
        let detector = LinuxModeDetector::new();

        // Should detect browser with video call URL
        assert!(detector.looks_like_focus_app("firefox", "firefox https://meet.google.com/abc-xyz"));

        // Should not match browser with regular URL
        assert!(!detector.looks_like_focus_app("firefox", "firefox https://example.com"));
    }

    #[test]
    fn test_game_category_display() {
        assert_eq!(format!("{}", GameCategory::Steam), "Steam");
        assert_eq!(format!("{}", GameCategory::WineProton), "Wine/Proton");
        assert_eq!(format!("{}", GameCategory::Emulator), "Emulator");
    }

    #[test]
    fn test_focus_category_display() {
        assert_eq!(format!("{}", FocusCategory::VideoCall), "Video Call");
        assert_eq!(format!("{}", FocusCategory::IdeEditor), "IDE/Editor");
        assert_eq!(format!("{}", FocusCategory::Creative), "Creative");
    }

    #[test]
    fn test_cache_duration() {
        let mut detector = LinuxModeDetector::new();
        detector.set_cache_duration(std::time::Duration::from_secs(5));
        assert_eq!(detector.cache_duration, std::time::Duration::from_secs(5));
    }
}
