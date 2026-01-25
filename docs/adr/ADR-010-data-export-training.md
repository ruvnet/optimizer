# ADR-010: Data Export & Training Pipeline

## Status
Proposed

## Date
2025-01-25

## Context

RuVector collects rich data that could train external ML models:
- Memory usage time series
- Process behavior patterns
- Optimization outcomes
- User activity patterns
- System health metrics

This data has value for:
- Training custom anomaly detectors
- Building predictive models
- Research and analysis
- Integration with external tools
- Federated learning systems

## Decision

Implement **Data Export & Training Pipeline** for external model training.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                   Data Export & Training Pipeline                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────────────┐   │
│  │  Collector  │   │  Processor  │   │     Exporter        │   │
│  │             │──▶│             │──▶│                     │   │
│  │ • Memory    │   │ • Clean     │   │ • CSV               │   │
│  │ • Process   │   │ • Transform │   │ • JSON              │   │
│  │ • Events    │   │ • Anonymize │   │ • Parquet           │   │
│  │ • Outcomes  │   │ • Window    │   │ • SQLite            │   │
│  └─────────────┘   └─────────────┘   │ • Streaming         │   │
│                                       └─────────────────────┘   │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                   Training Integration                     │  │
│  │                                                            │  │
│  │   ┌────────┐   ┌────────┐   ┌────────┐   ┌────────┐      │  │
│  │   │Scikit  │   │PyTorch │   │ Keras  │   │ Custom │      │  │
│  │   │ Learn  │   │        │   │        │   │ Models │      │  │
│  │   └────────┘   └────────┘   └────────┘   └────────┘      │  │
│  │                                                            │  │
│  │   Streaming API: WebSocket / Named Pipe / ZeroMQ          │  │
│  │                                                            │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Data Schema

```rust
/// Core memory snapshot
#[derive(Serialize)]
pub struct MemorySnapshot {
    pub timestamp: DateTime<Utc>,
    pub total_mb: f64,
    pub available_mb: f64,
    pub used_percent: f32,
    pub page_file_used_mb: f64,
    pub processes_count: u32,
}

/// Process memory record
#[derive(Serialize)]
pub struct ProcessRecord {
    pub timestamp: DateTime<Utc>,
    pub pid: u32,
    pub name_hash: u64,          // Anonymized
    pub working_set_mb: f64,
    pub private_bytes_mb: f64,
    pub cpu_percent: f32,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
}

/// Optimization event
#[derive(Serialize)]
pub struct OptimizationEvent {
    pub timestamp: DateTime<Utc>,
    pub trigger: OptimizationTrigger,
    pub before_available_mb: f64,
    pub after_available_mb: f64,
    pub freed_mb: f64,
    pub processes_trimmed: u32,
    pub duration_ms: u64,
    pub success: bool,
}

/// Training sample (labeled)
#[derive(Serialize)]
pub struct TrainingSample {
    pub features: MemoryFeatures,
    pub label: OptimizationLabel,
}

pub struct MemoryFeatures {
    pub usage_percent: f32,
    pub usage_trend: f32,          // -1 to 1
    pub process_count: u32,
    pub top_process_percent: f32,
    pub page_file_ratio: f32,
    pub time_of_day: u8,
    pub day_of_week: u8,
    pub minutes_since_last_opt: u32,
}

pub enum OptimizationLabel {
    NoActionNeeded,
    ShouldOptimize,
    ShouldOptimizeAggressive,
    Critical,
}
```

### Export Formats

| Format | Use Case | Size | Speed |
|--------|----------|------|-------|
| CSV | Excel, basic analysis | Large | Fast |
| JSON | Web tools, APIs | Large | Medium |
| Parquet | Big data, ML pipelines | Small | Fast |
| SQLite | Local queries | Medium | Fast |
| Arrow | In-memory analytics | Small | Fastest |

### Export API

