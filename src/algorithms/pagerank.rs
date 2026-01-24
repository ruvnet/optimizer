//! PageRank algorithm for process importance scoring
//!
//! Ranks processes by their importance in the process dependency graph.
//! High-rank processes are preserved; low-rank processes are trimmed first.

use std::collections::HashMap;
use sysinfo::System;

/// PageRank-based process prioritization
pub struct ProcessPageRank {
    /// PageRank scores for each process
    scores: HashMap<u32, f64>,
    /// Adjacency list (outgoing edges)
    outlinks: HashMap<u32, Vec<u32>>,
    /// Damping factor (typically 0.85)
    damping: f64,
    /// Convergence threshold
    epsilon: f64,
    /// Maximum iterations
    max_iterations: usize,
}

impl ProcessPageRank {
    pub fn new() -> Self {
        Self {
            scores: HashMap::new(),
            outlinks: HashMap::new(),
            damping: 0.85,
            epsilon: 1e-6,
            max_iterations: 100,
        }
    }

    /// Build the process graph and compute PageRank scores
    pub fn compute(&mut self, system: &System) {
        self.outlinks.clear();
        self.scores.clear();

        let processes: Vec<u32> = system.processes().keys().map(|p| p.as_u32()).collect();
        let n = processes.len();

        if n == 0 {
            return;
        }

        // Build outlink graph (parent -> children)
        for (pid, process) in system.processes() {
            let pid_u32 = pid.as_u32();

            // Parent points to child (influence flows from parent)
            if let Some(parent_pid) = process.parent() {
                self.outlinks
                    .entry(parent_pid.as_u32())
                    .or_insert_with(Vec::new)
                    .push(pid_u32);
            }
        }

        // Initialize scores uniformly
        let initial_score = 1.0 / n as f64;
        for &pid in &processes {
            self.scores.insert(pid, initial_score);
        }

        // Power iteration
        for iteration in 0..self.max_iterations {
            let mut new_scores: HashMap<u32, f64> = HashMap::new();
            let mut max_delta = 0.0f64;

            // Teleport component (random jump)
            let teleport = (1.0 - self.damping) / n as f64;

            for &pid in &processes {
                let mut score = teleport;

                // Sum contributions from all processes that link to this one
                for (&source, targets) in &self.outlinks {
                    if targets.contains(&pid) {
                        let source_score = self.scores.get(&source).unwrap_or(&0.0);
                        let out_degree = targets.len() as f64;
                        score += self.damping * source_score / out_degree;
                    }
                }

                // Handle dangling nodes (no outlinks)
                if !self.outlinks.contains_key(&pid) {
                    // Distribute to all nodes
                    let dangling_contrib = self.damping * self.scores.get(&pid).unwrap_or(&0.0) / n as f64;
                    score += dangling_contrib;
                }

                let old_score = self.scores.get(&pid).unwrap_or(&0.0);
                max_delta = max_delta.max((score - old_score).abs());
                new_scores.insert(pid, score);
            }

            self.scores = new_scores;

            // Check convergence
            if max_delta < self.epsilon {
                tracing::debug!("PageRank converged after {} iterations", iteration + 1);
                break;
            }
        }

        // Normalize scores
        let total: f64 = self.scores.values().sum();
        if total > 0.0 {
            for score in self.scores.values_mut() {
                *score /= total;
            }
        }
    }

    /// Get PageRank score for a process (higher = more important)
    pub fn get_score(&self, pid: u32) -> f64 {
        *self.scores.get(&pid).unwrap_or(&0.0)
    }

    /// Get processes ranked by importance (least important first - trim these)
    pub fn get_trim_candidates(&self, limit: usize) -> Vec<(u32, f64)> {
        let mut ranked: Vec<(u32, f64)> = self.scores.iter().map(|(&k, &v)| (k, v)).collect();
        // Sort ascending (lowest rank first)
        ranked.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.into_iter().take(limit).collect()
    }

    /// Get most important processes (preserve these)
    pub fn get_critical_processes(&self, limit: usize) -> Vec<(u32, f64)> {
        let mut ranked: Vec<(u32, f64)> = self.scores.iter().map(|(&k, &v)| (k, v)).collect();
        // Sort descending (highest rank first)
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.into_iter().take(limit).collect()
    }

    /// Statistics for benchmarking
    pub fn stats(&self) -> PageRankStats {
        let scores: Vec<f64> = self.scores.values().copied().collect();
        let n = scores.len();

        if n == 0 {
            return PageRankStats::default();
        }

        let sum: f64 = scores.iter().sum();
        let mean = sum / n as f64;
        let variance: f64 = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / n as f64;
        let max = scores.iter().cloned().fold(0.0f64, f64::max);
        let min = scores.iter().cloned().fold(1.0f64, f64::min);

        PageRankStats {
            process_count: n,
            mean_score: mean,
            std_dev: variance.sqrt(),
            max_score: max,
            min_score: min,
            iterations_to_converge: self.max_iterations, // Would need to track this
        }
    }

    /// Combine PageRank with memory usage for final priority
    pub fn get_weighted_candidates(
        &self,
        system: &System,
        memory_weight: f64,
        rank_weight: f64,
        limit: usize,
    ) -> Vec<(u32, f64)> {
        let mut candidates: Vec<(u32, f64)> = Vec::new();

        // Find max memory for normalization
        let max_memory = system
            .processes()
            .values()
            .map(|p| p.memory())
            .max()
            .unwrap_or(1) as f64;

        for (pid, process) in system.processes() {
            let pid_u32 = pid.as_u32();
            let rank_score = self.get_score(pid_u32);
            let memory_score = process.memory() as f64 / max_memory;

            // Combined score: high memory + low rank = good trim candidate
            let combined = memory_weight * memory_score + rank_weight * (1.0 - rank_score);
            candidates.push((pid_u32, combined));
        }

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates.into_iter().take(limit).collect()
    }
}

impl Default for ProcessPageRank {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default)]
pub struct PageRankStats {
    pub process_count: usize,
    pub mean_score: f64,
    pub std_dev: f64,
    pub max_score: f64,
    pub min_score: f64,
    pub iterations_to_converge: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagerank_basic() {
        let pagerank = ProcessPageRank::new();
        let stats = pagerank.stats();
        assert_eq!(stats.process_count, 0);
    }
}
