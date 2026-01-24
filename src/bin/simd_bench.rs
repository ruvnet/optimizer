//! SIMD Benchmark utility

use ruvector_memopt::accel::{CpuCapabilities, SimdOptimizer};
use std::time::Instant;

fn main() {
    println!("=== RuVector SIMD Benchmark ===\n");
    
    // Detect CPU capabilities
    let caps = CpuCapabilities::detect();
    caps.print_report();
    
    println!("\n=== SIMD vs Scalar Benchmark ===\n");
    
    let simd = SimdOptimizer::new();
    
    // Test various dimensions
    for dim in [8, 64, 256, 1024, 4096] {
        let iterations = 100000;
        let (scalar_time, simd_time, speedup) = simd.benchmark(dim, iterations);
        
        println!("Dimension {dim:5}:");
        println!("  Scalar:  {:.4}s ({:.0} ops/sec)", scalar_time, iterations as f64 / scalar_time);
        println!("  SIMD:    {:.4}s ({:.0} ops/sec)", simd_time, iterations as f64 / simd_time);
        println!("  Speedup: {:.2}x\n", speedup);
    }
    
    // Memory pattern dimension benchmark (8-dim vectors used by neural engine)
    println!("=== Memory Pattern Search Benchmark (8-dim) ===\n");
    let dim = 8;
    let num_patterns = 10000;
    
    // Create sample patterns
    let patterns: Vec<Vec<f32>> = (0..num_patterns)
        .map(|i| (0..dim).map(|j| (i as f32 * 0.01 + j as f32 * 0.1) % 1.0).collect())
        .collect();
    
    let query: Vec<f32> = vec![0.5; dim];
    
    // Benchmark batch distances
    let start = Instant::now();
    let _distances = simd.batch_distances(&query, &patterns);
    let elapsed = start.elapsed();
    
    println!("Searched {} patterns in {:.3}ms", num_patterns, elapsed.as_secs_f64() * 1000.0);
    println!("Throughput: {:.0} patterns/sec", num_patterns as f64 / elapsed.as_secs_f64());
    
    // Dot product benchmark
    println!("\n=== Dot Product Benchmark ===\n");
    let a: Vec<f32> = (0..1024).map(|i| i as f32 * 0.001).collect();
    let b: Vec<f32> = (0..1024).map(|i| (1024 - i) as f32 * 0.001).collect();
    
    let iterations = 100000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = simd.dot_product(&a, &b);
    }
    let elapsed = start.elapsed();
    println!("1024-dim dot product: {:.0} ops/sec", iterations as f64 / elapsed.as_secs_f64());
}
