//! JNI Bridge for Android Integration
//!
//! Provides JNI entry points for Kotlin/Java to call Rust code.
//! All memory operations on Android are read-only for safety.

#![cfg(target_os = "android")]

use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use serde::Serialize;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::core::config::OptimizerConfig;
use crate::core::patterns::{LabeledPattern, MemoryPattern};
use crate::neural::hnsw_patterns::PatternIndex;

/// Android-specific neural engine wrapper
/// Uses a simpler pattern index for mobile devices
pub struct AndroidNeuralEngine {
    pattern_index: PatternIndex,
    history: Vec<LabeledPattern>,
    model_path: PathBuf,
}

impl AndroidNeuralEngine {
    pub fn new(model_path: PathBuf) -> Result<Self, String> {
        let pattern_index = PatternIndex::new(MemoryPattern::DIM)
            .map_err(|e| format!("Failed to create pattern index: {}", e))?;

        let history = Self::load_history(&model_path).unwrap_or_default();

        Ok(Self {
            pattern_index,
            history,
            model_path,
        })
    }

    pub fn add_pattern(&mut self, pattern: &LabeledPattern) -> Result<(), String> {
        let vec = pattern.pattern.to_vector();
        self.pattern_index.add(&vec)?;
        self.history.push(pattern.clone());

        // Auto-save every 50 patterns
        if self.history.len() % 50 == 0 {
            let _ = self.save_history();
        }

        Ok(())
    }

    pub fn pattern_count(&self) -> usize {
        self.history.len()
    }

    fn load_history(path: &PathBuf) -> Result<Vec<LabeledPattern>, String> {
        let file_path = path.join("android_patterns.json");
        if !file_path.exists() {
            return Ok(vec![]);
        }
        let content = std::fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    fn save_history(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.model_path).map_err(|e| e.to_string())?;
        let content = serde_json::to_string_pretty(&self.history).map_err(|e| e.to_string())?;
        std::fs::write(self.model_path.join("android_patterns.json"), content)
            .map_err(|e| e.to_string())
    }
}

// Global neural engine instance (lazy initialized)
static NEURAL_ENGINE: OnceLock<Arc<Mutex<Option<AndroidNeuralEngine>>>> = OnceLock::new();

fn get_neural_engine_lock() -> &'static Arc<Mutex<Option<AndroidNeuralEngine>>> {
    NEURAL_ENGINE.get_or_init(|| Arc::new(Mutex::new(None)))
}

/// Android-specific memory status using sysinfo
#[derive(Debug, Clone, Serialize)]
pub struct AndroidMemoryStatus {
    pub total_physical_mb: f64,
    pub available_physical_mb: f64,
    pub memory_load_percent: u32,
    pub used_physical_mb: f64,
    pub is_high_pressure: bool,
    pub is_critical: bool,
}

impl AndroidMemoryStatus {
    pub fn current() -> Result<Self, String> {
        use sysinfo::System;

        let mut sys = System::new();
        sys.refresh_memory();

        let total = sys.total_memory() as f64 / 1024.0 / 1024.0;
        let available = sys.available_memory() as f64 / 1024.0 / 1024.0;
        let used = total - available;
        let load = if total > 0.0 {
            ((used / total) * 100.0) as u32
        } else {
            0
        };

        Ok(Self {
            total_physical_mb: total,
            available_physical_mb: available,
            memory_load_percent: load,
            used_physical_mb: used,
            is_high_pressure: load > 80,
            is_critical: load > 95,
        })
    }

    /// Convert to MemoryPattern for neural engine compatibility
    pub fn to_memory_pattern(&self) -> MemoryPattern {
        use chrono::{Datelike, Timelike};
        let now = chrono::Local::now();

        MemoryPattern {
            load: self.memory_load_percent as f32 / 100.0,
            consumption_rate: 0.0,
            available_ratio: (self.available_physical_mb / self.total_physical_mb) as f32,
            page_file_ratio: 0.0, // Android uses zRAM differently
            process_count: 0,     // Would require additional permissions
            hour: now.hour() as u8,
            day_of_week: now.weekday().num_days_from_monday() as u8,
            time_since_last_opt: 0.0,
        }
    }
}

/// Android process info for listing
#[derive(Debug, Clone, Serialize)]
pub struct AndroidProcessInfo {
    pub pid: u32,
    pub name: String,
    pub memory_mb: f64,
    pub cpu_percent: f32,
}

