# ALEC Gateway

Multi-sensor orchestration for IoT data streams.

## Overview

Gateway manages multiple sensor channels, buffers incoming values, and produces optimized frames for transmission.

## Features

- **Channel Management**: Add/remove sensors dynamically
- **Priority Scheduling**: Numeric priority (0 = highest, 255 = lowest)
- **Frame Aggregation**: Combine channels into single transmission
- **Buffer Management**: Per-channel buffering with overflow protection
- **Metrics** (optional): Entropy and resilience computation

## Quick Start

```rust
use alec_gateway::{Gateway, GatewayConfig, ChannelConfig};

// Create gateway with LoRaWAN frame limit
let config = GatewayConfig {
    max_frame_size: 242,
    ..Default::default()
};
let mut gateway = Gateway::with_config(config);

// Add sensor channels
gateway.add_channel("temperature", ChannelConfig {
    priority: 1,  // High priority
    ..Default::default()
})?;

gateway.add_channel("humidity", ChannelConfig {
    priority: 2,
    ..Default::default()
})?;

// Collect sensor readings
let timestamp = 1234567890;
gateway.push("temperature", 22.5, timestamp)?;
gateway.push("temperature", 22.6, timestamp + 1000)?;
gateway.push("humidity", 65.0, timestamp)?;

// Get aggregated frame
let frame = gateway.flush()?;
println!("Frame size: {} bytes", frame.size());

// Send frame.to_bytes() over LoRaWAN, MQTT, etc.
```

## Configuration

### GatewayConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_frame_size` | `usize` | 242 | Maximum frame size in bytes (LoRaWAN DR4) |
| `max_channels` | `usize` | 32 | Maximum number of channels |
| `enable_checksums` | `bool` | true | Enable checksums on all channels |

### ChannelConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `buffer_size` | `usize` | 64 | Maximum buffered values |
| `preload_path` | `Option<String>` | None | Path to context preload file |
| `priority` | `u8` | 128 | Channel priority (0 = highest) |
| `enable_checksum` | `bool` | true | Enable checksum for this channel |

### LoRaWAN Data Rate Presets

```rust
// Use preset for specific data rate
let config = GatewayConfig::lorawan(0);  // DR0: 51 bytes max
let config = GatewayConfig::lorawan(4);  // DR4: 242 bytes max
```

| Data Rate | Max Frame Size | SF/BW |
|-----------|---------------|-------|
| DR0 | 51 bytes | SF12/125kHz |
| DR1 | 51 bytes | SF11/125kHz |
| DR2 | 51 bytes | SF10/125kHz |
| DR3 | 115 bytes | SF9/125kHz |
| DR4 | 242 bytes | SF8/125kHz |
| DR5 | 242 bytes | SF7/125kHz |

## Frame Format

```
┌─────────┬─────────┬─────────────────────────────────┐
│ Version │ Flags   │ Channel Data...                 │
│ 1 byte  │ 1 byte  │ Variable                        │
└─────────┴─────────┴─────────────────────────────────┘

Channel Data:
┌───────────┬────────┬──────────────┐
│ Channel ID│ Length │ ALEC Payload │
│ 1 byte    │ 1 byte │ Variable     │
└───────────┴────────┴──────────────┘
```

## API Reference

### Gateway

```rust
impl Gateway {
    /// Create with default configuration
    pub fn new() -> Self;

    /// Create with custom configuration
    pub fn with_config(config: GatewayConfig) -> Self;

    /// Add a new channel
    pub fn add_channel(&mut self, id: &str, config: ChannelConfig) -> Result<()>;

    /// Remove a channel
    pub fn remove_channel(&mut self, id: &str) -> Result<()>;

    /// Push a value to a channel
    pub fn push(&mut self, channel_id: &str, value: f64, timestamp_ms: u64) -> Result<()>;

    /// Push multiple values
    pub fn push_multi(&mut self, channel_id: &str, values: &[(f64, u64)]) -> Result<()>;

    /// Flush all channels to a frame
    pub fn flush(&mut self) -> Result<Frame>;

    /// Check if any channel has data
    pub fn has_pending_data(&self) -> bool;

    /// List all channel IDs
    pub fn channels(&self) -> Vec<String>;

    /// Get current channel count
    pub fn channel_count(&self) -> usize;
}
```

### With Metrics (feature-gated)

```rust
#[cfg(feature = "metrics")]
impl Gateway {
    /// Enable metrics computation
    pub fn enable_metrics(&mut self, config: MetricsConfig);

    /// Disable metrics computation
    pub fn disable_metrics(&mut self);

    /// Get last computed metrics snapshot
    pub fn last_metrics(&self) -> Option<&MetricsSnapshot>;

    /// Check if metrics are enabled
    pub fn metrics_enabled(&self) -> bool;
}
```

## Error Handling

```rust
pub enum GatewayError {
    /// Channel already exists
    ChannelExists(String),

    /// Channel not found
    ChannelNotFound(String),

    /// Buffer is full
    BufferFull(String),

    /// Maximum channels reached
    MaxChannelsReached,

    /// Frame too large
    FrameTooLarge,

    /// Codec error
    CodecError(alec::Error),
}
```

## Best Practices

1. **Size your buffers**: Match buffer size to expected burst rate between flushes
2. **Use priorities**: Critical data should have low priority numbers (0-50)
3. **Flush regularly**: Don't let buffers fill up; `BufferFull` means data loss
4. **Handle errors**: Always check `Result` from `push()` and `flush()`
5. **Preload contexts**: Use preloads for faster cold-start compression

## Examples

### Basic Multi-Sensor Gateway

```rust
use alec_gateway::{Gateway, GatewayConfig, ChannelConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut gateway = Gateway::new();

    // Add sensors with different priorities
    gateway.add_channel("fire_alarm", ChannelConfig::with_priority(0))?;
    gateway.add_channel("temperature", ChannelConfig::with_priority(50))?;
    gateway.add_channel("humidity", ChannelConfig::with_priority(100))?;
    gateway.add_channel("ambient_light", ChannelConfig::with_priority(200))?;

    // Simulate data collection
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64;

    gateway.push("temperature", 22.5, now)?;
    gateway.push("humidity", 65.0, now)?;
    gateway.push("ambient_light", 450.0, now)?;

    // Flush and transmit
    let frame = gateway.flush()?;
    println!("Frame: {} bytes", frame.size());

    Ok(())
}
```

### With Context Preloads

```rust
use alec_gateway::{Gateway, ChannelConfig};

let mut gateway = Gateway::new();

// Use pre-trained context for faster compression
gateway.add_channel("temperature", ChannelConfig::with_preload(
    "contexts/temperature_sensor.alec-context"
))?;
```

## Integration with Complexity

See [INTEGRATION.md](INTEGRATION.md) for using Gateway with the Complexity module.
