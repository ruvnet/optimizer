//! Memory pattern representation for neural learning

use chrono::{Datelike, Timelike};
use crate::windows::memory::MemoryStatus;
use serde::{Deserialize, Serialize};

/// Memory pattern vector for HNSW indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPattern {
    /// Normalized memory load (0.0-1.0)
    pub load: f32,
    /// Rate of memory consumption (MB/sec)
    pub consumption_rate: f32,
    /// Available physical memory ratio
    pub available_ratio: f32,
    /// Page file usage ratio
    pub page_file_ratio: f32,
    /// Number of processes
    pub process_count: u32,
    /// Hour of day (0-23) for temporal patterns
    pub hour: u8,
    /// Day of week (0-6)
    pub day_of_week: u8,
    /// Time since last optimization (seconds)
    pub time_since_last_opt: f32,
}

impl MemoryPattern {
    /// Create pattern from current memory status
    pub fn from_status(status: &MemoryStatus) -> Self {
        let now = chrono::Local::now();
        
        Self {
            load: status.memory_load_percent as f32 / 100.0,
            consumption_rate: 0.0, // Would need historical data
            available_ratio: (status.available_physical_mb / status.total_physical_mb) as f32,
            page_file_ratio: 1.0 - (status.available_page_file_mb / status.total_page_file_mb) as f32,
            process_count: 0, // Would need process enumeration
            hour: now.hour() as u8,
            day_of_week: now.weekday().num_days_from_monday() as u8,
            time_since_last_opt: 0.0,
        }
    }
    
    /// Convert to vector for HNSW indexing
    pub fn to_vector(&self) -> Vec<f32> {
        vec![
            self.load,
            self.consumption_rate,
            self.available_ratio,
            self.page_file_ratio,
            self.process_count as f32 / 1000.0, // Normalize
            self.hour as f32 / 24.0,
            self.day_of_week as f32 / 7.0,
            self.time_since_last_opt / 3600.0, // Normalize to hours
        ]
    }
    
    /// Vector dimension
    pub const DIM: usize = 8;
}

/// Labeled pattern for learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabeledPattern {
    pub pattern: MemoryPattern,
    /// Whether optimization was triggered
    pub optimized: bool,
    /// Whether it was aggressive
    pub aggressive: bool,
    /// Memory freed (MB)
    pub freed_mb: f32,
    /// Whether the optimization was considered successful
    pub success: bool,
}