/// Get list of processes using sysinfo (read-only)
pub fn get_android_processes() -> Result<Vec<AndroidProcessInfo>, String> {
    use sysinfo::{ProcessRefreshKind, RefreshKind, System};

    let sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );

    let mut processes: Vec<AndroidProcessInfo> = sys
        .processes()
        .iter()
        .map(|(pid, proc)| AndroidProcessInfo {
            pid: pid.as_u32(),
            name: proc.name().to_string_lossy().to_string(),
            memory_mb: proc.memory() as f64 / 1024.0 / 1024.0,
            cpu_percent: proc.cpu_usage(),
        })
        .collect();

    // Sort by memory usage descending
    processes.sort_by(|a, b| b.memory_mb.partial_cmp(&a.memory_mb).unwrap_or(std::cmp::Ordering::Equal));

    Ok(processes)
}

/// Optimization recommendations for Android (read-only analysis)
#[derive(Debug, Clone, Serialize)]
pub struct OptimizationRecommendation {
    pub should_optimize: bool,
    pub severity: String,
    pub confidence: f32,
    pub reason: String,
    pub top_memory_consumers: Vec<String>,
    pub suggested_actions: Vec<String>,
}

pub fn get_recommendations() -> Result<OptimizationRecommendation, String> {
    let status = AndroidMemoryStatus::current()?;
    let processes = get_android_processes()?;

    let top_consumers: Vec<String> = processes
        .iter()
        .take(5)
        .map(|p| format!("{} ({:.1} MB)", p.name, p.memory_mb))
        .collect();

    let (severity, should_optimize, confidence) = if status.is_critical {
        ("critical".to_string(), true, 0.95)
    } else if status.is_high_pressure {
        ("high".to_string(), true, 0.80)
    } else if status.memory_load_percent > 60 {
        ("moderate".to_string(), false, 0.60)
    } else {
        ("low".to_string(), false, 0.40)
    };

    let mut suggested_actions = Vec::new();

    if status.is_critical {
        suggested_actions.push("Close background apps immediately".to_string());
        suggested_actions.push("Consider restarting device".to_string());
    } else if status.is_high_pressure {
        suggested_actions.push("Close unused background apps".to_string());
        suggested_actions.push("Clear app caches".to_string());
    } else {
        suggested_actions.push("Memory usage is healthy".to_string());
    }

    Ok(OptimizationRecommendation {
        should_optimize,
        severity,
        confidence,
        reason: format!(
            "Memory load at {}% ({:.0} MB available of {:.0} MB)",
            status.memory_load_percent, status.available_physical_mb, status.total_physical_mb
        ),
        top_memory_consumers: top_consumers,
        suggested_actions,
    })
}

// ============================================================================
// JNI Helper Functions
// ============================================================================

/// Convert Rust string to JNI jstring, handling errors
fn rust_string_to_jstring(env: &mut JNIEnv, s: &str) -> jstring {
    match env.new_string(s) {
        Ok(js) => js.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Convert JNI jstring to Rust String
fn jstring_to_rust_string(env: &mut JNIEnv, js: JString) -> Result<String, String> {
    env.get_string(&js)
        .map(|s| s.into())
        .map_err(|e| format!("Failed to convert JNI string: {}", e))
}

/// Throw a Java exception with the given message
fn throw_java_exception(env: &mut JNIEnv, message: &str) {
    let _ = env.throw_new("java/lang/RuntimeException", message);
}

/// Wrap a function call with panic catching and exception throwing
fn catch_panic_and_throw<F, T>(env: &mut JNIEnv, default: T, f: F) -> T
where
    F: FnOnce() -> Result<T, String>,
{
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => {
            throw_java_exception(env, &error);
            default
        }
        Err(_) => {
            throw_java_exception(env, "Rust panic occurred");
            default
        }
    }
}

// ============================================================================
// JNI Entry Points
// ============================================================================

/// Get current memory status as JSON
///
/// # Returns
/// JSON string with memory status:
/// ```json
/// {
///   "total_physical_mb": 8192.0,
///   "available_physical_mb": 4096.0,
///   "memory_load_percent": 50,
///   "used_physical_mb": 4096.0,
///   "is_high_pressure": false,
///   "is_critical": false
/// }
/// ```
#[no_mangle]
pub extern "system" fn Java_com_ruvector_memopt_NativeLib_getMemoryStatus<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    catch_panic_and_throw(&mut env, std::ptr::null_mut(), || {
        let status = AndroidMemoryStatus::current()?;
        let json = serde_json::to_string(&status)
            .map_err(|e| format!("JSON serialization failed: {}", e))?;

        // Need to reborrow env for the closure
        Ok(rust_string_to_jstring(&mut env, &json))
    })
}

