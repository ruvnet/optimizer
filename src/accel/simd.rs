//! SIMD-optimized operations for pattern matching

use super::cpu::CpuCapabilities;

/// SIMD optimizer for vector operations
pub struct SimdOptimizer {
    caps: CpuCapabilities,
}

impl SimdOptimizer {
    pub fn new() -> Self {
        Self { caps: CpuCapabilities::detect() }
    }

    /// SIMD-optimized Euclidean distance calculation
    #[cfg(target_arch = "x86_64")]
    pub fn euclidean_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() { return f32::MAX; }

        if self.caps.has_avx2 && a.len() >= 8 {
            // SAFETY: We've checked has_avx2 is true
            unsafe { self.euclidean_distance_avx2(a, b) }
        } else {
            self.euclidean_distance_scalar(a, b)
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub fn euclidean_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        self.euclidean_distance_scalar(a, b)
    }

    fn euclidean_distance_scalar(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn euclidean_distance_avx2(&self, a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let len = a.len();
        let chunks = len / 8;
        let mut sum = _mm256_setzero_ps();

        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm256_loadu_ps(b.as_ptr().add(offset));
            let diff = _mm256_sub_ps(va, vb);
            let sq = _mm256_mul_ps(diff, diff);
            sum = _mm256_add_ps(sum, sq);
        }

        // Horizontal sum
        let high = _mm256_extractf128_ps(sum, 1);
        let low = _mm256_castps256_ps128(sum);
        let sum128 = _mm_add_ps(low, high);
        let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
        let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));

        let mut result = _mm_cvtss_f32(sum32);

        // Handle remainder
        for i in (chunks * 8)..len {
            let diff = a[i] - b[i];
            result += diff * diff;
        }

        result.sqrt()
    }

    /// SIMD-optimized dot product
    pub fn dot_product(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() { return 0.0; }

        #[cfg(target_arch = "x86_64")]
        if self.caps.has_avx2 && a.len() >= 8 {
            return unsafe { self.dot_product_avx2(a, b) };
        }

        self.dot_product_scalar(a, b)
    }

    fn dot_product_scalar(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2", enable = "fma")]
    unsafe fn dot_product_avx2(&self, a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let len = a.len().min(b.len());
        let chunks = len / 8;
        let mut sum = _mm256_setzero_ps();

        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm256_loadu_ps(b.as_ptr().add(offset));
            sum = _mm256_fmadd_ps(va, vb, sum);
        }

        // Horizontal sum
        let high = _mm256_extractf128_ps(sum, 1);
        let low = _mm256_castps256_ps128(sum);
        let sum128 = _mm_add_ps(low, high);
        let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
        let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));

        let mut result = _mm_cvtss_f32(sum32);

        for i in (chunks * 8)..len {
            result += a[i] * b[i];
        }

        result
    }

    /// Batch distance calculation
    pub fn batch_distances(&self, query: &[f32], vectors: &[Vec<f32>]) -> Vec<f32> {
        vectors.iter()
            .map(|v| self.euclidean_distance(query, v))
            .collect()
    }

    /// Benchmark SIMD vs scalar
    pub fn benchmark(&self, dim: usize, iterations: usize) -> (f64, f64, f64) {
        use std::time::Instant;
        use std::hint::black_box;

        let a: Vec<f32> = (0..dim).map(|i| i as f32 * 0.1).collect();
        let b: Vec<f32> = (0..dim).map(|i| (dim - i) as f32 * 0.1).collect();

        // Warmup
        for _ in 0..100 {
            let _ = black_box(self.euclidean_distance_scalar(black_box(&a), black_box(&b)));
        }

        // Scalar benchmark
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = black_box(self.euclidean_distance_scalar(black_box(&a), black_box(&b)));
        }
        let scalar_time = start.elapsed().as_secs_f64();

        // SIMD benchmark (on ARM, this also uses scalar path but auto-vectorized by LLVM)
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = black_box(self.euclidean_distance(black_box(&a), black_box(&b)));
        }
        let simd_time = start.elapsed().as_secs_f64();

        // Avoid NaN/Inf
        let speedup = if simd_time > 0.0 { scalar_time / simd_time } else { 1.0 };
        (scalar_time, simd_time, speedup)
    }

    pub fn capabilities(&self) -> &CpuCapabilities { &self.caps }
}

impl Default for SimdOptimizer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_euclidean_distance() {
        let opt = SimdOptimizer::new();
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        assert!((opt.euclidean_distance(&a, &b) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_dot_product() {
        let opt = SimdOptimizer::new();
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        assert!((opt.dot_product(&a, &b) - 36.0).abs() < 0.001);
    }
}
