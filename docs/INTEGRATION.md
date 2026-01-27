# ALEC Integration Guide

Patterns for integrating ALEC components in your application.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│  Your Application                                               │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  Sensor Layer                                               ││
│  │  [Temp] [Humid] [Pressure] [Accel] ...                     ││
│  └──────────────────────────┬──────────────────────────────────┘│
│                             │ push()                            │
│                             ▼                                   │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  ALEC Gateway                                               ││
│  │  - Channel management                                       ││
│  │  - Frame aggregation                                        ││
│  │  - Metrics (optional)                                       ││
│  └──────────────────────────┬──────────────────────────────────┘│
│                             │ flush()                           │
│                             ▼                                   │
│  ┌───────────────┐    ┌───────────────────────────────────────┐│
│  │  Frame        │    │  MetricsSnapshot                      ││
│  │  → Network    │    │  → Complexity (optional)              ││
│  └───────────────┘    │  → Dashboard                          ││
│                       │  → Alerting                           ││
│                       └───────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

## Integration Patterns

### Pattern 1: Gateway Only (Compression)

Simplest setup - just compression without observability.

```rust
use alec_gateway::{Gateway, ChannelConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut gateway = Gateway::new();

    // Register sensors
    gateway.add_channel("temperature", ChannelConfig::default())?;
    gateway.add_channel("humidity", ChannelConfig::default())?;

    // Main loop
    loop {
        let timestamp = get_timestamp_ms();

        // Collect sensor data
        gateway.push("temperature", read_temperature(), timestamp)?;
        gateway.push("humidity", read_humidity(), timestamp)?;

        // Flush every N seconds or when buffer fills
        if should_flush() {
            let frame = gateway.flush()?;
            send_over_network(frame.to_bytes());
        }
    }
}
```

### Pattern 2: Gateway + Metrics (Observability)

Add real-time entropy and resilience monitoring.

```rust
use alec_gateway::{Gateway, ChannelConfig};
use alec_gateway::metrics::{MetricsConfig, ResilienceConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut gateway = Gateway::new();

    // Enable metrics with resilience
    gateway.enable_metrics(MetricsConfig {
        enabled: true,
        resilience: ResilienceConfig {
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    });

    // Register sensors
    gateway.add_channel("temperature", ChannelConfig::default())?;
    gateway.add_channel("humidity", ChannelConfig::default())?;

    // Main loop
    loop {
        collect_sensor_data(&mut gateway)?;

        if should_flush() {
            let frame = gateway.flush()?;
            send_over_network(frame.to_bytes());

            // Access metrics
            if let Some(metrics) = gateway.last_metrics() {
                log_metrics(metrics);

                // Check resilience zone
                if let Some(res) = &metrics.resilience {
                    if res.zone == Some("critical".to_string()) {
                        send_alert("System resilience critical!");
                    }
                }
            }
        }
    }
}
```

### Pattern 3: Full Stack (Gateway + Metrics + Complexity)

Complete observability with anomaly detection.

```rust
use alec_gateway::{Gateway, ChannelConfig};
use alec_gateway::metrics::{MetricsConfig, ResilienceConfig};
use alec_complexity::{ComplexityEngine, ComplexityConfig, MetricsSnapshotExt};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup Gateway with Metrics
    let mut gateway = Gateway::new();
    gateway.enable_metrics(MetricsConfig {
        enabled: true,
        resilience: ResilienceConfig { enabled: true, ..Default::default() },
        ..Default::default()
    });

    // Setup Complexity Engine
    let mut complexity = ComplexityEngine::new(ComplexityConfig {
        enabled: true,
        baseline: BaselineConfig {
            build_time_ms: 60_000,  // 1 minute baseline
            min_valid_snapshots: 10,
            ..Default::default()
        },
        ..Default::default()
    });

    // Register sensors
    for sensor in &["temp", "humid", "pressure"] {
        gateway.add_channel(sensor, ChannelConfig::default())?;
    }

    // Main loop
    loop {
        collect_sensor_data(&mut gateway)?;

        if should_flush() {
            let frame = gateway.flush()?;
            send_over_network(frame.to_bytes());

            // Feed metrics to complexity
            if let Some(metrics) = gateway.last_metrics() {
                let input = metrics.to_complexity_input();

                if let Some(snapshot) = complexity.process(&input) {
                    // Handle events
                    for event in &snapshot.events {
                        handle_complexity_event(event);
                    }

                    // Export snapshot for dashboard
                    export_snapshot(&snapshot);
                }
            }
        }
    }
}
```

### Pattern 4: Complexity Standalone (External Data)

Use Complexity without Gateway for external data sources.

```rust
use alec_complexity::{ComplexityEngine, ComplexityConfig, GenericInput};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = ComplexityEngine::new(ComplexityConfig {
        enabled: true,
        ..Default::default()
    });

    // Receive data from external source (Prometheus, InfluxDB, etc.)
    for record in external_data_stream() {
        let input = GenericInput::new(record.timestamp_ms, record.entropy)
            .with_tc(record.tc)
            .with_h_joint(record.h_joint)
            .with_r(record.r)
            .build();

        if let Some(snapshot) = engine.process(&input) {
            for event in &snapshot.events {
                forward_to_alerting(event);
            }
        }
    }

    Ok(())
}
```

### Pattern 5: JSON-Based Integration

