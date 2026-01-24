//! Count-Min Sketch for sublinear frequency estimation
//!
//! Provides O(1) space approximate frequency counting for memory patterns.
//! Used to detect frequent memory pressure events without storing all history.

use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use chrono::Timelike;

/// Count-Min Sketch for approximate frequency counting
/// Space: O(width * depth), Query: O(depth), Update: O(depth)
pub struct CountMinSketch {
    /// 2D array of counters
    counters: Vec<Vec<u64>>,
    /// Width of each row
    width: usize,
    /// Number of hash functions (rows)
    depth: usize,
    /// Total items added
    total_count: u64,
}

impl CountMinSketch {
    /// Create a new sketch with given error bounds
    /// - epsilon: error factor (smaller = more accurate, more space)
    /// - delta: probability of exceeding error bound
    pub fn new(epsilon: f64, delta: f64) -> Self {
        let width = (std::f64::consts::E / epsilon).ceil() as usize;
        let depth = (1.0 / delta).ln().ceil() as usize;

        Self::with_dimensions(width.max(64), depth.max(4))
    }

    /// Create with specific dimensions
    pub fn with_dimensions(width: usize, depth: usize) -> Self {
        Self {
            counters: vec![vec![0u64; width]; depth],
            width,
            depth,
            total_count: 0,
        }
    }

    /// Hash function for row i
    fn hash(&self, item: u64, row: usize) -> usize {
        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        (row as u64).hash(&mut hasher);
        (hasher.finish() as usize) % self.width
    }

    /// Add an item to the sketch
    pub fn add(&mut self, item: u64) {
        self.add_count(item, 1);
    }

    /// Add an item with a specific count
    pub fn add_count(&mut self, item: u64, count: u64) {
        for row in 0..self.depth {
            let col = self.hash(item, row);
            self.counters[row][col] = self.counters[row][col].saturating_add(count);
        }
        self.total_count = self.total_count.saturating_add(count);
    }

    /// Query the estimated count of an item
    pub fn estimate(&self, item: u64) -> u64 {
        let mut min_count = u64::MAX;
        for row in 0..self.depth {
            let col = self.hash(item, row);
            min_count = min_count.min(self.counters[row][col]);
        }
        min_count
    }

    /// Query the estimated frequency (count / total)
    pub fn frequency(&self, item: u64) -> f64 {
        if self.total_count == 0 {
            return 0.0;
        }
        self.estimate(item) as f64 / self.total_count as f64
    }

    /// Check if an item is likely frequent (above threshold)
    pub fn is_frequent(&self, item: u64, threshold: f64) -> bool {
        self.frequency(item) >= threshold
    }

    /// Get total items added
    pub fn total_count(&self) -> u64 {
        self.total_count
    }

    /// Memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        self.width * self.depth * std::mem::size_of::<u64>()
    }

    /// Reset all counters
    pub fn clear(&mut self) {
        for row in &mut self.counters {
            for counter in row {
                *counter = 0;
            }
        }
        self.total_count = 0;
    }

    /// Merge another sketch into this one
    pub fn merge(&mut self, other: &CountMinSketch) {
        if self.width != other.width || self.depth != other.depth {
            return;
        }
        for row in 0..self.depth {
            for col in 0..self.width {
                self.counters[row][col] =
                    self.counters[row][col].saturating_add(other.counters[row][col]);
            }
        }
        self.total_count = self.total_count.saturating_add(other.total_count);
    }

    /// Statistics
    pub fn stats(&self) -> SketchStats {
        let non_zero: usize = self.counters
            .iter()
            .flat_map(|row| row.iter())
            .filter(|&&c| c > 0)
            .count();

        SketchStats {
            width: self.width,
            depth: self.depth,
            total_count: self.total_count,
            memory_bytes: self.memory_usage(),
            fill_ratio: non_zero as f64 / (self.width * self.depth) as f64,
        }
    }
}

impl Default for CountMinSketch {
    fn default() -> Self {
        Self::new(0.01, 0.001) // 1% error with 99.9% probability
    }
}

#[derive(Debug, Clone)]
pub struct SketchStats {
    pub width: usize,
    pub depth: usize,
    pub total_count: u64,
    pub memory_bytes: usize,
    pub fill_ratio: f64,
}

/// Memory pressure pattern tracker using Count-Min Sketch
pub struct PressureTracker {
    /// Sketch for memory load percentages
    load_sketch: CountMinSketch,
    /// Sketch for process PIDs causing pressure
    process_sketch: CountMinSketch,
    /// Sketch for time-of-day patterns (hour buckets)
    time_sketch: CountMinSketch,
    /// High pressure threshold
    pressure_threshold: u32,
}

impl PressureTracker {
    pub fn new() -> Self {
        Self {
            load_sketch: CountMinSketch::new(0.01, 0.001),
            process_sketch: CountMinSketch::new(0.01, 0.001),
            time_sketch: CountMinSketch::with_dimensions(24, 4), // 24 hours
            pressure_threshold: 80,
        }
    }

    /// Record a memory pressure event
    pub fn record_pressure(&mut self, load_percent: u32, top_processes: &[u32]) {
        // Record load level
        self.load_sketch.add(load_percent as u64);

        // Record processes causing pressure
        for &pid in top_processes {
            self.process_sketch.add(pid as u64);
        }

        // Record time pattern
        let hour = chrono::Local::now().hour();
        self.time_sketch.add(hour as u64);
    }

    /// Check if a load level is frequently seen
    pub fn is_common_load(&self, load_percent: u32) -> bool {
        self.load_sketch.is_frequent(load_percent as u64, 0.05)
    }

    /// Get estimated frequency of a process causing pressure
    pub fn process_pressure_frequency(&self, pid: u32) -> f64 {
        self.process_sketch.frequency(pid as u64)
    }

    /// Get peak pressure hours
    pub fn get_peak_hours(&self) -> Vec<(u32, u64)> {
        let mut hours: Vec<(u32, u64)> = (0..24)
            .map(|h| (h, self.time_sketch.estimate(h as u64)))
            .collect();
        hours.sort_by(|a, b| b.1.cmp(&a.1));
        hours.into_iter().take(5).collect()
    }

    /// Statistics
    pub fn stats(&self) -> PressureTrackerStats {
        PressureTrackerStats {
            load_stats: self.load_sketch.stats(),
            process_stats: self.process_sketch.stats(),
            time_stats: self.time_sketch.stats(),
        }
    }
}

impl Default for PressureTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct PressureTrackerStats {
    pub load_stats: SketchStats,
    pub process_stats: SketchStats,
    pub time_stats: SketchStats,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_min_sketch() {
        let mut sketch = CountMinSketch::new(0.01, 0.001);

        // Add items
        for _ in 0..1000 {
            sketch.add(42);
        }
        for _ in 0..500 {
            sketch.add(100);
        }
        sketch.add(999);

        // Estimates should be >= actual counts
        assert!(sketch.estimate(42) >= 1000);
        assert!(sketch.estimate(100) >= 500);
        assert!(sketch.estimate(999) >= 1);

        // Frequency checks
        assert!(sketch.is_frequent(42, 0.5));
        assert!(!sketch.is_frequent(999, 0.1));
    }

    #[test]
    fn test_pressure_tracker() {
        let mut tracker = PressureTracker::new();

        // Record some pressure events
        tracker.record_pressure(85, &[1234, 5678]);
        tracker.record_pressure(90, &[1234]);
        tracker.record_pressure(75, &[9999]);

        // Process 1234 should be more frequent
        assert!(tracker.process_pressure_frequency(1234) > tracker.process_pressure_frequency(9999));
    }
}
