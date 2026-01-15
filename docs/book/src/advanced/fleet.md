# Fleet Mode

Manage multiple emitters with `FleetManager`.

## Overview

Fleet mode provides:
- Per-emitter context tracking
- Cross-fleet anomaly detection
- Automatic stale emitter cleanup

## Basic Usage

```rust
use alec::FleetManager;

let mut fleet = FleetManager::new();

// Process messages from different emitters
fleet.process_message(emitter_id, &message, timestamp)?;
```

## Configuration

```rust
use alec::fleet::FleetConfig;

let config = FleetConfig {
    max_emitters: 1000,
    stale_timeout: 3600,        // 1 hour
    cleanup_interval: 300,      // 5 minutes
    cross_fleet_threshold: 3.0, // z-score threshold
};

let fleet = FleetManager::with_config(config);
```

## Anomaly Detection

Detect emitters behaving differently from the fleet:

```rust
// Get anomalous emitters (z-score > threshold)
let anomalies = fleet.anomalous_emitters();
for (emitter_id, z_score) in anomalies {
    println!("Emitter {} is anomalous (z={:.2})", emitter_id, z_score);
}
```

## Fleet Statistics

```rust
let stats = fleet.stats();
println!("Active emitters: {}", stats.active_count);
println!("Fleet mean: {:.2}", stats.mean);
println!("Fleet std dev: {:.2}", stats.std_dev);
```

## With Security

Combine with security features:

```rust
use alec::SecurityContext;

let mut security = SecurityContext::new(security_config);

fleet.process_message_secure(
    emitter_id,
    &message,
    timestamp,
    &mut security
)?;
```
