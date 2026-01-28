//! Benchmark suite for memory optimization
//!
//! Includes advanced RuVector algorithm benchmarks:
//! - MinCut process clustering
//! - PageRank priority scoring
//! - Count-Min Sketch sublinear counting
//! - Spectral pattern analysis

pub mod metrics;
#[cfg(target_os = "windows")]
pub mod runner;
pub mod advanced;

pub use advanced::{AdvancedBenchmarkRunner, BenchmarkSuite, AlgorithmBenchmark};
