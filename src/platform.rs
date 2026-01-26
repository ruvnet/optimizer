//! Platform-agnostic type aliases for cross-platform compilation
//!
//! This module provides unified type aliases that resolve to the appropriate
//! platform-specific implementations at compile time.
//!
//! # Supported Platforms
//!
//! - **Windows**: Full memory optimization with working set trimming
//! - **macOS**: Memory optimization with purge and signal-based reclamation
//!
//! # Usage
//!
//! ```ignore
//! use optimizer::platform::{MemoryOptimizer, MemoryStatus, OptimizationResult};
//!
//! let optimizer = MemoryOptimizer::new();
//! let status = MemoryOptimizer::get_memory_status()?;
//! let result = optimizer.optimize(false)?;
//! ```

// Memory optimizer type alias
#[cfg(target_os = "windows")]
pub type MemoryOptimizer = crate::windows::memory::WindowsMemoryOptimizer;

#[cfg(target_os = "macos")]
pub type MemoryOptimizer = crate::macos::memory::MacOSMemoryOptimizer;

// Memory status type alias
#[cfg(target_os = "windows")]
pub type MemoryStatus = crate::windows::memory::MemoryStatus;

#[cfg(target_os = "macos")]
pub type MemoryStatus = crate::macos::memory::MemoryStatus;

// Optimization result type alias
#[cfg(target_os = "windows")]
pub type OptimizationResult = crate::windows::memory::OptimizationResult;

#[cfg(target_os = "macos")]
pub type OptimizationResult = crate::macos::memory::OptimizationResult;

// Compile-time error for unsupported platforms
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
compile_error!(
    "This crate only supports Windows and macOS. \
     Linux support is not yet implemented."
);

/// Returns the current platform name
pub fn platform_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }

    #[cfg(target_os = "macos")]
    {
        "macos"
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        "unsupported"
    }
}

/// Check if the current platform is supported
pub fn is_platform_supported() -> bool {
    cfg!(any(target_os = "windows", target_os = "macos"))
}

/// Returns true if running on Windows
pub fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

/// Returns true if running on macOS
pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}
