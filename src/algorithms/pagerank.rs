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
    /// Reverse adjacency list (incoming edges) - for O(1) inlink lookup
    inlinks: HashMap<u32, Vec<u32>>,
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
            inlinks: HashMap::new(),
            damping: 0.85,
            epsilon: 1e-6,
            max_iterations: 100,
        }
    }

    /// Build the process graph and compute PageRank scores
    pub fn compute(&mut self, system: &System) {
        self.outlinks.clear();
        self.inlinks.clear();
        self.scores.clear();

        let processes: Vec<u32> = system.processes().keys().map(|p| p.as_u32()).collect();
        let n = processes.len();

        if n == 0 {
            return;
        }

        // Build outlink + inlink graphs (parent -> children)
        for (pid, process) in system.processes() {
            let pid_u32 = pid.as_u32();

            // Parent points to child (influence flows from parent)
            if let Some(parent_pid) = process.parent() {
                let parent_u32 = parent_pid.as_u32();
                self.outlinks
                    .entry(parent_u32)
                    .or_default()
                    .push(pid_u32);
                // Build reverse index: child -> list of parents pointing to it
                self.inlinks
                    .entry(pid_u32)
                    .or_default()
                    .push(parent_u32);
            }
        }

        // Pre-compute out-degrees for O(1) lookup
        let out_degrees: HashMap<u32, f64> = self.outlinks
            .iter()
            .map(|(&k, v)| (k, v.len() as f64))
            .collect();

        // Collect dangling nodes (no outlinks)
        let dangling_nodes: Vec<u32> = processes.iter()
            .filter(|p| !self.outlinks.contains_key(p))
            .copied()
            .collect();

        // Initialize scores uniformly
        let initial_score = 1.0 / n as f64;
        for &pid in &processes {
            self.scores.insert(pid, initial_score);
        }

        let n_f64 = n as f64;

        // Power iteration
        for iteration in 0..self.max_iterations {
            let mut new_scores: HashMap<u32, f64> = HashMap::with_capacity(n);
            let mut max_delta = 0.0f64;

            // Pre-compute dangling node contribution (sum of their scores / n)
            let dangling_sum: f64 = dangling_nodes.iter()
                .map(|p| self.scores.get(p).unwrap_or(&0.0))
                .sum();
            let dangling_contrib = self.damping * dangling_sum / n_f64;

            // Teleport component (random jump)
            let teleport = (1.0 - self.damping) / n_f64;
            let base_score = teleport + dangling_contrib;

            for &pid in &processes {
                let mut score = base_score;

                // O(in-degree) instead of O(m): only iterate inlinks to this node
                if let Some(sources) = self.inlinks.get(&pid) {
                    for &source in sources {
                        let source_score = self.scores.get(&source).unwrap_or(&0.0);
                        let out_deg = out_degrees.get(&source).unwrap_or(&1.0);
                        score += self.damping * source_score / out_deg;
                    }
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
