//! AI Workload Detector
//!
//! Automatically detects running AI inference engines and workloads.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Known AI runtime types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AIRuntime {
    /// Ollama - REST API on :11434
    Ollama,
    /// llama.cpp - Direct process
    LlamaCpp,
    /// vLLM - OpenAI-compatible API
    VLLM,
    /// PyTorch - torch.exe / python with torch
    PyTorch,
    /// ONNX Runtime - onnxruntime.dll loaded
    ONNXRuntime,
    /// DirectML - Windows ML
    DirectML,
    /// Whisper - Audio transcription
    Whisper,
    /// Stable Diffusion - Image generation
    StableDiffusion,
    /// ComfyUI - SD workflow
    ComfyUI,
    /// RuVLLM - Rust LLM runtime
    RuVLLM,
    /// LM Studio
    LMStudio,
    /// Text Generation Inference
    TGI,
    /// Unknown AI workload
    Unknown,
}

impl std::fmt::Display for AIRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AIRuntime::Ollama => write!(f, "Ollama"),
            AIRuntime::LlamaCpp => write!(f, "llama.cpp"),
            AIRuntime::VLLM => write!(f, "vLLM"),
            AIRuntime::PyTorch => write!(f, "PyTorch"),
            AIRuntime::ONNXRuntime => write!(f, "ONNX Runtime"),
            AIRuntime::DirectML => write!(f, "DirectML"),
            AIRuntime::Whisper => write!(f, "Whisper"),
            AIRuntime::StableDiffusion => write!(f, "Stable Diffusion"),
            AIRuntime::ComfyUI => write!(f, "ComfyUI"),
            AIRuntime::RuVLLM => write!(f, "RuVLLM"),
            AIRuntime::LMStudio => write!(f, "LM Studio"),
            AIRuntime::TGI => write!(f, "TGI"),
            AIRuntime::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Information about a detected AI workload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveWorkload {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
    /// Detected runtime type
    pub runtime: AIRuntime,
    /// Estimated VRAM usage in MB
    pub vram_mb: Option<u64>,
    /// Estimated RAM usage in MB
    pub ram_mb: u64,
    /// Model name if detectable
    pub model: Option<String>,
    /// Is actively running inference
    pub is_active: bool,
}

/// Process signatures for AI runtime detection
struct ProcessSignature {
    name_patterns: Vec<&'static str>,
    dll_patterns: Vec<&'static str>,
    port: Option<u16>,
}

/// AI workload detector
pub struct AIWorkloadDetector {
    signatures: HashMap<AIRuntime, ProcessSignature>,
}

impl AIWorkloadDetector {
    pub fn new() -> Self {
        let mut signatures = HashMap::new();

        signatures.insert(AIRuntime::Ollama, ProcessSignature {
            name_patterns: vec!["ollama", "ollama_llama_server"],
            dll_patterns: vec![],
            port: Some(11434),
        });

        signatures.insert(AIRuntime::LlamaCpp, ProcessSignature {
            name_patterns: vec!["llama", "main", "server", "llama-server", "llama-cli"],
            dll_patterns: vec!["ggml"],
            port: Some(8080),
        });

        signatures.insert(AIRuntime::VLLM, ProcessSignature {
            name_patterns: vec!["vllm", "ray"],
            dll_patterns: vec![],
            port: Some(8000),
        });

        signatures.insert(AIRuntime::PyTorch, ProcessSignature {
            name_patterns: vec!["python", "python3", "pythonw"],
            dll_patterns: vec!["torch", "cuda", "cudnn"],
            port: None,
        });

        signatures.insert(AIRuntime::ONNXRuntime, ProcessSignature {
            name_patterns: vec![],
            dll_patterns: vec!["onnxruntime", "onnxruntime_providers_cuda"],
            port: None,
        });

        signatures.insert(AIRuntime::Whisper, ProcessSignature {
            name_patterns: vec!["whisper", "whisper.cpp"],
            dll_patterns: vec![],
            port: None,
        });

        signatures.insert(AIRuntime::StableDiffusion, ProcessSignature {
            name_patterns: vec!["python"],
            dll_patterns: vec!["diffusers", "stable_diffusion"],
            port: Some(7860), // Gradio default
        });

        signatures.insert(AIRuntime::ComfyUI, ProcessSignature {
            name_patterns: vec!["python"],
            dll_patterns: vec!["comfy"],
            port: Some(8188),
        });

        signatures.insert(AIRuntime::RuVLLM, ProcessSignature {
            name_patterns: vec!["ruvllm", "ruvector-llm"],
            dll_patterns: vec![],
            port: Some(8080),
        });

        signatures.insert(AIRuntime::LMStudio, ProcessSignature {
            name_patterns: vec!["lm studio", "lmstudio"],
            dll_patterns: vec![],
            port: Some(1234),
        });

        signatures.insert(AIRuntime::TGI, ProcessSignature {
            name_patterns: vec!["text-generation"],
            dll_patterns: vec![],
            port: Some(3000),
        });

        Self { signatures }
    }

