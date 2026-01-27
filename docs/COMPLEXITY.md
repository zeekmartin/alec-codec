# ALEC Complexity

Temporal analysis and anomaly detection for IoT metrics.

## Overview

The Complexity module provides:
- **Baseline Learning**: Statistical summary of nominal operation
- **Delta/Z-Score Computation**: Deviation from baseline
- **S-lite Structure Analysis**: Pairwise sensor dependencies
- **Anomaly Event Detection**: Notifications with persistence/cooldown

## Standalone Usage

Complexity works independently of Gateway:

```rust
use alec_complexity::{ComplexityEngine, ComplexityConfig};
use alec_complexity::input::{GenericInput, InputAdapter};

let mut engine = ComplexityEngine::new(ComplexityConfig {
    enabled: true,
    ..Default::default()
});

// Feed data from any source
let input = GenericInput::new(timestamp_ms, 6.5)
    .with_tc(2.3)
    .with_h_joint(8.1)
    .with_r(0.45)
    .with_channel("temp", 3.2)
    .with_channel("humid", 2.8)
    .build();

if let Some(snapshot) = engine.process(&input) {
    println!("Events: {:?}", snapshot.events);
}
```

## With Gateway (Feature: `gateway`)

```rust
use alec_complexity::{ComplexityEngine, ComplexityConfig, MetricsSnapshotExt};
use alec_gateway::Gateway;
use alec_gateway::metrics::MetricsConfig;

let mut gateway = Gateway::new();
gateway.enable_metrics(MetricsConfig { enabled: true, ..Default::default() });

let mut complexity = ComplexityEngine::new(ComplexityConfig {
    enabled: true,
    ..Default::default()
});

// After flush...
if let Some(metrics) = gateway.last_metrics() {
    let input = metrics.to_complexity_input();
    if let Some(snapshot) = complexity.process(&input) {
        // Handle snapshot
    }
}
```

## Concepts

### Baseline

Statistical summary of "normal" operation:

| State | Description |
|-------|-------------|
| **Building** | Collecting samples, computing running stats |
| **Locked** | Baseline stable, anomalies meaningful |

The baseline tracks mean and standard deviation for:
- TC (Total Correlation)
- H_joint (Joint Entropy)
- H_bytes (Payload Entropy)
- R (Resilience Index, optional)

### Baseline Lifecycle

```
┌──────────────────────────────────────────────────────────────────┐
│                        BUILDING PHASE                             │
│  - Collecting samples                                             │
│  - Computing running mean/std                                     │
│  - Progress: 0% → 100%                                            │
│  - Duration: build_time_ms AND min_valid_snapshots must be met    │
└─────────────────────────────┬────────────────────────────────────┘
                              │ Both conditions met
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                         LOCKED PHASE                              │
│  - Baseline stable                                                │
│  - Deltas and Z-scores computed                                   │
│  - Anomaly detection active                                       │
│  - Update mode: Frozen | EMA | Rolling                            │
└──────────────────────────────────────────────────────────────────┘
```

### Z-Scores

Normalized deviation from baseline:

```
z = (current - baseline_mean) / baseline_std
```

| Z-Score | Interpretation |
|---------|----------------|
| |z| < 2 | Normal |
| 2 ≤ |z| < 3 | Warning |
| |z| ≥ 3 | Critical |

### S-lite (Structure Summary)

Lightweight pairwise dependency graph:
- Edges weighted by normalized entropy difference
- Top-K edges retained (sparsification)
- Detects structural breaks when edges change

```
S-lite edges:
  temp ←──0.72──→ humid    (high similarity)
  temp ←──0.31──→ pressure (moderate)
  humid ←─0.45──→ pressure
```

### Anomaly Events

| Event Type | Trigger |
|------------|---------|
| `BaselineBuilding` | Baseline not yet locked |
| `BaselineLocked` | Baseline ready |
| `PayloadEntropySpike` | z(H_bytes) exceeds threshold |
| `StructureBreak` | S-lite edges change abruptly |
| `RedundancyDrop` | z(R) drops below threshold |
| `ComplexitySurge` | z(TC) or z(H_joint) persists high |
| `SensorCriticalityShift` | Criticality ranking changes |

### Event Lifecycle

```
Condition detected
        │
        ▼
┌───────────────────┐
│ Persistence Timer │ ← Must persist for persistence_ms
└─────────┬─────────┘
          │ Timer expires
          ▼
┌───────────────────┐
│   Emit Event      │ ← Event fired
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│  Cooldown Timer   │ ← Same event type blocked for cooldown_ms
└───────────────────┘
```

## Configuration

### ComplexityConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | false | Master switch |
| `baseline` | `BaselineConfig` | ... | Baseline learning |
| `deltas` | `DeltaConfig` | ... | Delta computation |
| `structure` | `StructureConfig` | ... | S-lite settings |
| `anomaly` | `AnomalyConfig` | ... | Event detection |
| `output` | `OutputConfig` | ... | Output settings |

### BaselineConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `build_time_ms` | `u64` | 300000 | Build duration (5 min) |
| `min_valid_snapshots` | `u32` | 20 | Min samples to lock |
| `update_mode` | `UpdateMode` | Frozen | Post-lock behavior |
| `rolling_window_snapshots` | `u32` | 100 | Window for Rolling mode |

### BaselineUpdateMode

```rust
pub enum BaselineUpdateMode {
    /// Baseline is frozen after initial build (deterministic)
    Frozen,
    /// Baseline updates using exponential moving average
    Ema { alpha: u32 },  // alpha * 100 (e.g., 10 = 0.10)
    /// Baseline updates using rolling window
    Rolling,
}
```

### AnomalyConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | true | Enable detection |
| `z_threshold_warn` | `f64` | 2.0 | Warning threshold |
| `z_threshold_crit` | `f64` | 3.0 | Critical threshold |
| `persistence_ms` | `u64` | 30000 | Required duration (30s) |
| `cooldown_ms` | `u64` | 120000 | Between events (2 min) |
| `events` | `EventTypeConfig` | all enabled | Per-event toggles |

### StructureConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | true | Enable S-lite |
| `emit_s_lite` | `bool` | true | Include in snapshot |
| `max_channels` | `usize` | 32 | Max channels |
| `sparsify.enabled` | `bool` | true | Enable sparsification |
| `sparsify.top_k_edges` | `usize` | 64 | Keep top K edges |
| `sparsify.min_abs_weight` | `f64` | 0.2 | Min edge weight |
| `detect_breaks` | `bool` | true | Detect structure changes |
| `break_threshold` | `f64` | 0.3 | Min change for break |

## ComplexitySnapshot Schema

```json
{
  "version": "0.1.0",
  "timestamp_ms": 1706000000000,
  "baseline": {
    "state": "locked",
    "sample_count": 25,
    "progress": 1.0,
    "stats": {
      "tc_mean": 2.1,
      "tc_std": 0.3,
      "h_joint_mean": 5.2,
      "h_joint_std": 0.5,
      "h_bytes_mean": 6.2,
      "h_bytes_std": 0.4,
      "r_mean": 0.45,
      "r_std": 0.08
    }
  },
  "deltas": {
    "tc": 0.7,
    "h_joint": 0.8,
    "h_bytes": 0.9,
    "r": -0.1
  },
  "z_scores": {
    "tc": 2.33,
    "h_joint": 1.60,
    "h_bytes": 2.25,
    "r": -1.25
  },
  "s_lite": {
    "edges": [
      { "channel_a": "temp", "channel_b": "humid", "weight": 0.72 }
    ],
    "channel_count": 4,
    "timestamp_ms": 1706000000000
  },
  "events": [
    {
      "event_type": "ComplexitySurge",
      "severity": "Warning",
      "timestamp_ms": 1706000000000,
      "details": {
        "field": "tc",
        "z_score": 2.33,
        "threshold": 2.0
      }
    }
  ],
  "flags": ["ANOMALY_ENABLED"]
}
```

## Input Adapters

### GenericInput (JSON)

```json
{
  "timestamp_ms": 1706000000000,
  "h_bytes": 6.5,
  "tc": 2.3,
  "h_joint": 8.1,
  "r": 0.45,
  "channels": [
    { "id": "temp", "h": 3.2 },
    { "id": "humid", "h": 2.8 }
  ]
}
```

```rust
use alec_complexity::GenericInput;

let json = r#"{ "timestamp_ms": 1000, "h_bytes": 6.5 }"#;
let input = GenericInput::from_json(json)?;
let snapshot = input.to_snapshot();
```

### GatewayInput (Feature: `gateway`)

```rust
use alec_complexity::MetricsSnapshotExt;

// Automatically converts MetricsSnapshot to InputSnapshot
let input = metrics_snapshot.to_complexity_input();
```

### Custom Adapter

```rust
use alec_complexity::input::{InputSnapshot, InputAdapter, ChannelEntropy};

struct MyAdapter {
    timestamp: u64,
    entropy: f64,
}

impl InputAdapter for MyAdapter {
    fn to_input_snapshot(&self) -> InputSnapshot {
        InputSnapshot {
            timestamp_ms: self.timestamp,
            h_bytes: self.entropy,
            tc: None,
            h_joint: None,
            r: None,
            channel_entropies: vec![],
            source: "my_adapter".to_string(),
        }
    }
}
```

## Best Practices

1. **Set appropriate baseline duration**: 5 minutes is good for most IoT
2. **Use Frozen mode for testing**: Deterministic behavior
3. **Use EMA for production**: Adapts to slow drift
4. **Tune persistence_ms**: Avoid false positives from transients
5. **Monitor baseline progress**: Don't expect meaningful anomalies before lock
