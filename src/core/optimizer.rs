//! Intelligent Memory Optimizer
//!
//! Combines Windows memory APIs with RuVector neural decision making.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::config::OptimizerConfig;
use super::patterns::MemoryPattern;
use super::process_scorer::ProcessScorer;
use crate::neural::engine::NeuralDecisionEngine;
use crate::windows::memory::{MemoryStatus, OptimizationResult, WindowsMemoryOptimizer};
use crate::bench::metrics::{BenchmarkMetrics, OptimizationMetrics};

/// Decision from neural engine
#[derive(Debug, Clone)]
pub struct OptimizationDecision {
    pub should_optimize: bool,
    pub aggressive: bool,
    pub confidence: f32,
    pub reason: String,
    pub target_processes: Vec<u32>,
}

/// Intelligent memory optimizer with neural decision making
pub struct IntelligentOptimizer {
    config: OptimizerConfig,
    windows_opt: WindowsMemoryOptimizer,
    neural_engine: Option<Arc<RwLock<NeuralDecisionEngine>>>,
    process_scorer: ProcessScorer,
    last_optimization: Option<Instant>,
    metrics: BenchmarkMetrics,
}

impl IntelligentOptimizer {
    /// Create a new intelligent optimizer
    pub fn new(config: OptimizerConfig) -> Self {
        let windows_opt = WindowsMemoryOptimizer::new();
        
        let neural_engine = if config.neural_enabled {
            match NeuralDecisionEngine::new(&config) {
                Ok(engine) => Some(Arc::new(RwLock::new(engine))),
                Err(e) => {
                    warn!("Failed to initialize neural engine: {}. Using rule-based fallback.", e);
                    None
                }
            }
        } else {
            None
        };
        
        Self {
            config,
            windows_opt,
            neural_engine,
            process_scorer: ProcessScorer::new(),
            last_optimization: None,
            metrics: BenchmarkMetrics::new(),
        }
    }
    
    /// Check current memory pressure and make optimization decision
    pub async fn evaluate(&self) -> Result<OptimizationDecision, String> {
        let status = WindowsMemoryOptimizer::get_memory_status()?;
        let pattern = MemoryPattern::from_status(&status);
        
        // Check minimum interval
        if let Some(last) = self.last_optimization {
            let elapsed = last.elapsed().as_secs();
            if elapsed < self.config.min_interval_secs {
                return Ok(OptimizationDecision {
                    should_optimize: false,
                    aggressive: false,
                    confidence: 1.0,
                    reason: format!("Cooldown: {}s remaining", 
                        self.config.min_interval_secs - elapsed),
                    target_processes: vec![],
                });
            }
        }
        
        // Neural decision if available
        if let Some(ref engine) = self.neural_engine {
            let engine = engine.read().await;
            return engine.decide(&pattern, &status).await;
        }
        
        // Fallback to rule-based decision
        self.rule_based_decision(&status)
    }
    
    /// Rule-based fallback decision making
    fn rule_based_decision(&self, status: &MemoryStatus) -> Result<OptimizationDecision, String> {
        let load = status.memory_load_percent;
        
        if load >= self.config.critical_threshold {
            Ok(OptimizationDecision {
                should_optimize: true,
                aggressive: true,
                confidence: 0.95,
                reason: format!("Critical memory pressure: {}%", load),
                target_processes: vec![], // Target all
            })
        } else if load >= self.config.pressure_threshold {
            // Score processes to find best trim candidates
            let targets = self.process_scorer.get_trim_candidates(10);
            
            Ok(OptimizationDecision {
                should_optimize: true,
                aggressive: false,
                confidence: 0.8,
                reason: format!("High memory pressure: {}%", load),
                target_processes: targets,
            })
        } else {
            Ok(OptimizationDecision {
                should_optimize: false,
                aggressive: false,
                confidence: 0.9,
                reason: format!("Memory OK: {}%", load),
                target_processes: vec![],
            })
        }
    }
    
    /// Execute memory optimization based on decision
    pub async fn optimize(&mut self, decision: &OptimizationDecision) -> Result<OptimizationResult, String> {
        if !decision.should_optimize {
            return Err("Optimization not recommended".into());
        }
        
        let start = Instant::now();
        info!("Starting optimization (aggressive={}): {}", 
            decision.aggressive, decision.reason);
        
        // Execute Windows optimization
        let result = self.windows_opt.optimize(decision.aggressive)?;
        
        // Record metrics
        let opt_metrics = OptimizationMetrics {
            freed_mb: result.freed_mb,
            processes_trimmed: result.processes_trimmed,
            duration_ms: result.duration_ms,
            aggressive: decision.aggressive,
            confidence: decision.confidence,
        };
        self.metrics.record_optimization(&opt_metrics);
        
        // Learn from result if enabled
        if self.config.learning_enabled {
            if let Some(ref engine) = self.neural_engine {
                let mut engine = engine.write().await;
                let success = result.freed_mb > 100.0; // Consider >100MB freed as success
                engine.learn_from_result(&decision, &result, success).await;
            }
        }
        
        self.last_optimization = Some(Instant::now());
        
        info!("Optimization complete: freed {:.1} MB in {}ms", 
            result.freed_mb, start.elapsed().as_millis());
        
        Ok(result)
    }
    
    /// Run startup optimization mode (aggressive, one-shot)
    pub async fn startup_optimize(&mut self) -> Result<OptimizationResult, String> {
        info!("Running startup optimization mode");
        
        // Wait for system to stabilize
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Force aggressive optimization on startup
        let decision = OptimizationDecision {
            should_optimize: true,
            aggressive: self.windows_opt.has_admin_privileges(),
            confidence: 1.0,
            reason: "Startup optimization".into(),
            target_processes: vec![],
        };
        
        self.optimize(&decision).await
    }
    
    /// Main optimization loop
    pub async fn run_loop(&mut self, interval: Duration) -> ! {
        info!("Starting optimization loop (interval: {:?})", interval);
        
        loop {
            match self.evaluate().await {
                Ok(decision) => {
                    if decision.should_optimize {
                        if let Err(e) = self.optimize(&decision).await {
                            error!("Optimization failed: {}", e);
                        }
                    } else {
                        debug!("Skipping: {}", decision.reason);
                    }
                }
                Err(e) => {
                    error!("Evaluation failed: {}", e);
                }
            }
            
            tokio::time::sleep(interval).await;
        }
    }
    
    /// Get current metrics
    pub fn get_metrics(&self) -> &BenchmarkMetrics {
        &self.metrics
    }
    
    /// Check if neural engine is active
    pub fn has_neural_engine(&self) -> bool {
        self.neural_engine.is_some()
    }
}
