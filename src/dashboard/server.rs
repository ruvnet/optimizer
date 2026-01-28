//! Simple HTTP server for dashboard data
//!
//! Serves JSON API for dashboard frontend

use std::sync::Arc;
use tokio::sync::RwLock;
use sysinfo::System;

use super::data::{
    DashboardCollector, DashboardData, SystemMetrics, ClusterInfo, SpectralState, SketchStats,
    ProcessInfo,
};
use crate::algorithms::{MinCutClusterer, ProcessPageRank, CountMinSketch, SpectralAnalyzer};
use crate::windows::memory::WindowsMemoryOptimizer;

/// Dashboard server state
pub struct DashboardServer {
    collector: Arc<RwLock<DashboardCollector>>,
    mincut: Arc<RwLock<MinCutClusterer>>,
    pagerank: Arc<RwLock<ProcessPageRank>>,
    sketch: Arc<RwLock<CountMinSketch>>,
    spectral: Arc<RwLock<SpectralAnalyzer>>,
    system: Arc<RwLock<System>>,
}

impl DashboardServer {
    pub fn new() -> Self {
        Self {
            collector: Arc::new(RwLock::new(DashboardCollector::new())),
            mincut: Arc::new(RwLock::new(MinCutClusterer::new())),
            pagerank: Arc::new(RwLock::new(ProcessPageRank::new())),
            sketch: Arc::new(RwLock::new(CountMinSketch::new(0.01, 0.001))),
            spectral: Arc::new(RwLock::new(SpectralAnalyzer::new(60))),
            system: Arc::new(RwLock::new(System::new_all())),
        }
    }

    /// Update all data and return dashboard state
    pub async fn update(&self) -> Result<DashboardData, String> {
        // Refresh system info
        let mut system = self.system.write().await;
        system.refresh_all();

        // Get memory status
        let status = WindowsMemoryOptimizer::get_memory_status()?;

        // Update spectral analyzer
        let spectral_state = {
            let mut spectral = self.spectral.write().await;
            spectral.add_sample(status.memory_load_percent as f64 / 100.0);
            let class = spectral.classify();
            let rec = spectral.get_recommendation();
            let stats = spectral.stats();

            SpectralState {
                pattern_class: format!("{:?}", class),
                trend: stats.trend,
                variance: stats.variance,
                recommendation: rec.reason,
                confidence: rec.confidence,
                predicted_relief_mb: rec.predicted_relief_mb,
            }
        };

        // Update sketch
        let sketch_stats = {
            let mut sketch = self.sketch.write().await;
            sketch.add(status.memory_load_percent as u64);
            let stats = sketch.stats();

            SketchStats {
                total_events: stats.total_count,
                memory_bytes: stats.memory_bytes,
                fill_ratio: stats.fill_ratio,
                peak_hours: vec![], // Would need pressure tracker for this
            }
        };

        // Update MinCut and PageRank
        let clusters = {
            let mut mincut = self.mincut.write().await;
            mincut.build_graph(&system);
            let raw_clusters = mincut.find_clusters(5);

            let mut pagerank = self.pagerank.write().await;
            pagerank.compute(&system);

            raw_clusters
                .into_iter()
                .map(|c| {
                    let top_procs: Vec<ProcessInfo> = c
                        .processes
                        .iter()
                        .take(5)
                        .filter_map(|&pid| {
                            system.process(sysinfo::Pid::from_u32(pid)).map(|p| ProcessInfo {
                                pid,
                                name: p.name().to_string_lossy().to_string(),
                                memory_mb: p.memory() as f64 / (1024.0 * 1024.0),
                                pagerank_score: pagerank.get_score(pid),
                                trim_priority: 0.5,
                            })
                        })
                        .collect();

                    ClusterInfo {
                        id: c.id,
                        process_count: c.processes.len(),
                        total_memory_mb: c.total_memory_mb,
                        connectivity: c.connectivity,
                        top_processes: top_procs,
                    }
                })
                .collect()
        };

        // Build metrics
        let metrics = SystemMetrics {
            timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
            memory_used_mb: status.used_physical_mb() as u64,
            memory_total_mb: status.total_physical_mb as u64,
            memory_percent: status.memory_load_percent as f64,
            swap_used_mb: (status.total_page_file_mb - status.available_page_file_mb) as u64,
            swap_total_mb: status.total_page_file_mb as u64,
            process_count: system.processes().len(),
            optimization_count: 0,
            total_freed_mb: 0.0,
        };

        // Record sample
        let mut collector = self.collector.write().await;
        collector.record_sample(metrics.memory_percent);

        Ok(collector.get_data(metrics, clusters, spectral_state, sketch_stats))
    }

    /// Record an optimization result
    pub async fn record_optimization(&self, freed_mb: f64) {
        let mut collector = self.collector.write().await;
        collector.record_optimization(freed_mb);
    }

    /// Get JSON data
    pub async fn get_json(&self) -> Result<String, String> {
        let data = self.update().await?;
        serde_json::to_string_pretty(&data).map_err(|e| e.to_string())
    }

    /// Run HTTP server (simplified - prints to console)
    pub async fn serve(&self, port: u16) -> Result<(), String> {
        println!("🌐 Dashboard server starting on http://localhost:{}", port);
        println!("   Press Ctrl+C to stop\n");

        // In a real implementation, this would use a web framework like axum or warp
        // For now, we'll just print updates periodically
        loop {
            match self.update().await {
                Ok(data) => {
                    println!("\n📊 Dashboard Update");
                    println!("   Memory: {:.1}% ({} MB / {} MB)",
                        data.metrics.memory_percent,
                        data.metrics.memory_used_mb,
                        data.metrics.memory_total_mb
                    );
                    println!("   Processes: {}", data.metrics.process_count);
                    println!("   Pattern: {} (confidence: {:.0}%)",
                        data.spectral.pattern_class,
                        data.spectral.confidence * 100.0
                    );
                    println!("   Clusters: {}", data.clusters.len());
                    println!("   Sketch events: {}", data.sketch.total_events);
                }
                Err(e) => {
                    eprintln!("   Error: {}", e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
}

impl Default for DashboardServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dashboard_server() {
        let server = DashboardServer::new();
        let result = server.update().await;
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(data.metrics.memory_total_mb > 0);
    }
}
