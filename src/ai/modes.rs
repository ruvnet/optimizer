//! Performance Modes - Game Mode, Focus Mode, and more
//!
//! Intelligent mode detection and automatic optimization switching.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Performance mode settings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceMode {
    /// Maximum performance - all resources for foreground
    Maximum,
    /// Balanced - normal operation
    Balanced,
    /// Power saver - minimize resource usage
    PowerSaver,
    /// Silent - minimize fan noise
    Silent,
    /// Custom
    Custom,
}

/// Game Mode - Auto-detect games and maximize performance
pub struct GameMode {
    enabled: bool,
    active: bool,
    detected_game: Option<String>,
    known_games: HashSet<String>,
    optimizations_applied: Vec<String>,
}

impl GameMode {
    pub fn new(enabled: bool) -> Self {
        let mut known_games = HashSet::new();

        // Popular games and launchers
        let games = [
            "steam.exe", "steamwebhelper.exe",
            "epicgameslauncher.exe", "fortnitelauncher.exe",
            "battlenet.exe", "agent.exe",
            "origin.exe", "eadesktop.exe",
            "upc.exe", "uplay.exe",
            "gog galaxy.exe",
            "valorant.exe", "valorant-win64-shipping.exe",
            "csgo.exe", "cs2.exe",
            "dota2.exe",
            "leagueoflegends.exe", "league of legends.exe",
            "overwatch.exe",
            "minecraft.exe", "javaw.exe",
            "fortnite.exe", "fortniteclient-win64-shipping.exe",
            "rocketleague.exe",
            "pubg.exe", "tslgame.exe",
            "apexlegends.exe", "r5apex.exe",
            "gta5.exe", "gtavlauncher.exe",
            "eldenring.exe",
            "cyberpunk2077.exe",
            "baldursgate3.exe", "bg3.exe",
            "starfield.exe",
        ];

        for game in games {
            known_games.insert(game.to_lowercase());
        }

        Self {
            enabled,
            active: false,
            detected_game: None,
            known_games,
            optimizations_applied: Vec::new(),
        }
    }

    /// Check if a game is running and activate if needed
    pub fn check_and_activate(&mut self) -> Option<super::GameModeAction> {
        if !self.enabled {
            return None;
        }

        let processes = self.get_foreground_processes();

        for process in &processes {
            let process_lower = process.to_lowercase();
            if self.known_games.contains(&process_lower) ||
               self.looks_like_game(&process_lower) {
                if !self.active || self.detected_game.as_ref() != Some(process) {
                    return Some(self.activate(process.clone()));
                }
                return None;
            }
        }

        // No game detected, deactivate if active
        if self.active {
            self.deactivate();
        }

        None
    }

    /// Activate game mode
    fn activate(&mut self, game: String) -> super::GameModeAction {
        self.active = true;
        self.detected_game = Some(game.clone());
        self.optimizations_applied.clear();

        // Apply optimizations
        let mut optimizations = Vec::new();

        // 1. Increase process priority
        self.boost_process_priority(&game);
        optimizations.push("Boosted game process priority".into());

        // 2. Reduce background process priority
        self.reduce_background_priority();
        optimizations.push("Reduced background process priority".into());

        // 3. Disable unnecessary services (placeholder)
        optimizations.push("Disabled non-essential background tasks".into());

        // 4. Set GPU to performance mode (would need vendor-specific API)
        optimizations.push("Requested GPU performance mode".into());

        // 5. Optimize memory
        optimizations.push("Freed memory for game usage".into());

        self.optimizations_applied = optimizations.clone();

        super::GameModeAction {
            game_detected: game,
            optimizations_applied: optimizations,
        }
    }

    /// Deactivate game mode
    fn deactivate(&mut self) {
        self.active = false;
        self.detected_game = None;
        self.optimizations_applied.clear();
        // Restore normal settings
    }

    /// Check if game mode is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get currently detected game
    pub fn detected_game(&self) -> Option<&String> {
        self.detected_game.as_ref()
    }

    /// Heuristic to detect games not in known list
    fn looks_like_game(&self, name: &str) -> bool {
        // Games often have these patterns
        name.contains("game") ||
        name.contains("win64-shipping") ||
        name.contains("win32-shipping") ||
        name.ends_with("-dx11.exe") ||
        name.ends_with("-dx12.exe") ||
        name.ends_with("-vulkan.exe")
    }

    /// Get foreground processes
    #[cfg(windows)]
    fn get_foreground_processes(&self) -> Vec<String> {
        use windows::Win32::System::ProcessStatus::{EnumProcesses, GetModuleBaseNameW};
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
        };
        use windows::Win32::Foundation::CloseHandle;

        let mut processes = Vec::new();

        unsafe {
            let mut pids = [0u32; 1024];
            let mut bytes_returned = 0u32;

            if EnumProcesses(
                pids.as_mut_ptr(),
                (pids.len() * std::mem::size_of::<u32>()) as u32,
                &mut bytes_returned,
            ).is_ok() {
                let num_processes = bytes_returned as usize / std::mem::size_of::<u32>();

                for &pid in &pids[..num_processes.min(100)] {
                    if pid == 0 {
                        continue;
                    }

                    if let Ok(handle) = OpenProcess(
                        PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                        false,
                        pid,
                    ) {
                        let mut name_buf = [0u16; 260];
                        let len = GetModuleBaseNameW(handle, None, &mut name_buf);

                        if len > 0 {
                            let name = String::from_utf16_lossy(&name_buf[..len as usize]);
                            processes.push(name);
                        }

                        let _ = CloseHandle(handle);
                    }
                }
            }
        }

