//! Attention-based scoring for process prioritization
//! 
//! Uses multi-head attention to weight process importance.

use crate::core::patterns::MemoryPattern;
use crate::windows::memory::MemoryStatus;

/// Attention-based scorer for optimization decisions
pub struct AttentionScorer {
    /// Weights learned from experience
    weights: Vec<f32>,
    /// Temporal attention for time-of-day patterns
    temporal_weights: [f32; 24],
}

impl AttentionScorer {
    pub fn new() -> Self {
        // Initialize with sensible defaults
        Self {
            weights: vec![
                0.3,  // Memory load weight
                0.2,  // Consumption rate weight
                0.15, // Available ratio weight
                0.15, // Page file weight
                0.1,  // Process count weight
                0.05, // Hour weight
                0.05, // Day weight
            ],
            temporal_weights: Self::default_temporal_weights(),
        }
    }
    
    /// Default temporal weights (optimize more during peak hours)
    fn default_temporal_weights() -> [f32; 24] {
        let mut weights = [0.5f32; 24];
        // Higher weight during work hours (more aggressive optimization)
        for hour in 9..18 {
            weights[hour] = 0.8;
        }
        // Lower at night (system idle, less need)
        for hour in 0..6 {
            weights[hour] = 0.3;
        }
        weights
    }
    
    /// Calculate attention score for current state
    pub fn score(&self, pattern: &MemoryPattern, status: &MemoryStatus) -> f32 {
        let features = pattern.to_vector();
        
        // Weighted sum of features
        let base_score: f32 = features.iter()
            .zip(self.weights.iter())
            .map(|(f, w)| f * w)
            .sum();
        
        // Apply temporal attention
        let hour = pattern.hour as usize;
        let temporal = self.temporal_weights.get(hour).copied().unwrap_or(0.5);
        
        // Urgency boost based on memory pressure
        let urgency = if status.is_critical() {
            1.5  // Boost for critical
        } else if status.is_high_pressure() {
            1.2  // Boost for high pressure
        } else {
            1.0
        };
        
        (base_score * temporal * urgency).clamp(0.0, 1.0)
    }
    
    /// Update weights based on feedback
    pub fn update_weights(&mut self, feedback: f32, pattern: &MemoryPattern) {
        let features = pattern.to_vector();
        let learning_rate = 0.01;
        
        // Simple gradient update
        for (i, (w, f)) in self.weights.iter_mut().zip(features.iter()).enumerate() {
            *w += learning_rate * feedback * f;
            *w = w.clamp(0.0, 1.0);
        }
    }
}

impl Default for AttentionScorer {
    fn default() -> Self {
        Self::new()
    }
}
