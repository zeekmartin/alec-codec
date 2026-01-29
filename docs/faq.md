# ALEC Frequently Asked Questions

## General

### What is ALEC?

ALEC (Adaptive Lazy Evolving Compression) is a compression codec and observability suite designed for bandwidth-constrained IoT environments. It combines:

1. **Lazy Compression**: Decides if data should be transmitted before compressing
2. **Evolving Context**: Dictionary improves over time
3. **Information-Theoretic Metrics**: Real-time entropy and resilience monitoring
4. **Anomaly Detection**: Automatic detection of system changes

### What compression ratios can I expect?

| Situation | Typical Ratio |
|-----------|---------------|
| Random data | 0.8-1.0 (little gain) |
| First day (learning) | 0.5-0.7 |
| After one week | 0.1-0.3 |
| Highly predictable data | 0.02-0.08 |

### Is ALEC lossy or lossless?

**Lossless** for numeric values. Reconstructed data is identical to original data (at configured precision).

However, ALEC may *decide not to transmit* certain data (P4, P5 priorities). This is filtering, not compression loss.

---

## Gateway

### What is ALEC Gateway?

Gateway manages multiple ALEC encoder instances for IoT gateways that aggregate data from many sensors into efficient transmission frames.

### How many channels can Gateway handle?

Default maximum is 32 channels. This is configurable via `GatewayConfig.max_channels`. Each channel uses approximately 2KB of memory.

### What happens if a buffer fills up?

`push()` returns `GatewayError::BufferFull`. The value is dropped. To avoid this:
- Increase `buffer_size` in `ChannelConfig`
- Flush more frequently
- Handle the error and log dropped values

### Can I use Gateway without Metrics?

Yes. Metrics is an optional feature that must be explicitly enabled:

```toml
# Without metrics (default)
alec-gateway = "0.1"

# With metrics
alec-gateway = { version = "0.1", features = ["metrics"] }
```

---

## Metrics

### What is Total Correlation (TC)?

Total Correlation measures the redundancy (shared information) across all channels:

```
TC = Σ H(X_i) - H(X₁, ..., Xₙ)
```

- TC = 0: Channels are completely independent
- TC > 0: Channels share information (redundancy exists)
- Higher TC: More redundancy, system can tolerate sensor loss

### What is the Resilience Index (R)?

R is the normalized Total Correlation:

```
R = TC / Σ H(X_i)    (0 ≤ R ≤ 1)
```

| R Value | Zone | Meaning |
|---------|------|---------|
| R ≥ 0.5 | Healthy | High redundancy |
| 0.2 ≤ R < 0.5 | Attention | Moderate redundancy |
| R < 0.2 | Critical | Low redundancy, fragile |

### Why is my signal.valid = false?

Common reasons:
- Insufficient samples (need `min_aligned_samples`, default 32)
- No overlapping timestamps across channels
- All channels have zero variance

Check `signal.invalid_reason` for details.

### What is Criticality (ΔR)?

Criticality measures how important each channel is to system redundancy:

```
ΔR_k = R_all - R_without_k
```

High ΔR = removing the channel significantly drops R = critical channel.

---

## Complexity

### What is ALEC Complexity?

Complexity provides temporal analysis of metrics:
- Baseline learning (statistical summary of "normal")
- Delta/Z-score computation (deviation from baseline)
- S-lite structure analysis (pairwise dependencies)
- Anomaly event detection (with persistence/cooldown)

### How long does baseline building take?

Default: 5 minutes (`build_time_ms: 300_000`) AND 20 samples (`min_valid_snapshots: 20`). Both conditions must be met.

For faster startup, reduce these values:

```rust
baseline: BaselineConfig {
    build_time_ms: 60_000,    // 1 minute
    min_valid_snapshots: 10,
    ..Default::default()
}
```

### What do Z-scores mean?

Z-score = (current - baseline_mean) / baseline_std

| |Z| | Interpretation |
|-----|----------------|
| < 2 | Normal |
| 2-3 | Warning |
| ≥ 3 | Critical |