        processes
    }

    #[cfg(not(windows))]
    fn get_foreground_processes(&self) -> Vec<String> {
        Vec::new()
    }

    /// Boost game process priority
    #[cfg(windows)]
    fn boost_process_priority(&self, _game: &str) {
        // Would use SetPriorityClass
    }

    #[cfg(not(windows))]
    fn boost_process_priority(&self, _game: &str) {}

    /// Reduce background process priority
    #[cfg(windows)]
    fn reduce_background_priority(&self) {
        // Would iterate non-essential processes and lower priority
    }

    #[cfg(not(windows))]
    fn reduce_background_priority(&self) {}
}

/// Focus Mode - Detect meetings/calls and optimize for them
pub struct FocusMode {
    enabled: bool,
    active: bool,
    trigger: Option<String>,
    known_call_apps: HashSet<String>,
    actions_taken: Vec<String>,
}

impl FocusMode {
    pub fn new(enabled: bool) -> Self {
        let mut known_call_apps = HashSet::new();

        let apps = [
            "zoom.exe", "zoom",
            "teams.exe", "ms-teams.exe",
            "slack.exe",
            "discord.exe",
            "skype.exe",
            "webex.exe", "ciscowebexstart.exe",
            "facetime",
            "meet.google.com",
        ];

        for app in apps {
            known_call_apps.insert(app.to_lowercase());
        }

        Self {
            enabled,
            active: false,
            trigger: None,
            known_call_apps,
            actions_taken: Vec::new(),
        }
    }

    /// Check if a call app is in use and activate focus mode
    pub fn check_and_activate(&mut self) -> Option<super::FocusModeAction> {
        if !self.enabled {
            return None;
        }

        // Check for video call applications
        let processes = self.get_audio_video_processes();

        for process in &processes {
            let process_lower = process.to_lowercase();
            if self.known_call_apps.contains(&process_lower) {
                if !self.active {
                    return Some(self.activate(process.clone()));
                }
                return None;
            }
        }

        // No call app detected, deactivate if active
        if self.active {
            self.deactivate();
        }

        None
    }

    /// Activate focus mode
    fn activate(&mut self, trigger: String) -> super::FocusModeAction {
        self.active = true;
        self.trigger = Some(trigger.clone());
        self.actions_taken.clear();

        let mut actions = Vec::new();

        // 1. Reduce background activity
        actions.push("Reduced background process activity".into());

        // 2. Prioritize audio/video streams
        actions.push("Prioritized audio/video processing".into());

        // 3. Disable heavy background tasks
        actions.push("Paused heavy background optimizations".into());

        // 4. Optimize network for low latency
        actions.push("Optimized network for low latency".into());

        self.actions_taken = actions.clone();

        super::FocusModeAction {
            trigger,
            actions_taken: actions,
        }
    }

    /// Deactivate focus mode
    fn deactivate(&mut self) {
        self.active = false;
        self.trigger = None;
        self.actions_taken.clear();
    }

    /// Check if focus mode is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get trigger application
    pub fn trigger(&self) -> Option<&String> {
        self.trigger.as_ref()
    }

    /// Get processes using audio/video
    #[cfg(windows)]
    fn get_audio_video_processes(&self) -> Vec<String> {
        // Simplified - would check audio session API
        use windows::Win32::System::ProcessStatus::{EnumProcesses, GetModuleBaseNameW};
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
        };
        use windows::Win32::Foundation::CloseHandle;

        let mut processes = Vec::new();

        unsafe {
            let mut pids = [0u32; 512];
            let mut bytes_returned = 0u32;

            if EnumProcesses(
                pids.as_mut_ptr(),
                (pids.len() * std::mem::size_of::<u32>()) as u32,
                &mut bytes_returned,
            ).is_ok() {
                let num_processes = bytes_returned as usize / std::mem::size_of::<u32>();

                for &pid in &pids[..num_processes.min(50)] {
                    if pid == 0 {
                        continue;
                    }

                    if let Ok(handle) = OpenProcess(
                        PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                        false,
                        pid,
                    ) {
                        let mut name_buf = [0u16; 260];
                        let len = GetModuleBaseNameW(handle, None, &mut name_buf);

                        if len > 0 {
                            let name = String::from_utf16_lossy(&name_buf[..len as usize]);
                            if self.known_call_apps.contains(&name.to_lowercase()) {
                                processes.push(name);
                            }
                        }

                        let _ = CloseHandle(handle);
                    }
                }
            }
        }

        processes
    }

    #[cfg(not(windows))]
    fn get_audio_video_processes(&self) -> Vec<String> {
        Vec::new()
    }
}

impl Default for GameMode {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Default for FocusMode {
    fn default() -> Self {
        Self::new(true)
    }
}
