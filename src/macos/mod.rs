//! macOS Memory Management APIs with Safety Guards
//!
//! Provides memory optimization for macOS using:
//! - Mach VM statistics for memory status
//! - madvise hints for memory pressure
//! - Process memory management via BSD APIs
//! - Apple Silicon unified memory awareness

pub mod memory;
pub mod process;
pub mod safety;
pub mod tray;

pub use memory::*;
pub use process::*;
pub use safety::*;
pub use tray::MacTrayApp;
