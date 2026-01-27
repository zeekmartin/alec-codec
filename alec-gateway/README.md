# ALEC Gateway

Multi-sensor orchestration layer for ALEC compression.

## Overview

ALEC Gateway manages multiple ALEC encoder instances for IoT gateways that aggregate data from many sensors into efficient transmission frames.

## Features

- **Multi-channel management**: Handle dozens of sensor channels
- **Priority-based aggregation**: Critical sensors get bandwidth first
- **Frame packing**: Optimize for LoRaWAN/MQTT payload limits
- **Preload support**: Load pre-trained contexts per channel

## Installation

```toml
[dependencies]
alec-gateway = "0.1"
```

## Quick Start

```rust
use alec_gateway::{Gateway, ChannelConfig, GatewayConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    Ok(())
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  IoT Gateway                                                │
│  ┌────────────────────────────────────────────────────────┐│
│  │  ALEC Gateway                                          ││
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐               ││
│  │  │ Channel  │ │ Channel  │ │ Channel  │  ...          ││
│  │  │ Temp #1  │ │ Humid #1 │ │ Accel #1 │               ││
│  │  │ [Context]│ │ [Context]│ │ [Context]│               ││
│  │  └────┬─────┘ └────┬─────┘ └────┬─────┘               ││
│  │       │            │            │                      ││
│  │       └────────────┼────────────┘                      ││
│  │                    ▼                                   ││
│  │              ┌───────────┐                             ││
│  │              │ Aggregator│                             ││
│  │              └─────┬─────┘                             ││
│  │                    ▼                                   ││
│  │              ┌───────────┐                             ││
│  │              │  Frame    │  → LoRaWAN / MQTT / etc.   ││
│  │              └───────────┘                             ││
│  └────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

## Configuration

### Gateway Configuration

```rust
use alec_gateway::GatewayConfig;

let config = GatewayConfig {
    // Maximum frame size in bytes (default: 242 for LoRaWAN DR0)
    max_frame_size: 242,

    // Maximum number of channels (default: 32)
    max_channels: 32,

    // Enable checksums on all channels (default: true)
    enable_checksums: true,
};
```

### LoRaWAN Data Rates

```rust
// Use built-in LoRaWAN configuration
let config = GatewayConfig::lorawan(4); // DR4: 242 bytes max
```

| Data Rate | Max Payload |
|-----------|-------------|
| DR0       | 51 bytes    |
| DR1       | 51 bytes    |
| DR2       | 51 bytes    |
| DR3       | 115 bytes   |
| DR4-DR5   | 242 bytes   |

### Channel Configuration

```rust
use alec_gateway::ChannelConfig;

let config = ChannelConfig {
    // Buffer size for pending values (default: 64)
    buffer_size: 64,

    // Preload file path (optional)
    preload_path: Some("contexts/temperature.alec-context".into()),

    // Priority: 0 = highest (default: 128)
    priority: 1,

    // Enable checksum for this channel (default: true)
    enable_checksum: true,
};
```

## Frame Format

Frames aggregate data from multiple channels into a single transmission unit:

```
[version: 1] [channel_count: 1] [channel_data...]

channel_data:
[id_len: 1] [id: N] [data_len: 2 LE] [data: M]
```

### Parsing Frames

```rust
use alec_gateway::Frame;

// Serialize
let bytes = frame.to_bytes();

// Deserialize
let frame = Frame::from_bytes(&bytes)?;

// Access channel data
for channel in &frame.channels {
    println!("Channel {}: {} bytes", channel.id, channel.data.len());
}
```

## Priority System

Channels are processed in priority order during aggregation:

- **Priority 0-10**: Critical sensors (always included first)
- **Priority 11-100**: Important sensors
- **Priority 101-200**: Normal sensors
- **Priority 201-255**: Low priority (may be dropped if frame is full)

```rust
gateway.add_channel("critical_temp", ChannelConfig::with_priority(1))?;
gateway.add_channel("normal_humid", ChannelConfig::with_priority(128))?;
gateway.add_channel("debug_log", ChannelConfig::with_priority(250))?;
```

## Preload Support

Use pre-trained contexts for optimal compression from the first byte:

```rust
let config = ChannelConfig::with_preload("contexts/demo_temperature_v1.alec-context");
gateway.add_channel("temp", config)?;
```

## Error Handling

```rust
use alec_gateway::{Gateway, GatewayError};

match gateway.push("temp", 22.5, timestamp) {
    Ok(()) => println!("Value pushed"),
    Err(GatewayError::ChannelNotFound(id)) => eprintln!("Unknown channel: {}", id),
    Err(GatewayError::BufferFull(id)) => eprintln!("Buffer full for: {}", id),
    Err(e) => eprintln!("Error: {}", e),
}
```

## Future Roadmap

- **Phase 2**: Context Library, hot-plug channels, auto-detection
- **Phase 3**: Complexity monitoring, anomaly detection
- **Phase 4**: QoS & scheduling, bandwidth limiting
- **Phase 5**: Cloud decoder, error recovery, logging

## License

AGPL-3.0 or commercial license. See [LICENSE](../LICENSE).
