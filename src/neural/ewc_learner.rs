//! Elastic Weight Consolidation for preventing catastrophic forgetting
//!
//! Ensures the optimizer does not forget good strategies when learning new ones.

use crate::core::patterns::{MemoryPattern, LabeledPattern};

/// EWC Learner - prevents catastrophic forgetting
pub struct EWCLearner {
    /// Lambda parameter (higher = more preservation of old knowledge)
    lambda: f32,
    /// Fisher information diagonal (importance weights)
    fisher_diag: Vec<f32>,
    /// Optimal weights from previous tasks
    optimal_weights: Vec<f32>,
    /// Number of updates
    update_count: usize,
}

impl EWCLearner {
    pub fn new(lambda: f32) -> Self {
        Self {
            lambda,
            fisher_diag: vec![1.0; MemoryPattern::DIM],
            optimal_weights: vec![0.5; MemoryPattern::DIM],
            update_count: 0,
        }
    }
    
    /// Get confidence adjustment based on EWC
    pub fn get_confidence_adjustment(&self, pattern: &MemoryPattern) -> f32 {
        let features = pattern.to_vector();
        
        // Calculate how much current pattern aligns with learned optimal
        let alignment: f32 = features.iter()
            .zip(self.optimal_weights.iter())
            .zip(self.fisher_diag.iter())
            .map(|((f, w), fisher)| {
                let diff = (f - w).abs();
                fisher * (1.0 - diff)
            })
            .sum::<f32>() / MemoryPattern::DIM as f32;
        
        alignment.clamp(0.0, 1.0)
    }
    
    /// Update EWC from new labeled pattern
    pub fn update(&mut self, labeled: &LabeledPattern) {
        let features = labeled.pattern.to_vector();
        let success_weight = if labeled.success { 1.0 } else { 0.2 };
        
        // Update Fisher diagonal (importance)
        for (i, &f) in features.iter().enumerate() {
            if i < self.fisher_diag.len() {
                // Higher variance = more important
                let importance = (f - self.optimal_weights[i]).abs();
                self.fisher_diag[i] = self.fisher_diag[i] * 0.99 + importance * 0.01;
            }
        }
        
        // Update optimal weights with momentum
        let alpha = 0.1 * success_weight;
        for (i, &f) in features.iter().enumerate() {
            if i < self.optimal_weights.len() {
                self.optimal_weights[i] = self.optimal_weights[i] * (1.0 - alpha) + f * alpha;
            }
        }
        
        self.update_count += 1;
    }
    
    /// Calculate EWC penalty (for loss function)
    pub fn penalty(&self, current_weights: &[f32]) -> f32 {
        if current_weights.len() != self.optimal_weights.len() {
            return 0.0;
        }
        
        let penalty: f32 = current_weights.iter()
            .zip(self.optimal_weights.iter())
            .zip(self.fisher_diag.iter())
            .map(|((c, o), f)| f * (c - o).powi(2))
            .sum();
        
        0.5 * self.lambda * penalty
    }
    
    /// Get update count
    pub fn updates(&self) -> usize {
        self.update_count
    }
}
