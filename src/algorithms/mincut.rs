//! MinCut algorithm for process clustering
//!
//! Groups processes that share memory pages or have parent-child relationships.
//! Optimizing related processes together improves cache coherence.

use std::collections::{HashMap, HashSet, VecDeque};
use sysinfo::{System, ProcessesToUpdate};

/// Edge in the process graph
#[derive(Debug, Clone)]
struct Edge {
    from: u32,
    to: u32,
    weight: f64,
}

/// Process cluster identified by MinCut
#[derive(Debug, Clone)]
pub struct ProcessCluster {
    pub id: usize,
    pub processes: Vec<u32>,
    pub total_memory_mb: f64,
    pub connectivity: f64,
}

/// MinCut-based process clustering
pub struct MinCutClusterer {
    adjacency: HashMap<u32, Vec<(u32, f64)>>,
    process_memory: HashMap<u32, u64>,
    min_cluster_size: usize,
}

impl MinCutClusterer {
    pub fn new() -> Self {
        Self {
            adjacency: HashMap::with_capacity(512),
            process_memory: HashMap::with_capacity(512),
            min_cluster_size: 2,
        }
    }

    /// Build process graph from system state
    pub fn build_graph(&mut self, system: &System) {
        self.adjacency.clear();
        self.process_memory.clear();

        // Phase 1: Collect process info and build name -> PIDs index (O(n))
        let mut name_groups: HashMap<String, Vec<u32>> = HashMap::new();

        for (pid, process) in system.processes() {
            let pid_u32 = pid.as_u32();
            self.process_memory.insert(pid_u32, process.memory());

            // Parent-child edge (strong connection)
            if let Some(parent_pid) = process.parent() {
                self.add_edge(pid_u32, parent_pid.as_u32(), 1.0);
            }

            // Index by name for O(n) same-name grouping
            let name = process.name().to_string_lossy().to_lowercase();
            name_groups.entry(name).or_default().push(pid_u32);
        }

        // Phase 2: Connect same-name processes (O(n) total across all groups)
        for (_name, pids) in &name_groups {
            if pids.len() < 2 { continue; }
            // Connect each to the first (star topology) instead of all-pairs O(k^2)
            let hub = pids[0];
            for &pid in &pids[1..] {
                self.add_edge(hub, pid, 0.5);
            }
        }
    }

    fn add_edge(&mut self, from: u32, to: u32, weight: f64) {
        self.adjacency
            .entry(from)
            .or_insert_with(Vec::new)
            .push((to, weight));
        self.adjacency
            .entry(to)
            .or_insert_with(Vec::new)
            .push((from, weight));
    }

    /// Find clusters using Karger's MinCut approximation
    /// Returns groups of related processes
    pub fn find_clusters(&self, target_clusters: usize) -> Vec<ProcessCluster> {
        if self.adjacency.is_empty() {
            return vec![];
        }

        // Use BFS-based clustering (faster than Karger's for our use case)
        let mut visited: HashSet<u32> = HashSet::new();
        let mut clusters = Vec::new();
        let mut cluster_id = 0;

        for &start_pid in self.adjacency.keys() {
            if visited.contains(&start_pid) {
                continue;
            }

            let cluster = self.bfs_cluster(start_pid, &mut visited);
            if cluster.len() >= self.min_cluster_size {
                let total_memory: u64 = cluster
                    .iter()
                    .filter_map(|pid| self.process_memory.get(pid))
                    .sum();

                let connectivity = self.calculate_connectivity(&cluster);

                clusters.push(ProcessCluster {
                    id: cluster_id,
                    processes: cluster,
                    total_memory_mb: total_memory as f64 / (1024.0 * 1024.0),
                    connectivity,
                });
                cluster_id += 1;
            }

            if clusters.len() >= target_clusters {
                break;
            }
        }

        // Sort by total memory (optimize largest clusters first)
        clusters.sort_by(|a, b| {
            b.total_memory_mb
                .partial_cmp(&a.total_memory_mb)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        clusters
    }

    /// BFS to find connected component
    fn bfs_cluster(&self, start: u32, visited: &mut HashSet<u32>) -> Vec<u32> {
        let mut cluster = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(start);

        while let Some(pid) = queue.pop_front() {
            if visited.contains(&pid) {
                continue;
            }
            visited.insert(pid);
            cluster.push(pid);

            if let Some(neighbors) = self.adjacency.get(&pid) {
                for &(neighbor, weight) in neighbors {
                    if !visited.contains(&neighbor) && weight > 0.3 {
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        cluster
    }

    /// Calculate internal connectivity of a cluster
    fn calculate_connectivity(&self, cluster: &[u32]) -> f64 {
        if cluster.len() < 2 {
            return 0.0;
        }

        let cluster_set: HashSet<u32> = cluster.iter().copied().collect();
        let mut internal_edges = 0.0;
        let mut total_edges = 0.0;

        for &pid in cluster {
            if let Some(neighbors) = self.adjacency.get(&pid) {
                for &(neighbor, weight) in neighbors {
                    total_edges += weight;
                    if cluster_set.contains(&neighbor) {
                        internal_edges += weight;
                    }
                }
            }
        }

        if total_edges > 0.0 {
            internal_edges / total_edges
        } else {
            0.0
        }
    }

    /// Get optimal trim order within a cluster
    pub fn get_trim_order(&self, cluster: &ProcessCluster) -> Vec<u32> {
        // Sort by memory usage (trim largest first within cluster)
        let mut ordered = cluster.processes.clone();
        ordered.sort_by(|a, b| {
            let mem_a = self.process_memory.get(a).unwrap_or(&0);
            let mem_b = self.process_memory.get(b).unwrap_or(&0);
            mem_b.cmp(mem_a)
        });
        ordered
    }

    /// Statistics for benchmarking
    pub fn stats(&self) -> MinCutStats {
        MinCutStats {
            total_processes: self.process_memory.len(),
            total_edges: self.adjacency.values().map(|v| v.len()).sum::<usize>() / 2,
            total_memory_mb: self.process_memory.values().sum::<u64>() as f64 / (1024.0 * 1024.0),
        }
    }
}

impl Default for MinCutClusterer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct MinCutStats {
    pub total_processes: usize,
    pub total_edges: usize,
    pub total_memory_mb: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mincut_clusterer() {
        let mut clusterer = MinCutClusterer::new();

        // Add some test edges manually
        clusterer.add_edge(1, 2, 1.0);
        clusterer.add_edge(2, 3, 1.0);
        clusterer.add_edge(4, 5, 1.0);

        clusterer.process_memory.insert(1, 100 * 1024 * 1024);
        clusterer.process_memory.insert(2, 200 * 1024 * 1024);
        clusterer.process_memory.insert(3, 150 * 1024 * 1024);
        clusterer.process_memory.insert(4, 50 * 1024 * 1024);
        clusterer.process_memory.insert(5, 75 * 1024 * 1024);

        let clusters = clusterer.find_clusters(10);
        assert!(!clusters.is_empty());

        let stats = clusterer.stats();
        assert_eq!(stats.total_processes, 5);
    }
}
