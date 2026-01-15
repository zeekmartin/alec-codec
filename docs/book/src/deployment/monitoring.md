# Monitoring

Monitor ALEC in production.

## Health Checks

```rust
use alec::{HealthMonitor, HealthCheckable};

let mut monitor = HealthMonitor::new();

// Check context health
monitor.add_check(context.health_check());

// Get overall status
println!("System: {:?}", monitor.status());
println!("{}", monitor.report());
```

## Compression Metrics

```rust
use alec::CompressionMetrics;

let mut metrics = CompressionMetrics::new();

// Record each encode
encoder.encode_with_metrics(&data, &class, &context, &mut metrics);

// Check periodically
println!("Compression ratio: {:.1}%", metrics.compression_ratio() * 100.0);
println!("Encoding distribution: {:?}", metrics.encoding_distribution());
```

## Fleet Monitoring

```rust
let stats = fleet.stats();
println!("Active emitters: {}", stats.active_count);
println!("Fleet mean: {:.2}", stats.mean);

// Check for anomalies
for (id, score) in fleet.anomalous_emitters() {
    alert_anomaly(id, score);
}
```

## Circuit Breaker Status

```rust
match breaker.state() {
    CircuitState::Closed => { /* Normal */ }
    CircuitState::Open => { /* Failing */ }
    CircuitState::HalfOpen => { /* Recovering */ }
}
```

## Key Metrics to Track

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| Compression ratio | Encoding efficiency | <30% |
| Decode errors | Corruption/sync issues | >1% |
| Rate limit hits | Potential attack | >100/min |
| Memory usage | Context size | >50MB |
| Anomalous emitters | Fleet health | >5% |
