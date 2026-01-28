//! Simple text-based dashboard
//!
//! Currently Windows-only, requires the windows module.

#![cfg(target_os = "windows")]

use crate::windows::memory::MemoryStatus;
use crate::bench::metrics::MetricsSummary;

pub fn render_dashboard(status: &MemoryStatus, metrics: &MetricsSummary) -> String {
    let bar_width = 40;
    let filled = (status.memory_load_percent as usize * bar_width) / 100;
    let bar: String = format!(
        "[{}{}]",
        "#".repeat(filled),
        "-".repeat(bar_width - filled)
    );
    
    format!(
r#"
================ RuVector MemOpt Dashboard ================

Memory Usage: {} {}%
Total:     {:>8.0} MB
Available: {:>8.0} MB  
Used:      {:>8.0} MB

-------------------- Optimization Stats --------------------
Total Freed:    {:>8.1} MB
Optimizations:  {:>8}
Avg Freed:      {:>8.1} MB
Avg Duration:   {:>8} ms
Uptime:         {:>8} sec

Status: {}
============================================================
"#,
        bar,
        status.memory_load_percent,
        status.total_physical_mb,
        status.available_physical_mb,
        status.used_physical_mb(),
        metrics.total_freed_mb,
        metrics.total_optimizations,
        metrics.avg_freed_mb,
        metrics.avg_duration_ms,
        metrics.uptime_secs,
        if status.is_critical() { "CRITICAL" } 
        else if status.is_high_pressure() { "HIGH PRESSURE" } 
        else { "OK" }
    )
}
