//! Hardware acceleration detection and optimization

pub mod cpu;
pub mod simd;

pub use cpu::CpuCapabilities;
pub use simd::SimdOptimizer;
