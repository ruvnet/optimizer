//! Optimization metrics tracking

use std::collections::VecDeque;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct OptimizationMetrics {
    pub freed_mb: f64,
    pub processes_trimmed: usize,
    pub duration_ms: u64,
    pub aggressive: bool,
    pub confidence: f32,
}

pub struct BenchmarkMetrics {
    history: VecDeque<OptimizationMetrics>,
    start_time: Instant,
    total_freed_mb: f64,
    total_optimizations: usize,
    max_history: usize,
}

impl BenchmarkMetrics {
    pub fn new() -> Self {
        Self {
            history: VecDeque::new(),
            start_time: Instant::now(),
            total_freed_mb: 0.0,
            total_optimizations: 0,
            max_history: 1000,
        }
    }
    
    pub fn record_optimization(&mut self, metrics: &OptimizationMetrics) {
        self.total_freed_mb += metrics.freed_mb;
        self.total_optimizations += 1;
        
        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(metrics.clone());
    }
    
    pub fn summary(&self) -> MetricsSummary {
        let avg_freed = if self.total_optimizations > 0 {
            self.total_freed_mb / self.total_optimizations as f64
        } else { 0.0 };
        
        let avg_duration = if !self.history.is_empty() {
            self.history.iter().map(|m| m.duration_ms).sum::<u64>() / self.history.len() as u64
        } else { 0 };
        
        MetricsSummary {
            total_freed_mb: self.total_freed_mb,
            total_optimizations: self.total_optimizations,
            avg_freed_mb: avg_freed,
            avg_duration_ms: avg_duration,
            uptime_secs: self.start_time.elapsed().as_secs(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetricsSummary {
    pub total_freed_mb: f64,
    pub total_optimizations: usize,
    pub avg_freed_mb: f64,
    pub avg_duration_ms: u64,
    pub uptime_secs: u64,
}

impl Default for BenchmarkMetrics {
    fn default() -> Self { Self::new() }
}
