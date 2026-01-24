//! Neural Decision Engine

use std::path::Path;
use chrono::{Datelike, Timelike};
use tracing::{debug, info};

use crate::core::config::OptimizerConfig;
use crate::core::optimizer::OptimizationDecision;
use crate::core::patterns::{MemoryPattern, LabeledPattern};
use crate::windows::memory::{MemoryStatus, OptimizationResult};

use super::hnsw_patterns::PatternIndex;
use super::ewc_learner::EWCLearner;
use super::attention::AttentionScorer;

pub struct NeuralDecisionEngine {
    pattern_index: PatternIndex,
    attention: AttentionScorer,
    ewc: EWCLearner,
    config: OptimizerConfig,
    history: Vec<LabeledPattern>,
}

impl NeuralDecisionEngine {
    pub fn new(config: &OptimizerConfig) -> Result<Self, String> {
        let pattern_index = PatternIndex::new(MemoryPattern::DIM)
            .map_err(|e| format!("Failed to create pattern index: {}", e))?;
        let attention = AttentionScorer::new();
        let ewc = EWCLearner::new(config.ewc_lambda);
        let history = Self::load_history(&config.model_path).unwrap_or_default();
        info!("Neural engine initialized with {} historical patterns", history.len());
        Ok(Self { pattern_index, attention, ewc, config: config.clone(), history })
    }

    pub async fn decide(&self, pattern: &MemoryPattern, status: &MemoryStatus) -> Result<OptimizationDecision, String> {
        let pattern_vec = pattern.to_vector();
        let similar = self.pattern_index.search(&pattern_vec, 5)?;
        let (_, base_confidence) = self.analyze_similar_patterns(&similar);
        let attention_score = self.attention.score(pattern, status);
        let ewc_adjustment = self.ewc.get_confidence_adjustment(pattern);
        let final_confidence = (base_confidence * 0.5 + attention_score * 0.3 + ewc_adjustment * 0.2).clamp(0.0, 1.0);
        let should_optimize = status.memory_load_percent >= self.config.pressure_threshold || final_confidence > 0.7;
        let aggressive = status.memory_load_percent >= self.config.critical_threshold || (should_optimize && final_confidence > 0.9);
        let reason = format!("Neural: conf={:.2}, attn={:.2}, similar={}", final_confidence, attention_score, similar.len());
        debug!("{}", reason);
        Ok(OptimizationDecision { should_optimize, aggressive, confidence: final_confidence, reason, target_processes: vec![] })
    }

    fn analyze_similar_patterns(&self, similar: &[(usize, f32)]) -> (bool, f32) {
        if similar.is_empty() { return (false, 0.5); }
        let mut success_weight = 0.0f32;
        let mut total_weight = 0.0f32;
        for (idx, similarity) in similar {
            if let Some(labeled) = self.history.get(*idx) {
                if labeled.success { success_weight += similarity; }
                total_weight += similarity;
            }
        }
        if total_weight > 0.0 { (success_weight / total_weight > 0.5, success_weight / total_weight) } else { (false, 0.5) }
    }

    pub async fn learn_from_result(&mut self, decision: &OptimizationDecision, result: &OptimizationResult, success: bool) {
        let now = chrono::Local::now();
        let pattern = LabeledPattern {
            pattern: MemoryPattern {
                load: (result.before_available_mb / 100.0) as f32,
                consumption_rate: 0.0,
                available_ratio: (result.before_available_mb / 32000.0) as f32,
                page_file_ratio: 0.0,
                process_count: result.processes_trimmed as u32,
                hour: now.hour() as u8,
                day_of_week: now.weekday().num_days_from_monday() as u8,
                time_since_last_opt: 0.0,
            },
            optimized: true,
            aggressive: decision.aggressive,
            freed_mb: result.freed_mb as f32,
            success,
        };
        let vec = pattern.pattern.to_vector();
        let _ = self.pattern_index.add(&vec);
        self.ewc.update(&pattern);
        self.history.push(pattern);
        if self.history.len() % 100 == 0 { let _ = self.save_history(&self.config.model_path); }
        info!("Learned: success={}, freed={:.1}MB", success, result.freed_mb);
    }

    fn load_history(path: &Path) -> Result<Vec<LabeledPattern>, String> {
        let file_path = path.join("patterns.json");
        if !file_path.exists() { return Ok(vec![]); }
        let content = std::fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    fn save_history(&self, path: &Path) -> Result<(), String> {
        std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
        let content = serde_json::to_string_pretty(&self.history).map_err(|e| e.to_string())?;
        std::fs::write(path.join("patterns.json"), content).map_err(|e| e.to_string())
    }

    pub fn pattern_count(&self) -> usize { self.history.len() }
}