    /// Detect all running AI workloads
    pub fn detect(&self) -> Vec<ActiveWorkload> {
        let mut workloads = Vec::new();

        // Use Windows API to enumerate processes
        #[cfg(windows)]
        {
            use windows::Win32::System::ProcessStatus::{
                EnumProcesses, GetModuleBaseNameW,
            };
            use windows::Win32::System::Threading::{
                OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
            };
            use windows::Win32::Foundation::CloseHandle;

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
                                let name = String::from_utf16_lossy(&name_buf[..len as usize])
                                    .to_lowercase();

                                // Check against signatures
                                if let Some(runtime) = self.match_process(&name) {
                                    workloads.push(ActiveWorkload {
                                        pid,
                                        name: name.clone(),
                                        runtime,
                                        vram_mb: None, // Will be filled by GPU monitor
                                        ram_mb: self.get_process_memory(pid),
                                        model: None,
                                        is_active: true,
                                    });
                                }
                            }

                            let _ = CloseHandle(handle);
                        }
                    }
                }
            }
        }

        workloads
    }

    /// Match process name to AI runtime
    fn match_process(&self, name: &str) -> Option<AIRuntime> {
        let name_lower = name.to_lowercase();

        for (runtime, sig) in &self.signatures {
            for pattern in &sig.name_patterns {
                if name_lower.contains(pattern) {
                    return Some(*runtime);
                }
            }
        }

        None
    }

    /// Get process memory usage
    #[cfg(windows)]
    fn get_process_memory(&self, pid: u32) -> u64 {
        use windows::Win32::System::ProcessStatus::GetProcessMemoryInfo;
        use windows::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS;
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
        use windows::Win32::Foundation::CloseHandle;

        unsafe {
            if let Ok(handle) = OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                false,
                pid,
            ) {
                let mut pmc = PROCESS_MEMORY_COUNTERS::default();
                pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

                if GetProcessMemoryInfo(
                    handle,
                    &mut pmc,
                    std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                ).is_ok() {
                    let _ = CloseHandle(handle);
                    return pmc.WorkingSetSize as u64 / (1024 * 1024); // Convert to MB
                }

                let _ = CloseHandle(handle);
            }
        }
        0
    }

    #[cfg(not(windows))]
    fn get_process_memory(&self, _pid: u32) -> u64 {
        0
    }

    /// Check if a specific port is in use (indicating a service)
    pub fn check_port(&self, port: u16) -> bool {
        use std::net::TcpStream;
        TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok()
    }

    /// Detect runtime by checking known ports
    pub fn detect_by_port(&self) -> Vec<AIRuntime> {
        let mut found = Vec::new();

        for (runtime, sig) in &self.signatures {
            if let Some(port) = sig.port {
                if self.check_port(port) {
                    found.push(*runtime);
                }
            }
        }

        found
    }
}

impl Default for AIWorkloadDetector {
    fn default() -> Self {
        Self::new()
    }
}
