# Performance Tuning

Optimize ALEC for your use case.

## Build Configuration

**Always use release mode for benchmarks:**

```bash
cargo build --release
cargo test --release
```

## Memory Optimization

Reduce context memory:

```rust
use alec::context::ContextConfig;

let config = ContextConfig {
    max_patterns: 1000,      // Fewer patterns
    max_memory: 16 * 1024,   // 16KB limit
    history_size: 50,        // Less history
    ..Default::default()
};
```

## Evolution Tuning

Balance compression vs CPU:

```rust
use alec::context::EvolutionConfig;

// Aggressive pruning (lower memory, more CPU)
let aggressive = EvolutionConfig {
    min_frequency: 5,
    max_age: 1000,
    evolution_interval: 50,
    ..Default::default()
};

// Relaxed pruning (more memory, less CPU)
let relaxed = EvolutionConfig {
    min_frequency: 1,
    max_age: 100000,
    evolution_interval: 1000,
    ..Default::default()
};
```

## Benchmarking

Run stress tests:

```bash
cargo test --release stress -- --ignored
```

Expected minimums:
- Encoding: >100k msg/s
- Roundtrip: >50k msg/s
- Fleet: >10k msg/s

## Health Monitoring

Monitor performance in production:

```rust
use alec::HealthCheckable;

let check = context.health_check();
if check.status == HealthStatus::Degraded {
    // Consider evolution or cleanup
    context.evolve();
}
```