/// Get list of processes as JSON array
///
/// # Returns
/// JSON array of process objects:
/// ```json
/// [
///   {"pid": 1234, "name": "app", "memory_mb": 256.5, "cpu_percent": 2.5},
///   ...
/// ]
/// ```
#[no_mangle]
pub extern "system" fn Java_com_ruvector_memopt_NativeLib_getProcessList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    catch_panic_and_throw(&mut env, std::ptr::null_mut(), || {
        let processes = get_android_processes()?;
        let json = serde_json::to_string(&processes)
            .map_err(|e| format!("JSON serialization failed: {}", e))?;

        Ok(rust_string_to_jstring(&mut env, &json))
    })
}

/// Run optimization analysis (read-only on Android)
///
/// This performs analysis only - Android does not allow direct memory manipulation
/// from userspace apps without root. Returns analysis results as JSON.
///
/// # Returns
/// JSON with analysis results:
/// ```json
/// {
///   "analyzed": true,
///   "memory_status": {...},
///   "recommendation": {...},
///   "message": "Analysis complete (read-only mode)"
/// }
/// ```
#[no_mangle]
pub extern "system" fn Java_com_ruvector_memopt_NativeLib_runOptimization<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    catch_panic_and_throw(&mut env, std::ptr::null_mut(), || {
        let status = AndroidMemoryStatus::current()?;
        let recommendation = get_recommendations()?;

        #[derive(Serialize)]
        struct AnalysisResult {
            analyzed: bool,
            memory_status: AndroidMemoryStatus,
            recommendation: OptimizationRecommendation,
            message: String,
        }

        let result = AnalysisResult {
            analyzed: true,
            memory_status: status,
            recommendation,
            message: "Analysis complete (read-only mode - Android restricts memory manipulation)"
                .to_string(),
        };

        let json = serde_json::to_string(&result)
            .map_err(|e| format!("JSON serialization failed: {}", e))?;

        Ok(rust_string_to_jstring(&mut env, &json))
    })
}

/// Get optimization recommendations as JSON
///
/// # Returns
/// JSON with recommendations:
/// ```json
/// {
///   "should_optimize": true,
///   "severity": "high",
///   "confidence": 0.80,
///   "reason": "Memory load at 85%",
///   "top_memory_consumers": ["Chrome (512 MB)", ...],
///   "suggested_actions": ["Close background apps", ...]
/// }
/// ```
#[no_mangle]
pub extern "system" fn Java_com_ruvector_memopt_NativeLib_getRecommendations<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    catch_panic_and_throw(&mut env, std::ptr::null_mut(), || {
        let recommendation = get_recommendations()?;
        let json = serde_json::to_string(&recommendation)
            .map_err(|e| format!("JSON serialization failed: {}", e))?;

        Ok(rust_string_to_jstring(&mut env, &json))
    })
}

/// Initialize the neural decision engine
///
/// # Arguments
/// * `model_path` - Path to store/load neural model data (JString)
///
/// # Returns
/// `true` (JNI_TRUE) if initialization succeeded, `false` (JNI_FALSE) otherwise
#[no_mangle]
pub extern "system" fn Java_com_ruvector_memopt_NativeLib_initNeuralEngine<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    model_path: JString<'local>,
) -> jboolean {
    catch_panic_and_throw(&mut env, JNI_FALSE, || {
        let path_str = jstring_to_rust_string(&mut env, model_path)?;
        let path = PathBuf::from(path_str);

        let engine = AndroidNeuralEngine::new(path)?;

        let engine_lock = get_neural_engine_lock();
        let mut guard = engine_lock
            .lock()
            .map_err(|_| "Failed to acquire neural engine lock")?;
        *guard = Some(engine);

        Ok(JNI_TRUE)
    })
}