### Why aren't my anomaly events firing?

Check these settings:
1. `anomaly.enabled` is true
2. `persistence_ms`: Condition must persist this long (default 30s)
3. `cooldown_ms`: Same event blocked for this period (default 2min)
4. Baseline is locked (check `baseline.state`)

### Can I use Complexity without Gateway?

Yes. Complexity can consume data from any source via `GenericInput`:

```rust
let input = GenericInput::new(timestamp_ms, entropy)
    .with_tc(tc_value)
    .with_r(resilience)
    .build();
```

---

## Integration

### How do I integrate all three components?

```rust
// 1. Create Gateway with Metrics
let mut gateway = Gateway::new();
gateway.enable_metrics(MetricsConfig { enabled: true, ..Default::default() });

// 2. Create Complexity Engine
let mut complexity = ComplexityEngine::new(ComplexityConfig { enabled: true, ..Default::default() });

// 3. Feed data flow
gateway.push("sensor", value, timestamp)?;
let frame = gateway.flush()?;

if let Some(metrics) = gateway.last_metrics() {
    let input = metrics.to_complexity_input();
    if let Some(snapshot) = complexity.process(&input) {
        // Handle events
    }
}
```

### Can I use Prometheus/Grafana with ALEC?

Yes. Export `MetricsSnapshot` fields to your time-series database:
- `payload.h_bytes` → `alec_payload_entropy`
- `signal.total_corr` → `alec_total_correlation`
- `resilience.r` → `alec_resilience`

See [INTEGRATION.md](INTEGRATION.md) for examples.

### Is ALEC thread-safe?

Individual components are not thread-safe. For multi-threaded applications:
- Use one Gateway per thread, or
- Use channels (mpsc) to communicate between threads
- Wrap components in `Mutex` if needed

---

## Performance

### What's the memory footprint?

| Component | Typical Memory |
|-----------|---------------|
| Gateway (per channel) | ~2KB |
| Metrics window (60s) | ~100KB |
| Complexity baseline | ~50KB |

### What's the CPU overhead?

| Operation | Typical Latency |
|-----------|----------------|
| push() | < 1µs |
| flush() 10 channels | < 5ms |
| Metrics computation | < 10ms |
| Complexity snapshot | < 2ms |

### How do I minimize overhead?

1. Disable metrics if not needed
2. Increase `signal_compute` interval
3. Reduce `signal_window` size
4. Disable resilience/criticality computation
5. Use `Frozen` baseline mode

---

## Troubleshooting

### Gateway: "ChannelNotFound" error

Channel was not added before pushing. Call `add_channel()` first:

```rust
gateway.add_channel("my_sensor", ChannelConfig::default())?;
gateway.push("my_sensor", value, timestamp)?;
```

### Metrics: signal.h_joint is 0

This means:
- Only one channel exists (joint entropy = single entropy)
- Channels have zero variance
- Alignment produced no overlapping samples

### Complexity: No deltas/z_scores in snapshot

Baseline is still building. Check:
- `baseline.state` should be "locked"
- `baseline.progress` should be 1.0

### Events not appearing in snapshot

1. Check `anomaly.enabled = true`
2. Check specific event type is enabled in `events`
3. Check persistence timer (condition must persist)
4. Check cooldown (same event recently fired)

---

## Licensing

### What license is ALEC under?

ALEC is **dual-licensed**:

1. **AGPL-3.0** (Open Source): Free for open source projects, research, and personal use. You must open-source your code if you distribute ALEC.

2. **Commercial License**: For proprietary use without open-source obligations. Starting at $500/year.

### Which license do I need?

| Use Case | License |
|----------|---------|
| Open source project | AGPL-3.0 |
| Research/academic | AGPL-3.0 |
| Internal tools (no distribution) | AGPL-3.0 |
| SaaS/network service | Commercial |
| Proprietary product | Commercial |
| Embedded in closed-source | Commercial |

Contact: https://alec-codec.com/pricing
