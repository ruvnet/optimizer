//! Real-time memory monitoring
//!
//! Currently Windows-only, requires the windows module.

#![cfg(target_os = "windows")]

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use crate::windows::memory::{MemoryStatus, WindowsMemoryOptimizer};

pub struct RealtimeMonitor {
    interval: Duration,
    history: Arc<RwLock<Vec<MemorySnapshot>>>,
    max_history: usize,
}

#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    pub timestamp: chrono::DateTime<chrono::Local>,
    pub status: MemoryStatus,
}

impl RealtimeMonitor {
    pub fn new(interval_secs: u64) -> Self {
        Self {
            interval: Duration::from_secs(interval_secs),
            history: Arc::new(RwLock::new(Vec::new())),
            max_history: 3600, // 1 hour at 1s interval
        }
    }
    
    pub async fn start(&self) {
        loop {
            if let Ok(status) = WindowsMemoryOptimizer::get_memory_status() {
                let snapshot = MemorySnapshot {
                    timestamp: chrono::Local::now(),
                    status,
                };
                
                let mut history = self.history.write().await;
                if history.len() >= self.max_history {
                    history.remove(0);
                }
                history.push(snapshot);
            }
            
            tokio::time::sleep(self.interval).await;
        }
    }
    
    pub async fn get_current(&self) -> Option<MemorySnapshot> {
        let history = self.history.read().await;
        history.last().cloned()
    }
    
    pub async fn get_history(&self, count: usize) -> Vec<MemorySnapshot> {
        let history = self.history.read().await;
        history.iter().rev().take(count).cloned().collect()
    }
    
    pub async fn get_stats(&self) -> MonitorStats {
        let history = self.history.read().await;
        
        if history.is_empty() {
            return MonitorStats::default();
        }
        
        let loads: Vec<u32> = history.iter().map(|s| s.status.memory_load_percent).collect();
        let avg_load = loads.iter().sum::<u32>() as f64 / loads.len() as f64;
        let max_load = *loads.iter().max().unwrap_or(&0);
        let min_load = *loads.iter().min().unwrap_or(&0);
        
        MonitorStats {
            sample_count: history.len(),
            avg_memory_load: avg_load,
            max_memory_load: max_load,
            min_memory_load: min_load,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MonitorStats {
    pub sample_count: usize,
    pub avg_memory_load: f64,
    pub max_memory_load: u32,
    pub min_memory_load: u32,
}
