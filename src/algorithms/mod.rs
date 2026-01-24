//! Advanced algorithms for intelligent memory optimization
//!
//! This module provides graph-based and sublinear algorithms:
//! - MinCut: Process clustering for grouped optimization
//! - PageRank: Process importance scoring
//! - Count-Min Sketch: Sublinear frequency estimation
//! - Spectral Analysis: Memory pattern classification

pub mod mincut;
pub mod pagerank;
pub mod sketch;
pub mod spectral;

pub use mincut::MinCutClusterer;
pub use pagerank::ProcessPageRank;
pub use sketch::CountMinSketch;
pub use spectral::SpectralAnalyzer;