For non-Rust applications or microservices.

```rust
use alec_complexity::{ComplexityEngine, ComplexityConfig, GenericInput};

// HTTP endpoint handler
fn handle_metrics_input(json_body: &str) -> Result<String, Error> {
    let input = GenericInput::from_json(json_body)?;
    let snapshot = ENGINE.process(&input.to_snapshot());

    match snapshot {
        Some(s) => Ok(s.to_json()?),
        None => Ok("{}".to_string()),
    }
}
```

## Threading Patterns

### Single-Threaded (Simple)

```rust
// Everything on main thread
loop {
    collect_sensors(&mut gateway);
    if should_flush() {
        process_flush(&mut gateway, &mut complexity);
    }
}
```

### Multi-Threaded (Performance)

```rust
use std::sync::mpsc;
use std::thread;

// Sensor collection thread
let (tx_sensors, rx_sensors) = mpsc::channel();
thread::spawn(move || {
    loop {
        let reading = collect_sensor();
        tx_sensors.send(reading).unwrap();
    }
});

// Gateway thread
let (tx_metrics, rx_metrics) = mpsc::channel();
thread::spawn(move || {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(MetricsConfig { enabled: true, ..Default::default() });

    for reading in rx_sensors {
        gateway.push(&reading.channel, reading.value, reading.timestamp).ok();

        if should_flush() {
            let frame = gateway.flush().unwrap();
            send_frame(frame);

            if let Some(metrics) = gateway.last_metrics() {
                tx_metrics.send(metrics.clone()).ok();
            }
        }
    }
});

// Complexity thread
thread::spawn(move || {
    let mut complexity = ComplexityEngine::new(ComplexityConfig::default());

    for metrics in rx_metrics {
        let input = metrics.to_complexity_input();
        if let Some(snapshot) = complexity.process(&input) {
            handle_snapshot(snapshot);
        }
    }
});
```

## Error Handling

### Graceful Degradation

```rust
// Gateway errors don't stop the application
match gateway.push("sensor", value, timestamp) {
    Ok(()) => {},
    Err(GatewayError::BufferFull(ch)) => {
        log::warn!("Buffer full for {}, dropping value", ch);
    }
    Err(GatewayError::ChannelNotFound(ch)) => {
        log::error!("Unknown channel: {}", ch);
    }
    Err(e) => {
        log::error!("Gateway error: {:?}", e);
    }
}

// Metrics failures don't affect compression
let frame = gateway.flush()?;  // Always works
let metrics = gateway.last_metrics();  // May be None
```

### Complexity Disabled Gracefully

```rust
let snapshot = complexity.process(&input);
if snapshot.is_none() {
    // Complexity disabled or not enough data
    // Application continues normally
}
```

## Dashboard Integration

### Grafana/InfluxDB

```rust
// Export metrics to InfluxDB
fn export_to_influxdb(metrics: &MetricsSnapshot) {
    let point = influxdb::Point::new("alec_metrics")
        .add_field("h_bytes", metrics.payload.h_bytes)
        .add_field("total_corr", metrics.signal.total_corr)
        .add_field("h_joint", metrics.signal.h_joint);

    if let Some(res) = &metrics.resilience {
        if let Some(r) = res.r {
            point.add_field("resilience_r", r);
        }
    }

    influxdb_client.write_point(point);
}
```

### Prometheus Metrics

```rust
use prometheus::{Gauge, register_gauge};

lazy_static! {
    static ref ENTROPY_GAUGE: Gauge = register_gauge!(
        "alec_payload_entropy_bits",
        "Payload entropy in bits"
    ).unwrap();
    static ref RESILIENCE_GAUGE: Gauge = register_gauge!(
        "alec_resilience_r",
        "Resilience index R"
    ).unwrap();
}

fn update_prometheus(metrics: &MetricsSnapshot) {
    ENTROPY_GAUGE.set(metrics.payload.h_bytes);
    if let Some(res) = &metrics.resilience {
        if let Some(r) = res.r {
            RESILIENCE_GAUGE.set(r);
        }
    }
}
```

## Testing

### Unit Testing with Mock Data

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_baseline() {
        let mut engine = ComplexityEngine::new(ComplexityConfig {
            enabled: true,
            baseline: BaselineConfig {
                build_time_ms: 0,
                min_valid_snapshots: 3,
                ..Default::default()
            },
            ..Default::default()
        });

        // Build baseline
        for i in 0..3 {
            let input = GenericInput::new(i * 1000, 5.0 + (i as f64 * 0.1)).build();
            engine.process(&input);
        }

        // Verify baseline locked
        let input = GenericInput::new(3000, 5.5).build();
        let snapshot = engine.process(&input).unwrap();
        assert!(snapshot.is_baseline_locked());
    }
}
```

### Integration Testing

```rust
#[test]
fn test_full_pipeline() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(MetricsConfig { enabled: true, ..Default::default() });
    gateway.add_channel("test", ChannelConfig::default()).unwrap();

    // Push data
    for i in 0..100 {
        gateway.push("test", 20.0 + (i as f64 * 0.01), i * 1000).unwrap();
    }

    // Flush and check metrics
    let frame = gateway.flush().unwrap();
    assert!(frame.size() > 0);

    let metrics = gateway.last_metrics();
    assert!(metrics.is_some());
}
```
