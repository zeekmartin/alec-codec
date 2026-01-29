# ALEC Configuration Reference

Complete reference for all configuration options.

## Gateway Configuration

### GatewayConfig

```rust
GatewayConfig {
    max_frame_size: 242,      // bytes (LoRaWAN DR4)
    max_channels: 32,         // maximum channels
    enable_checksums: true,   // enable checksums globally
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_frame_size` | `usize` | 242 | Maximum frame size in bytes |
| `max_channels` | `usize` | 32 | Maximum number of channels |
| `enable_checksums` | `bool` | true | Enable checksums on all channels |

### ChannelConfig

```rust
ChannelConfig {
    buffer_size: 64,          // pending values buffer
    preload_path: None,       // optional context preload
    priority: 128,            // 0 = highest priority
    enable_checksum: true,    // per-channel checksum
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `buffer_size` | `usize` | 64 | Buffer size for pending values |
| `preload_path` | `Option<String>` | None | Path to preload file |
| `priority` | `u8` | 128 | Priority (0 = highest, 255 = lowest) |
| `enable_checksum` | `bool` | true | Enable checksum for this channel |

## Metrics Configuration

### MetricsConfig

```rust
MetricsConfig {
    enabled: false,                            // master switch
    signal_compute: SignalComputeSchedule::NFlushesOrMillis {
        n_flushes: 10,
        millis: 10_000,
    },
    signal_window: SignalWindow::TimeMillis(60_000),
    alignment: AlignmentStrategy::SampleAndHold,
    missing_data: MissingDataPolicy::DropIncompleteSnapshots,
    normalization: NormalizationConfig::default(),
    signal_estimator: SignalEstimator::GaussianCovariance {
        log_base: LogBase::Two,
    },
    payload: PayloadMetricsConfig::default(),
    resilience: ResilienceConfig::default(),
    numerics: NumericsConfig::default(),
}
```

### SignalComputeSchedule

```rust
pub enum SignalComputeSchedule {
    EveryNFlushes(u32),                      // every N flushes
    EveryMillis(u64),                        // every T ms
    NFlushesOrMillis { n_flushes: u32, millis: u64 }, // whichever first
}
```

### SignalWindow

```rust
pub enum SignalWindow {
    TimeMillis(u64),     // default: 60_000 (60 seconds)
    LastNSamples(usize), // last N samples per channel
}
```

### AlignmentStrategy

```rust
pub enum AlignmentStrategy {
    SampleAndHold,        // default: ZOH alignment
    Nearest,              // nearest sample to reference
    LinearInterpolation,  // linear interpolation
}
```

### MissingDataPolicy

```rust
pub enum MissingDataPolicy {
    DropIncompleteSnapshots,              // default: skip incomplete
    AllowPartial { min_channels: usize }, // allow if >= min channels
    FillWithLastKnown,                    // forward-fill
}
```

### NormalizationConfig

```rust
NormalizationConfig {
    enabled: true,
    method: NormalizationMethod::ZScore,
    min_samples: 10,
}
```

### PayloadMetricsConfig

```rust
PayloadMetricsConfig {
    frame_entropy: true,       // H_bytes of frame
    per_channel_entropy: false, // H_bytes per channel
    sizes: true,               // frame/channel sizes
    include_histogram: false,   // 256-bin histogram
}
```

### ResilienceConfig

```rust
ResilienceConfig {
    enabled: false,            // separate opt-in
    criticality: CriticalityConfig {
        enabled: true,
        max_channels: 16,
        every_n_signal_computes: 1,
    },
    thresholds: ResilienceThresholds {
        healthy_min: 0.5,
        attention_min: 0.2,
    },
    min_sum_h: 0.1,
}
```

### NumericsConfig

```rust
NumericsConfig {
    min_aligned_samples: 32,    // min samples for metrics
    covariance_epsilon: 1e-8,   // regularization
    max_channels_for_joint: 32, // max channels for H_joint
}
```

## Complexity Configuration

### ComplexityConfig

```rust
ComplexityConfig {
    enabled: false,                        // master switch
    baseline: BaselineConfig::default(),
    deltas: DeltaConfig::default(),
    structure: StructureConfig::default(),
    anomaly: AnomalyConfig::default(),
    output: OutputConfig::default(),
}
```

### BaselineConfig

```rust
BaselineConfig {
    build_time_ms: 300_000,               // 5 minutes
    min_valid_snapshots: 20,              // min samples
    update_mode: BaselineUpdateMode::Frozen,
    rolling_window_snapshots: 100,        // for Rolling mode
}
```

### BaselineUpdateMode

```rust
pub enum BaselineUpdateMode {
    Frozen,                 // default: no updates after lock
    Ema { alpha: u32 },     // EMA: alpha * 100 (e.g., 10 = 0.10)
    Rolling,                // rolling window
}
```

### DeltaConfig

```rust
DeltaConfig {
    compute_tc: true,
    compute_r: true,
    compute_h_joint: true,
    compute_payload_entropy: true,
    smoothing: SmoothingConfig {
        enabled: true,
        alpha: 0.2,
    },
}
```

### StructureConfig

```rust
StructureConfig {
    enabled: true,
    emit_s_lite: true,
    max_channels: 32,
    sparsify: SparsifyConfig {
        enabled: true,
        top_k_edges: 64,
        min_abs_weight: 0.2,
    },
    detect_breaks: true,
    break_threshold: 0.3,
}
```

### AnomalyConfig

```rust
AnomalyConfig {
    enabled: true,
    z_threshold_warn: 2.0,
    z_threshold_crit: 3.0,
    persistence_ms: 30_000,    // 30 seconds
    cooldown_ms: 120_000,      // 2 minutes
    events: EventTypeConfig {
        baseline_events: true,
        payload_entropy_spike: true,
        structure_break: true,
        redundancy_drop: true,
        complexity_surge: true,
        criticality_shift: true,
    },
}
```

### OutputConfig

```rust
OutputConfig {
    snapshot_every_n_ticks: 1,
    emit_events: true,
    include_baseline_stats: true,
}
```

## Configuration Examples

### Minimal Gateway

```rust
let gateway = Gateway::new(); // All defaults
```

### LoRaWAN DR0 (Constrained)

```rust
let config = GatewayConfig::lorawan(0); // 51 bytes max
let mut gateway = Gateway::with_config(config);
```

### Full Observability Stack

```rust
use alec_gateway::{Gateway, GatewayConfig};
use alec_gateway::metrics::{MetricsConfig, ResilienceConfig};
use alec_complexity::{ComplexityEngine, ComplexityConfig};