/// Train the neural engine on an observed pattern
///
/// # Arguments
/// * `pattern_json` - JSON string representing the pattern to train on:
///   ```json
///   {
///     "load": 0.85,
///     "success": true,
///     "freed_mb": 256.0,
///     "aggressive": false
///   }
///   ```
///
/// # Returns
/// `true` if training succeeded, `false` otherwise
#[no_mangle]
pub extern "system" fn Java_com_ruvector_memopt_NativeLib_trainPattern<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    pattern_json: JString<'local>,
) -> jboolean {
    catch_panic_and_throw(&mut env, JNI_FALSE, || {
        let json_str = jstring_to_rust_string(&mut env, pattern_json)?;

        #[derive(serde::Deserialize)]
        struct TrainingInput {
            load: f32,
            success: bool,
            freed_mb: f32,
            aggressive: bool,
            #[serde(default)]
            consumption_rate: f32,
            #[serde(default)]
            available_ratio: f32,
            #[serde(default)]
            page_file_ratio: f32,
            #[serde(default)]
            process_count: u32,
        }

        let input: TrainingInput = serde_json::from_str(&json_str)
            .map_err(|e| format!("Invalid pattern JSON: {}", e))?;

        // Get current time for temporal features
        use chrono::{Datelike, Timelike};
        let now = chrono::Local::now();

        let pattern = crate::core::patterns::LabeledPattern {
            pattern: MemoryPattern {
                load: input.load,
                consumption_rate: input.consumption_rate,
                available_ratio: input.available_ratio,
                page_file_ratio: input.page_file_ratio,
                process_count: input.process_count,
                hour: now.hour() as u8,
                day_of_week: now.weekday().num_days_from_monday() as u8,
                time_since_last_opt: 0.0,
            },
            optimized: true,
            aggressive: input.aggressive,
            freed_mb: input.freed_mb,
            success: input.success,
        };

        let engine_lock = get_neural_engine_lock();
        let mut guard = engine_lock
            .lock()
            .map_err(|_| "Failed to acquire neural engine lock")?;

        if let Some(ref mut engine) = *guard {
            // Add pattern to the index
            let vec = pattern.pattern.to_vector();
            engine
                .pattern_index
                .add(&vec)
                .map_err(|e| format!("Failed to add pattern: {}", e))?;

            Ok(JNI_TRUE)
        } else {
            Err("Neural engine not initialized. Call initNeuralEngine first.".to_string())
        }
    })
}

/// Get the current pattern count from the neural engine
///
/// # Returns
/// Number of patterns learned, or -1 if engine not initialized
#[no_mangle]
pub extern "system" fn Java_com_ruvector_memopt_NativeLib_getPatternCount<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jlong {
    catch_panic_and_throw(&mut env, -1, || {
        let engine_lock = get_neural_engine_lock();
        let guard = engine_lock
            .lock()
            .map_err(|_| "Failed to acquire neural engine lock")?;

        if let Some(ref engine) = *guard {
            Ok(engine.pattern_count() as jlong)
        } else {
            Ok(-1)
        }
    })
}

/// Get neural engine status as JSON
///
/// # Returns
/// JSON with engine status:
/// ```json
/// {
///   "initialized": true,
///   "pattern_count": 150,
///   "ready": true
/// }
/// ```
#[no_mangle]
pub extern "system" fn Java_com_ruvector_memopt_NativeLib_getNeuralStatus<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    catch_panic_and_throw(&mut env, std::ptr::null_mut(), || {
        #[derive(Serialize)]
        struct NeuralStatus {
            initialized: bool,
            pattern_count: i64,
            ready: bool,
        }

        let engine_lock = get_neural_engine_lock();
        let guard = engine_lock
            .lock()
            .map_err(|_| "Failed to acquire neural engine lock")?;

        let status = if let Some(ref engine) = *guard {
            NeuralStatus {
                initialized: true,
                pattern_count: engine.pattern_count() as i64,
                ready: true,
            }
        } else {
            NeuralStatus {
                initialized: false,
                pattern_count: 0,
                ready: false,
            }
        };

        let json = serde_json::to_string(&status)
            .map_err(|e| format!("JSON serialization failed: {}", e))?;

        Ok(rust_string_to_jstring(&mut env, &json))
    })
}

// ============================================================================
// Library Initialization
// ============================================================================

/// Called when the native library is loaded
/// Initialize Android logging
#[no_mangle]
pub extern "system" fn JNI_OnLoad(
    _vm: jni::JavaVM,
    _reserved: *mut std::ffi::c_void,
) -> jni::sys::jint {
    // Initialize Android logger
    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("RuVectorMemOpt"),
        );
    }

    jni::sys::JNI_VERSION_1_6
}

// ============================================================================
// Tests (run on host, not Android)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_android_memory_status() {
        // This will work on any platform for testing
        let status = AndroidMemoryStatus::current();
        assert!(status.is_ok());

        let status = status.unwrap();
        assert!(status.total_physical_mb > 0.0);
        assert!(status.memory_load_percent <= 100);
    }

    #[test]
    fn test_get_recommendations() {
        let rec = get_recommendations();
        assert!(rec.is_ok());

        let rec = rec.unwrap();
        assert!(!rec.severity.is_empty());
        assert!(!rec.reason.is_empty());
    }

    #[test]
    fn test_process_list() {
        let processes = get_android_processes();
        assert!(processes.is_ok());
    }
}
