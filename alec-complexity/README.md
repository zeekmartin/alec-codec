# alec-complexity

Complexity monitoring and anomaly detection for IoT systems.

[![Crates.io](https://img.shields.io/crates/v/alec-complexity.svg)](https://crates.io/crates/alec-complexity)
[![Documentation](https://docs.rs/alec-complexity/badge.svg)](https://docs.rs/alec-complexity)
[![License](https://img.shields.io/badge/license-AGPL--3.0%2FCommercial-blue.svg)](../LICENSE)

## Features

- **Baseline Learning**: Statistical summary of nominal operation
- **Delta/Z-Scores**: Deviation tracking from baseline
- **S-lite Structure**: Sensor dependency analysis
- **Anomaly Events**: Automatic detection with persistence/cooldown

## Standalone Usage

```rust
use alec_complexity::{ComplexityEngine, ComplexityConfig};
use alec_complexity::input::{GenericInput, InputAdapter};

let mut engine = ComplexityEngine::new(ComplexityConfig {
    enabled: true,
    ..Default::default()
});

// Feed data from any source
let input = GenericInput::new(timestamp_ms, 6.5)  // h_bytes
    .with_tc(2.3)
    .with_h_joint(8.1)
    .with_r(0.45)
    .with_channel("temp", 3.2)
    .with_channel("humid", 2.8)
    .build();

if let Some(snapshot) = engine.process(&input) {
    // Check baseline state
    if snapshot.is_baseline_locked() {
        println!("Deltas: {:?}", snapshot.deltas);
        println!("Z-scores: {:?}", snapshot.z_scores);
    }

    // Handle events
    for event in &snapshot.events {
        println!("Event: {:?}", event);
    }
}
```

## With ALEC Gateway

Enable the `gateway` feature to consume `MetricsSnapshot` directly:

```toml
[dependencies]
alec-complexity = { version = "0.1", features = ["gateway"] }
```

```rust
use alec_complexity::{ComplexityEngine, ComplexityConfig, MetricsSnapshotExt};

// Get MetricsSnapshot from Gateway
if let Some(metrics) = gateway.last_metrics() {
    let input = metrics.to_complexity_input();
    if let Some(snapshot) = engine.process(&input) {
        // Handle snapshot
    }
}
```

## JSON Input

```rust
let json = r#"{
    "timestamp_ms": 1706000000000,
    "h_bytes": 6.5,
    "tc": 2.3,
    "h_joint": 8.1,
    "r": 0.45
}"#;

let input = GenericInput::from_json(json)?;
let snapshot = engine.process(&input.to_snapshot());
```

## Configuration

### Quick Start

```rust
let config = ComplexityConfig {
    enabled: true,
    baseline: BaselineConfig {
        build_time_ms: 60_000,    // 1 minute
        min_valid_snapshots: 10,
        update_mode: BaselineUpdateMode::Frozen,
        ..Default::default()
    },
    anomaly: AnomalyConfig {
        enabled: true,
        z_threshold_warn: 2.0,
        z_threshold_crit: 3.0,
        persistence_ms: 5000,
        cooldown_ms: 30000,
        ..Default::default()
    },
    ..Default::default()
};
```

### Key Settings

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | false | Master switch |
| `baseline.build_time_ms` | 300000 | Baseline build duration (5 min) |
| `baseline.min_valid_snapshots` | 20 | Min samples to lock |
| `anomaly.z_threshold_warn` | 2.0 | Warning threshold |
| `anomaly.z_threshold_crit` | 3.0 | Critical threshold |
| `anomaly.persistence_ms` | 30000 | Persistence requirement |
| `anomaly.cooldown_ms` | 120000 | Cooldown between events |

## Event Types

| Event | Trigger |
|-------|---------|
| `BaselineBuilding` | Baseline not yet locked |
| `BaselineLocked` | Baseline ready |
| `PayloadEntropySpike` | H_bytes z-score exceeds threshold |
| `StructureBreak` | S-lite edges change abruptly |
| `RedundancyDrop` | R z-score drops below threshold |
| `ComplexitySurge` | TC/H_joint z-score persists high |
| `SensorCriticalityShift` | Criticality ranking changes |

## Output Schema

```json
{
  "version": "0.1.0",
  "timestamp_ms": 1706000000000,
  "baseline": {
    "state": "locked",
    "progress": 1.0,
    "stats": { "h_bytes_mean": 6.2, "h_bytes_std": 0.4 }
  },
  "deltas": { "h_bytes": 0.9, "tc": 0.7 },
  "z_scores": { "h_bytes": 2.25, "tc": 2.33 },
  "events": [
    { "event_type": "ComplexitySurge", "severity": "Warning" }
  ]
}
```

## Documentation

- [Complexity Guide](../docs/COMPLEXITY.md)
- [Configuration Reference](../docs/CONFIGURATION.md)
- [Integration Guide](../docs/INTEGRATION.md)
- [JSON Schemas](../docs/JSON_SCHEMAS.md)
- [FAQ](../docs/FAQ.md)

## License

ALEC Complexity is dual-licensed:

- **AGPL-3.0**: Free for open source projects, research, and personal use
- **Commercial License**: For proprietary use without open-source obligations

See [LICENSE](../LICENSE) for details.