// Gateway with metrics
let mut gateway = Gateway::with_config(GatewayConfig::default());
gateway.enable_metrics(MetricsConfig {
    enabled: true,
    resilience: ResilienceConfig {
        enabled: true,
        ..Default::default()
    },
    ..Default::default()
});

// Complexity engine
let complexity = ComplexityEngine::new(ComplexityConfig {
    enabled: true,
    baseline: BaselineConfig {
        build_time_ms: 60_000,  // 1 minute for faster startup
        min_valid_snapshots: 10,
        ..Default::default()
    },
    ..Default::default()
});
```

### Production Configuration (JSON)

```json
{
  "gateway": {
    "max_frame_size": 242,
    "max_channels": 32,
    "enable_checksums": true
  },
  "metrics": {
    "enabled": true,
    "signal_window": { "TimeMillis": 60000 },
    "resilience": {
      "enabled": true,
      "thresholds": {
        "healthy_min": 0.5,
        "attention_min": 0.2
      }
    }
  },
  "complexity": {
    "enabled": true,
    "baseline": {
      "build_time_ms": 300000,
      "min_valid_snapshots": 20,
      "update_mode": "Frozen"
    },
    "anomaly": {
      "enabled": true,
      "z_threshold_warn": 2.0,
      "z_threshold_crit": 3.0,
      "persistence_ms": 30000,
      "cooldown_ms": 120000
    }
  }
}
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `ALEC_LOG` | Log level (trace, debug, info, warn, error) | info |
| `ALEC_METRICS_ENABLED` | Enable metrics at startup | false |
| `ALEC_BASELINE_BUILD_TIME_MS` | Override baseline build time | 300000 |
| `ALEC_CONTEXT_DIR` | Directory for context files | ./contexts |
