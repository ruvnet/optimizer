//! Workload Classification
//!
//! ML-based workload detection and classification for intelligent optimization.
//! Detects gaming, coding, video editing, browsing, AI inference, and more.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Workload type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkloadType {
    /// Gaming - prioritize GPU, low latency
    Gaming,
    /// Software development - IDE, compilers, VMs
    Coding,
    /// Video editing - high memory, GPU compute
    VideoEditing,
    /// Web browsing - moderate, bursty
    Browsing,
    /// AI/ML inference - VRAM, compute intensive
    AIInference,
    /// AI/ML training - sustained GPU, high memory
    AITraining,
    /// Office work - low resource
    Office,
    /// Media playback - steady, low CPU
    MediaPlayback,
    /// Video call - webcam, audio, moderate
    VideoCall,
    /// Idle - minimal activity
    Idle,
    /// Mixed workload
    Mixed,
    /// Unknown
    Unknown,
}

impl std::fmt::Display for WorkloadType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkloadType::Gaming => write!(f, "Gaming"),
            WorkloadType::Coding => write!(f, "Coding"),
            WorkloadType::VideoEditing => write!(f, "Video Editing"),
            WorkloadType::Browsing => write!(f, "Browsing"),
            WorkloadType::AIInference => write!(f, "AI Inference"),
            WorkloadType::AITraining => write!(f, "AI Training"),
            WorkloadType::Office => write!(f, "Office"),
            WorkloadType::MediaPlayback => write!(f, "Media Playback"),
            WorkloadType::VideoCall => write!(f, "Video Call"),
            WorkloadType::Idle => write!(f, "Idle"),
            WorkloadType::Mixed => write!(f, "Mixed"),
            WorkloadType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Workload profile with optimization hints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadProfile {
    /// Workload type
    pub workload_type: WorkloadType,
    /// CPU priority hint (0-100)
    pub cpu_priority: u32,
    /// GPU priority hint (0-100)
    pub gpu_priority: u32,
    /// Memory priority hint (0-100)
    pub memory_priority: u32,
    /// Latency sensitive
    pub latency_sensitive: bool,
    /// Throughput focused
    pub throughput_focused: bool,
    /// Background tasks allowed
    pub background_allowed: bool,
    /// Optimization suggestions
    pub suggestions: Vec<String>,
}

impl WorkloadProfile {
    pub fn for_type(workload_type: WorkloadType) -> Self {
        match workload_type {
            WorkloadType::Gaming => Self {
                workload_type,
                cpu_priority: 80,
                gpu_priority: 100,
                memory_priority: 70,
                latency_sensitive: true,
                throughput_focused: false,
                background_allowed: false,
                suggestions: vec![
                    "Disable background apps".into(),
                    "Set GPU to performance mode".into(),
                    "Increase process priority".into(),
                ],
            },
            WorkloadType::Coding => Self {
                workload_type,
                cpu_priority: 70,
                gpu_priority: 30,
                memory_priority: 80,
                latency_sensitive: false,
                throughput_focused: true,
                background_allowed: true,
                suggestions: vec![
                    "Keep IDE in high priority".into(),
                    "Reserve RAM for compilers".into(),
                ],
            },
            WorkloadType::VideoEditing => Self {
                workload_type,
                cpu_priority: 90,
                gpu_priority: 90,
                memory_priority: 95,
                latency_sensitive: false,
                throughput_focused: true,
                background_allowed: false,
                suggestions: vec![
                    "Maximize RAM availability".into(),
                    "Enable GPU acceleration".into(),
                    "Close unnecessary apps".into(),
                ],
            },
            WorkloadType::AIInference => Self {
                workload_type,
                cpu_priority: 60,
                gpu_priority: 95,
                memory_priority: 90,
                latency_sensitive: true,
                throughput_focused: false,
                background_allowed: true,
                suggestions: vec![
                    "Reserve VRAM for models".into(),
                    "Enable KV cache optimization".into(),
                ],
            },
            WorkloadType::AITraining => Self {
                workload_type,
                cpu_priority: 70,
                gpu_priority: 100,
                memory_priority: 95,
                latency_sensitive: false,
                throughput_focused: true,
                background_allowed: false,
                suggestions: vec![
                    "Maximize VRAM allocation".into(),
                    "Enable mixed precision".into(),
                    "Close all non-essential apps".into(),
                ],
            },
            WorkloadType::VideoCall => Self {
                workload_type,
                cpu_priority: 60,
                gpu_priority: 40,
                memory_priority: 50,
                latency_sensitive: true,
                throughput_focused: false,
                background_allowed: true,
                suggestions: vec![
                    "Prioritize network stability".into(),
                    "Reduce background CPU usage".into(),
                ],
            },
            _ => Self {
                workload_type,
                cpu_priority: 50,
                gpu_priority: 50,
                memory_priority: 50,
                latency_sensitive: false,
                throughput_focused: false,
                background_allowed: true,
                suggestions: vec![],
            },
        }
    }
}

/// Process signatures for workload classification
struct ProcessSignature {
    patterns: Vec<&'static str>,
    workload_type: WorkloadType,
    weight: f32,
}

/// Workload classifier using process analysis
pub struct WorkloadClassifier {
    signatures: Vec<ProcessSignature>,
    history: Vec<(std::time::Instant, WorkloadType)>,
}

