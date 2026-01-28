//! Platform abstraction layer for cross-platform memory optimization
//!
//! Provides unified traits that abstract OS-specific implementations.

/// Memory status (cross-platform)
#[derive(Debug, Clone)]
pub struct MemoryStatus {
    pub total_physical_mb: f64,
    pub available_physical_mb: f64,
    pub memory_load_percent: u32,
}

impl MemoryStatus {
    pub fn used_physical_mb(&self) -> f64 {
        self.total_physical_mb - self.available_physical_mb
    }

    pub fn is_high_pressure(&self) -> bool {
        self.memory_load_percent > 80
    }

    pub fn is_critical(&self) -> bool {
        self.memory_load_percent > 95
    }
}

/// Optimization result (cross-platform)
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub freed_mb: f64,
    pub before_available_mb: f64,
    pub after_available_mb: f64,
    pub processes_affected: usize,
    pub duration_ms: u64,
}

/// Platform-agnostic memory optimizer trait
pub trait MemoryOptimizer: Send + Sync {
    /// Get current memory status
    fn get_memory_status(&self) -> Result<MemoryStatus, String>;

    /// Run memory optimization
    fn optimize(&self, aggressive: bool) -> Result<OptimizationResult, String>;

    /// Check if running with elevated privileges
    fn has_elevated_privileges(&self) -> bool;

    /// Get platform name
    fn platform_name(&self) -> &'static str;
}

/// Create platform-specific optimizer
pub fn create_optimizer() -> Box<dyn MemoryOptimizer> {
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsOptimizer::new())
    }

    #[cfg(target_os = "macos")]
    {
        Box::new(MacOptimizer::new())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Box::new(StubOptimizer::new())
    }
}

// ============================================================================
// Windows Implementation
// ============================================================================

#[cfg(target_os = "windows")]
pub struct WindowsOptimizer {
    inner: crate::windows::memory::WindowsMemoryOptimizer,
}

#[cfg(target_os = "windows")]
impl WindowsOptimizer {
    pub fn new() -> Self {
        Self {
            inner: crate::windows::memory::WindowsMemoryOptimizer::new(),
        }
    }
}

#[cfg(target_os = "windows")]
impl MemoryOptimizer for WindowsOptimizer {
    fn get_memory_status(&self) -> Result<MemoryStatus, String> {
        let status = crate::windows::memory::WindowsMemoryOptimizer::get_memory_status()?;
        Ok(MemoryStatus {
            total_physical_mb: status.total_physical_mb,
            available_physical_mb: status.available_physical_mb,
            memory_load_percent: status.memory_load_percent,
        })
    }

    fn optimize(&self, aggressive: bool) -> Result<OptimizationResult, String> {
        let result = self.inner.optimize(aggressive)?;
        Ok(OptimizationResult {
            freed_mb: result.freed_mb,
            before_available_mb: result.before_available_mb,
            after_available_mb: result.after_available_mb,
            processes_affected: result.processes_trimmed,
            duration_ms: result.duration_ms,
        })
    }

    fn has_elevated_privileges(&self) -> bool {
        self.inner.has_admin_privileges()
    }

    fn platform_name(&self) -> &'static str {
        "Windows"
    }
}

// ============================================================================
// macOS Implementation
// ============================================================================

#[cfg(target_os = "macos")]
pub struct MacOptimizer {
    inner: crate::macos::memory::MacMemoryOptimizer,
}

#[cfg(target_os = "macos")]
impl MacOptimizer {
    pub fn new() -> Self {
        Self {
            inner: crate::macos::memory::MacMemoryOptimizer::new(),
        }
    }
}

#[cfg(target_os = "macos")]
impl MemoryOptimizer for MacOptimizer {
    fn get_memory_status(&self) -> Result<MemoryStatus, String> {
        let status = crate::macos::memory::MacMemoryOptimizer::get_memory_status()?;
        Ok(MemoryStatus {
            total_physical_mb: status.total_physical_mb,
            available_physical_mb: status.available_physical_mb,
            memory_load_percent: status.memory_load_percent,
        })
    }

    fn optimize(&self, aggressive: bool) -> Result<OptimizationResult, String> {
        let result = self.inner.optimize(aggressive)?;
        Ok(OptimizationResult {
            freed_mb: result.freed_mb,
            before_available_mb: result.before_available_mb,
            after_available_mb: result.after_available_mb,
            processes_affected: result.processes_affected,
            duration_ms: result.duration_ms,
        })
    }

    fn has_elevated_privileges(&self) -> bool {
        self.inner.has_sudo_privileges()
    }

    fn platform_name(&self) -> &'static str {
        if self.inner.is_apple_silicon() {
            "macOS (Apple Silicon)"
        } else {
            "macOS (Intel)"
        }
    }
}

// ============================================================================
// Stub Implementation (for unsupported platforms)
// ============================================================================

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub struct StubOptimizer;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl StubOptimizer {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
impl MemoryOptimizer for StubOptimizer {
    fn get_memory_status(&self) -> Result<MemoryStatus, String> {
        use sysinfo::System;

        let mut sys = System::new();
        sys.refresh_memory();

        let total = sys.total_memory() as f64 / 1024.0 / 1024.0;
        let available = sys.available_memory() as f64 / 1024.0 / 1024.0;
        let load = if total > 0.0 {
            (((total - available) / total) * 100.0) as u32
        } else {
            0
        };

        Ok(MemoryStatus {
            total_physical_mb: total,
            available_physical_mb: available,
            memory_load_percent: load,
        })
    }

    fn optimize(&self, _aggressive: bool) -> Result<OptimizationResult, String> {
        Err("Memory optimization not supported on this platform".into())
    }

    fn has_elevated_privileges(&self) -> bool {
        unsafe { libc::geteuid() == 0 }
    }

    fn platform_name(&self) -> &'static str {
        "Linux (Limited Support)"
    }
}
