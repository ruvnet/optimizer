//! Dashboard data structures
//!
//! These structures are designed to be serializable for WASM/JSON transport

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Real-time system metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub timestamp_ms: u64,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub memory_percent: f64,
    pub swap_used_mb: u64,
    pub swap_total_mb: u64,
    pub process_count: usize,
    pub optimization_count: u32,
    pub total_freed_mb: f64,
}

/// Algorithm performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgorithmMetrics {
    pub name: String,
    pub last_run_us: u64,
    pub avg_run_us: f64,
    pub calls: u64,
    pub success_rate: f64,
}

/// Process cluster information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterInfo {
    pub id: usize,
    pub process_count: usize,
    pub total_memory_mb: f64,
    pub connectivity: f64,
    pub top_processes: Vec<ProcessInfo>,
}

/// Individual process information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub memory_mb: f64,
    pub pagerank_score: f64,
    pub trim_priority: f64,
}

/// Spectral analysis state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralState {
    pub pattern_class: String,
    pub trend: f64,
    pub variance: f64,
    pub recommendation: String,
    pub confidence: f64,
    pub predicted_relief_mb: u64,
}

/// Count-Min Sketch statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SketchStats {
    pub total_events: u64,
    pub memory_bytes: usize,
    pub fill_ratio: f64,
    pub peak_hours: Vec<(u32, u64)>,
}

/// Complete dashboard data packet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub metrics: SystemMetrics,
    pub algorithms: Vec<AlgorithmMetrics>,
    pub clusters: Vec<ClusterInfo>,
    pub spectral: SpectralState,
    pub sketch: SketchStats,
    pub history: Vec<HistoryPoint>,
}

/// Historical data point for charts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryPoint {
    pub timestamp_ms: u64,
    pub memory_percent: f64,
    pub freed_mb: f64,
}

/// Incremental update for websocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardUpdate {
    pub update_type: UpdateType,
    pub timestamp_ms: u64,
    pub data: UpdateData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateType {
    Metrics,
    Optimization,
    Alert,
    PatternChange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateData {
    Metrics(SystemMetrics),
    Optimization { freed_mb: f64, duration_ms: u64 },
    Alert { level: String, message: String },
    Pattern { old: String, new: String },
}

/// Dashboard data collector
pub struct DashboardCollector {
    history: VecDeque<HistoryPoint>,
    max_history: usize,
    optimization_count: u32,
    total_freed_mb: f64,
    algorithm_stats: Vec<AlgorithmMetrics>,
}

impl DashboardCollector {
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(3600),
            max_history: 3600,
            optimization_count: 0,
            total_freed_mb: 0.0,
            algorithm_stats: vec![
                AlgorithmMetrics {
                    name: "MinCut".into(),
                    last_run_us: 0,
                    avg_run_us: 0.0,
                    calls: 0,
                    success_rate: 1.0,
                },
                AlgorithmMetrics {
                    name: "PageRank".into(),
                    last_run_us: 0,
                    avg_run_us: 0.0,
                    calls: 0,
                    success_rate: 1.0,
                },
                AlgorithmMetrics {
                    name: "Sketch".into(),
                    last_run_us: 0,
                    avg_run_us: 0.0,
                    calls: 0,
                    success_rate: 1.0,
                },
                AlgorithmMetrics {
                    name: "Spectral".into(),
                    last_run_us: 0,
                    avg_run_us: 0.0,
                    calls: 0,
                    success_rate: 1.0,
                },
            ],
        }
    }

    /// Record a memory sample
    pub fn record_sample(&mut self, memory_percent: f64) {
        let point = HistoryPoint {
            timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
            memory_percent,
            freed_mb: 0.0,
        };

        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(point);
    }

    /// Record an optimization
    pub fn record_optimization(&mut self, freed_mb: f64) {
        self.optimization_count += 1;
        self.total_freed_mb += freed_mb;

        if let Some(last) = self.history.back_mut() {
            last.freed_mb = freed_mb;
        }
    }

    /// Record algorithm timing
    pub fn record_algorithm(&mut self, name: &str, duration_us: u64, success: bool) {
        if let Some(stats) = self.algorithm_stats.iter_mut().find(|s| s.name == name) {
            stats.calls += 1;
            stats.last_run_us = duration_us;
            stats.avg_run_us =
                (stats.avg_run_us * (stats.calls - 1) as f64 + duration_us as f64) / stats.calls as f64;
            if !success {
                stats.success_rate =
                    (stats.success_rate * (stats.calls - 1) as f64) / stats.calls as f64;
            }
        }
    }

    /// Get full dashboard data
    pub fn get_data(&self, metrics: SystemMetrics, clusters: Vec<ClusterInfo>, spectral: SpectralState, sketch: SketchStats) -> DashboardData {
        DashboardData {
            metrics,
            algorithms: self.algorithm_stats.clone(),
            clusters,
            spectral,
            sketch,
            history: self.history.iter().cloned().collect(),
        }
    }

    /// Get recent history
    pub fn get_history(&self, count: usize) -> Vec<HistoryPoint> {
        self.history.iter().rev().take(count).cloned().collect()
    }

    /// Get statistics
    pub fn stats(&self) -> CollectorStats {
        CollectorStats {
            history_count: self.history.len(),
            optimization_count: self.optimization_count,
            total_freed_mb: self.total_freed_mb,
        }
    }
}

impl Default for DashboardCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct CollectorStats {
    pub history_count: usize,
    pub optimization_count: u32,
    pub total_freed_mb: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collector() {
        let mut collector = DashboardCollector::new();

        collector.record_sample(50.0);
        collector.record_sample(55.0);
        collector.record_optimization(100.0);
        collector.record_algorithm("MinCut", 1000, true);

        let stats = collector.stats();
        assert_eq!(stats.history_count, 2);
        assert_eq!(stats.optimization_count, 1);
        assert_eq!(stats.total_freed_mb, 100.0);
    }

    #[test]
    fn test_serialization() {
        let metrics = SystemMetrics {
            timestamp_ms: 0,
            memory_used_mb: 8000,
            memory_total_mb: 16000,
            memory_percent: 50.0,
            swap_used_mb: 0,
            swap_total_mb: 0,
            process_count: 100,
            optimization_count: 5,
            total_freed_mb: 500.0,
        };

        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("memory_used_mb"));
    }
}
