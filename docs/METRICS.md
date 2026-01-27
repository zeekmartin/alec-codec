# ALEC Metrics

Entropy-based observability for IoT data streams.

## Overview

The Metrics module provides real-time computation of:
- **Signal Entropy**: Per-channel and joint entropy
- **Total Correlation**: Redundancy measure across channels
- **Payload Entropy**: Compressed data randomness
- **Resilience Index**: System health indicator
- **Criticality Ranking**: Importance of each sensor

## Enable Metrics

```rust
use alec_gateway::{Gateway, GatewayConfig, ChannelConfig};
use alec_gateway::metrics::{MetricsConfig, ResilienceConfig};

let mut gateway = Gateway::new();

// Enable metrics
gateway.enable_metrics(MetricsConfig {
    enabled: true,
    resilience: ResilienceConfig {
        enabled: true,
        ..Default::default()
    },
    ..Default::default()
});

// ... push values and flush ...

// Access metrics
if let Some(snapshot) = gateway.last_metrics() {
    println!("TC: {} bits", snapshot.signal.total_corr);
    if let Some(res) = &snapshot.resilience {
        println!("R: {:?}", res.r);
    }
}
```

## Metrics Explained

### Signal Entropy (H_i)

Shannon entropy of each channel's signal values:

```
H(X) = -Σ p(x) log₂ p(x)
```

**Interpretation:**
- High H → High variability (good for detecting changes)
- Low H → Predictable signal (potential stuck sensor)

### Joint Entropy (H_joint)

Entropy of all channels combined:

```
H(X₁, X₂, ..., Xₙ)
```

**Interpretation:**
- Measures total information in the system
- Upper bound: Σ H(X_i) when independent

### Total Correlation (TC)

Redundancy measure (information shared across channels):

```
TC = Σ H(X_i) - H(X₁, ..., Xₙ)
```

**Interpretation:**
- TC > 0 → Channels are correlated
- TC ≈ 0 → Channels are independent
- Rising TC → Increasing redundancy

### Payload Entropy (H_bytes)

Entropy of the compressed frame bytes:

```
H(bytes) = -Σ p(byte) log₂ p(byte)
```

**Interpretation:**
- Max 8 bits (uniform distribution)
- Lower → Better compression potential
- Sudden changes → Anomaly indicator

### Resilience Index (R)

Normalized redundancy measure:

```
R = TC / Σ H(X_i)    (0 ≤ R ≤ 1)
```

**Interpretation:**

| R Value | Zone | Meaning |
|---------|------|---------|
| R ≥ 0.5 | Healthy | High redundancy, can lose sensors |
| 0.2 ≤ R < 0.5 | Attention | Moderate redundancy |
| R < 0.2 | Critical | Low redundancy, fragile system |

### Criticality (ΔR_k)

Impact of removing channel k on resilience:

```
ΔR_k = R_all - R_{-k}
```

**Interpretation:**
- High ΔR_k → Channel is critical (removing it drops R significantly)
- Low ΔR_k → Channel is redundant (can be removed safely)
- Ranked by ΔR_k descending

## Configuration

### MetricsConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | false | Master switch |
| `signal_compute` | `SignalComputeSchedule` | 10 flushes or 10s | When to compute |
| `signal_window` | `SignalWindow` | 60s | Sample window |
| `alignment` | `AlignmentStrategy` | SampleAndHold | Align async channels |
| `missing_data` | `MissingDataPolicy` | DropIncomplete | Handle missing values |
| `normalization` | `NormalizationConfig` | Z-Score | Pre-normalize signals |
| `signal_estimator` | `SignalEstimator` | GaussianCovariance | Entropy estimator |
| `payload` | `PayloadMetricsConfig` | ... | Payload settings |
| `resilience` | `ResilienceConfig` | disabled | R computation |
| `numerics` | `NumericsConfig` | ... | Numerical safety |

### SignalComputeSchedule

