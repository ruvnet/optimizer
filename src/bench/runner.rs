//! Benchmark runner for performance testing

use std::time::Instant;
use crate::windows::memory::WindowsMemoryOptimizer;

pub struct BenchmarkRunner {
    iterations: usize,
    warmup: usize,
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: usize,
    pub total_ms: u64,
    pub avg_ms: f64,
    pub min_ms: u64,
    pub max_ms: u64,
    pub ops_per_sec: f64,
}

impl BenchmarkRunner {
    pub fn new(iterations: usize) -> Self {
        Self { iterations, warmup: 5 }
    }
    
    pub fn run_memory_status_bench(&self) -> BenchmarkResult {
        // Warmup
        for _ in 0..self.warmup {
            let _ = WindowsMemoryOptimizer::get_memory_status();
        }
        
        let mut times = Vec::with_capacity(self.iterations);
        let start = Instant::now();
        
        for _ in 0..self.iterations {
            let iter_start = Instant::now();
            let _ = WindowsMemoryOptimizer::get_memory_status();
            times.push(iter_start.elapsed().as_micros() as u64);
        }
        
        let total = start.elapsed().as_millis() as u64;
        let min = times.iter().min().copied().unwrap_or(0) / 1000;
        let max = times.iter().max().copied().unwrap_or(0) / 1000;
        let avg = total as f64 / self.iterations as f64;
        let ops = self.iterations as f64 / (total as f64 / 1000.0);
        
        BenchmarkResult {
            name: "memory_status".into(),
            iterations: self.iterations,
            total_ms: total,
            avg_ms: avg,
            min_ms: min,
            max_ms: max,
            ops_per_sec: ops,
        }
    }
    
    pub fn run_all(&self) -> Vec<BenchmarkResult> {
        vec![self.run_memory_status_bench()]
    }
}