impl WorkloadClassifier {
    pub fn new() -> Self {
        let signatures = vec![
            // Gaming
            ProcessSignature {
                patterns: vec![
                    "steam", "epicgameslauncher", "gog", "origin", "uplay",
                    "battlenet", "valorant", "fortnite", "minecraft",
                    "rocketleague", "csgo", "dota2", "leagueoflegends",
                ],
                workload_type: WorkloadType::Gaming,
                weight: 1.0,
            },
            // Coding
            ProcessSignature {
                patterns: vec![
                    "code", "vscode", "idea", "pycharm", "webstorm", "rider",
                    "visual studio", "devenv", "vim", "nvim", "emacs",
                    "sublime_text", "atom", "notepad++", "cargo", "rustc",
                    "node", "python", "java", "go", "dotnet",
                ],
                workload_type: WorkloadType::Coding,
                weight: 0.8,
            },
            // Video Editing
            ProcessSignature {
                patterns: vec![
                    "premiere", "aftereffects", "resolve", "davinci",
                    "finalcut", "vegas", "kdenlive", "shotcut", "blender",
                    "handbrake", "ffmpeg",
                ],
                workload_type: WorkloadType::VideoEditing,
                weight: 1.0,
            },
            // AI Inference
            ProcessSignature {
                patterns: vec![
                    "ollama", "llama", "vllm", "tgi", "lmstudio",
                    "oobabooga", "koboldcpp", "exllama",
                ],
                workload_type: WorkloadType::AIInference,
                weight: 1.0,
            },
            // AI Training
            ProcessSignature {
                patterns: vec![
                    "pytorch", "tensorflow", "train", "accelerate",
                ],
                workload_type: WorkloadType::AITraining,
                weight: 0.7,
            },
            // Video Call
            ProcessSignature {
                patterns: vec![
                    "zoom", "teams", "slack", "discord", "skype",
                    "webex", "googlemeet", "facetime",
                ],
                workload_type: WorkloadType::VideoCall,
                weight: 0.9,
            },
            // Browsing
            ProcessSignature {
                patterns: vec![
                    "chrome", "firefox", "edge", "safari", "opera", "brave",
                ],
                workload_type: WorkloadType::Browsing,
                weight: 0.5,
            },
            // Office
            ProcessSignature {
                patterns: vec![
                    "word", "excel", "powerpoint", "outlook", "onenote",
                    "libreoffice", "sheets", "docs",
                ],
                workload_type: WorkloadType::Office,
                weight: 0.6,
            },
            // Media Playback
            ProcessSignature {
                patterns: vec![
                    "vlc", "mpv", "mpc-hc", "spotify", "musicbee",
                    "foobar2000", "netflix", "plex",
                ],
                workload_type: WorkloadType::MediaPlayback,
                weight: 0.5,
            },
        ];

        Self {
            signatures,
            history: Vec::new(),
        }
    }

    /// Classify current workload based on running processes
    pub fn classify_current(&self) -> WorkloadType {
        let processes = self.get_running_processes();
        self.classify_from_processes(&processes)
    }

    /// Classify workload from process list
    pub fn classify_from_processes(&self, processes: &[String]) -> WorkloadType {
        let mut scores: HashMap<WorkloadType, f32> = HashMap::new();

        for process in processes {
            let process_lower = process.to_lowercase();

            for sig in &self.signatures {
                for pattern in &sig.patterns {
                    if process_lower.contains(pattern) {
                        *scores.entry(sig.workload_type).or_insert(0.0) += sig.weight;
                    }
                }
            }
        }

        // Find highest scoring workload
        scores
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(wt, _)| wt)
            .unwrap_or(WorkloadType::Unknown)
    }

    /// Get optimization profile for current workload
    pub fn get_profile(&self) -> WorkloadProfile {
        let workload = self.classify_current();
        WorkloadProfile::for_type(workload)
    }

    /// Get running processes
    #[cfg(windows)]
    fn get_running_processes(&self) -> Vec<String> {
        use windows::Win32::System::ProcessStatus::{EnumProcesses, GetModuleBaseNameW};
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
        };
        use windows::Win32::Foundation::CloseHandle;

        let mut processes = Vec::new();

        unsafe {
            let mut pids = [0u32; 2048];
            let mut bytes_returned = 0u32;

            if EnumProcesses(
                pids.as_mut_ptr(),
                (pids.len() * std::mem::size_of::<u32>()) as u32,
                &mut bytes_returned,
            ).is_ok() {
                let num_processes = bytes_returned as usize / std::mem::size_of::<u32>();

                for &pid in &pids[..num_processes] {
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
    fn get_running_processes(&self) -> Vec<String> {
        Vec::new()
    }

    /// Record workload for history/learning
    pub fn record_workload(&mut self, workload: WorkloadType) {
        self.history.push((std::time::Instant::now(), workload));

        // Keep only last 1000 entries
        if self.history.len() > 1000 {
            self.history.remove(0);
        }
    }

    /// Get workload trend over time
    pub fn get_trend(&self, duration: std::time::Duration) -> HashMap<WorkloadType, usize> {
        let cutoff = std::time::Instant::now() - duration;
        let mut counts: HashMap<WorkloadType, usize> = HashMap::new();

        for (time, workload) in &self.history {
            if *time > cutoff {
                *counts.entry(*workload).or_insert(0) += 1;
            }
        }

        counts
    }
}

impl Default for WorkloadClassifier {
    fn default() -> Self {
        Self::new()
    }
}