```rust
pub enum SignalComputeSchedule {
    /// Compute once every N flushes
    EveryNFlushes(u32),
    /// Compute at most once every T milliseconds
    EveryMillis(u64),
    /// Compute on whichever trigger fires first (default)
    NFlushesOrMillis { n_flushes: u32, millis: u64 },
}
```

### SignalWindow

```rust
pub enum SignalWindow {
    /// Keep samples within the last `millis` milliseconds (default: 60_000)
    TimeMillis(u64),
    /// Keep only the last `n` samples per channel
    LastNSamples(usize),
}
```

### AlignmentStrategy

```rust
pub enum AlignmentStrategy {
    /// Sample & Hold (ZOH): pick latest value <= t_ref (default)
    SampleAndHold,
    /// Nearest sample to t_ref
    Nearest,
    /// Linear interpolation (requires bracketing samples)
    LinearInterpolation,
}
```

### ResilienceConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | false | Enable R computation |
| `criticality.enabled` | `bool` | true | Compute ΔR_k |
| `criticality.max_channels` | `usize` | 16 | Max channels for criticality |
| `thresholds.healthy_min` | `f64` | 0.5 | R threshold for healthy |
| `thresholds.attention_min` | `f64` | 0.2 | R threshold for attention |
| `min_sum_h` | `f64` | 0.1 | Min total entropy for valid R |

### NumericsConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `min_aligned_samples` | `usize` | 32 | Min samples for metrics |
| `covariance_epsilon` | `f64` | 1e-8 | Regularization for Σ |
| `max_channels_for_joint` | `usize` | 32 | Max channels for H_joint |

## MetricsSnapshot Schema

```json
{
  "version": 1,
  "timestamp_ms": 1706000000000,
  "window": {
    "kind": "time_ms",
    "value": 60000,
    "aligned_samples": 50,
    "channels_included": 4
  },
  "signal": {
    "valid": true,
    "log_base": "log2",
    "h_per_channel": [
      { "channel_id": "temp", "h": 3.21 },
      { "channel_id": "humid", "h": 2.87 }
    ],
    "sum_h": 6.08,
    "h_joint": 5.12,
    "total_corr": 0.96
  },
  "payload": {
    "frame_size_bytes": 48,
    "h_bytes": 6.21
  },
  "resilience": {
    "enabled": true,
    "r": 0.158,
    "zone": "critical",
    "criticality": {
      "enabled": true,
      "ranking": [
        { "channel_id": "temp", "delta_r": 0.12 },
        { "channel_id": "humid", "delta_r": 0.08 }
      ],
      "note": "delta_r = R_all - R_without_channel (leave-one-out)"
    }
  },
  "flags": ["SIGNAL_COMPUTED"]
}
```

## Computation Details

### Gaussian Entropy Estimation

For continuous signals, we assume multivariate Gaussian distribution:

```
H(X) = 0.5 * log((2πe)^n * |Σ|)
```

Where:
- n = number of dimensions (channels)
- Σ = covariance matrix
- |Σ| = determinant of Σ

### Numerical Stability

1. **Regularization**: Σ' = Σ + ε·I (prevents singular matrices)
2. **Log determinant**: Use eigenvalue decomposition for stability
3. **Minimum samples**: Require min_aligned_samples before computing

### Missing Data Handling

| Policy | Behavior |
|--------|----------|
| `DropIncompleteSnapshots` | Skip time points with any missing channel |
| `AllowPartial { min_channels }` | Allow partial data if >= min channels present |
| `FillWithLastKnown` | Forward-fill missing with last known value |

## Best Practices

1. **Set appropriate window**: 60s is good for most IoT; shorter for fast changes
2. **Enable resilience only when needed**: It's O(n²) for n channels
3. **Use SampleAndHold alignment**: Most robust for gateways
4. **Monitor invalid_reason**: Explains why signal metrics are unavailable
5. **Watch for zone transitions**: healthy → attention → critical