```rust
pub struct DataExporter {
    collector: DataCollector,
    config: ExportConfig,
}

impl DataExporter {
    /// Export time range to file
    pub fn export_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        format: ExportFormat,
        path: &Path,
    ) -> Result<ExportStats, Error>;

    /// Export last N hours
    pub fn export_recent(
        &self,
        hours: u32,
        format: ExportFormat,
        path: &Path,
    ) -> Result<ExportStats, Error>;

    /// Start streaming export
    pub fn start_stream(
        &mut self,
        format: StreamFormat,
        endpoint: &str,
    ) -> Result<StreamHandle, Error>;

    /// Export training dataset
    pub fn export_training_data(
        &self,
        config: TrainingDataConfig,
        path: &Path,
    ) -> Result<TrainingDataStats, Error>;
}

pub struct ExportConfig {
    pub anonymize: bool,
    pub include_processes: bool,
    pub include_events: bool,
    pub sample_interval_ms: u64,
    pub compression: Option<Compression>,
}

pub struct TrainingDataConfig {
    pub include_features: Vec<String>,
    pub label_type: LabelType,
    pub train_test_split: f32,
    pub balance_classes: bool,
}
```

### Streaming API

```rust
pub struct DataStream {
    socket: StreamSocket,
    format: StreamFormat,
}

impl DataStream {
    /// Real-time memory metrics
    pub async fn subscribe_memory(&mut self) -> impl Stream<Item = MemorySnapshot>;

    /// Process events
    pub async fn subscribe_processes(&mut self) -> impl Stream<Item = ProcessEvent>;

    /// Optimization events
    pub async fn subscribe_optimizations(&mut self) -> impl Stream<Item = OptimizationEvent>;

    /// All events combined
    pub async fn subscribe_all(&mut self) -> impl Stream<Item = DataEvent>;
}

pub enum StreamFormat {
    JsonLines,
    MessagePack,
    Protobuf,
}
```

### Training Pipeline

```python
# Example: Training custom optimization model

import pandas as pd
from ruvector import DataExporter

# Export training data
exporter = DataExporter()
exporter.export_training_data(
    start="2024-01-01",
    end="2024-01-31",
    format="parquet",
    path="training_data.parquet"
)

# Load and train
df = pd.read_parquet("training_data.parquet")
X = df[["usage_percent", "trend", "process_count", ...]]
y = df["label"]

from sklearn.ensemble import RandomForestClassifier
model = RandomForestClassifier()
model.fit(X, y)

# Export model for RuVector to use
import joblib
joblib.dump(model, "custom_model.pkl")
```

### Privacy & Anonymization

```rust
pub struct Anonymizer {
    salt: [u8; 32],
}

impl Anonymizer {
    /// Hash process name (irreversible)
    pub fn hash_process_name(&self, name: &str) -> u64;

    /// Remove file paths
    pub fn strip_paths(&self, data: &mut ProcessRecord);

    /// Generalize timestamps (round to hour)
    pub fn generalize_time(&self, timestamp: DateTime<Utc>) -> DateTime<Utc>;

    /// Remove low-count categories
    pub fn k_anonymize(&self, records: &mut Vec<ProcessRecord>, k: usize);
}
```

### CLI Commands

```bash
# Export last 24 hours to CSV
ruvector-memopt export --format csv --hours 24 --output data.csv

# Export training data
ruvector-memopt export --training --split 0.8 --output training/

# Start streaming server
ruvector-memopt export --stream --port 9999 --format jsonlines

# Export with anonymization
ruvector-memopt export --anonymize --format parquet --output anon_data.parquet
```

## Consequences

### Positive
- Enables custom model training
- Supports research and analysis
- Integration with external tools
- Foundation for federated learning
- User data ownership

### Negative
- Storage requirements for historical data
- Privacy concerns if misused
- Export overhead during operation
- Format maintenance burden

### Risks
- Sensitive data exposure if not anonymized
- Large exports may impact performance
- External models may not be compatible

## Implementation Phases

### Phase 1: Data Collection (1 week)
- Schema definition
- Rolling buffer storage
- Efficient sampling

### Phase 2: Export Formats (2 weeks)
- CSV/JSON export
- Parquet export
- SQLite export

### Phase 3: Streaming API (1 week)
- WebSocket server
- Named pipe support
- Stream multiplexing

### Phase 4: Training Pipeline (2 weeks)
- Feature engineering
- Label generation
- Train/test split
- Model integration

## Success Metrics

| Metric | Target |
|--------|--------|
| Export Speed | > 100k records/sec |
| Storage Efficiency | < 1 KB/minute |
| Streaming Latency | < 100ms |
| Privacy Compliance | 100% anonymized by default |

## References

- [Apache Parquet](https://parquet.apache.org/)
- [Arrow](https://arrow.apache.org/)
- [K-Anonymity](https://en.wikipedia.org/wiki/K-anonymity)
- Existing: `src/core/patterns.rs`, `src/neural/hnsw_patterns.rs`
