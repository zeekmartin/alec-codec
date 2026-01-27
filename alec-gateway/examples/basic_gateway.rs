// ALEC Gateway - Basic Example
//
// This example demonstrates the basic usage of the ALEC Gateway
// for managing multiple sensor channels.

use alec_gateway::{ChannelConfig, Frame, Gateway, GatewayConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ALEC Gateway Basic Example ===\n");

    // Create gateway with LoRaWAN DR4 configuration
    let config = GatewayConfig::lorawan(4);
    println!(
        "Gateway created with max frame size: {} bytes",
        config.max_frame_size
    );

    let mut gateway = Gateway::with_config(config);

    // Add sensor channels with different priorities
    gateway.add_channel(
        "temperature",
        ChannelConfig {
            priority: 1, // High priority
            ..Default::default()
        },
    )?;

    gateway.add_channel(
        "humidity",
        ChannelConfig {
            priority: 2, // Medium-high priority
            ..Default::default()
        },
    )?;

    gateway.add_channel(
        "pressure",
        ChannelConfig {
            priority: 3, // Medium priority
            ..Default::default()
        },
    )?;

    println!(
        "Added {} channels: {:?}",
        gateway.channel_count(),
        gateway.channels()
    );

    // Simulate sensor readings over time
    println!("\n--- Collecting sensor data ---");

    let base_time = 1700000000u64; // Example timestamp

    // Temperature readings (simulated)
    let temp_readings = [22.5, 22.6, 22.4, 22.7, 22.5];
    for (i, &temp) in temp_readings.iter().enumerate() {
        gateway.push("temperature", temp, base_time + i as u64 * 1000)?;
    }
    println!(
        "Temperature: {} values buffered",
        gateway.pending("temperature")?
    );

    // Humidity readings
    let humidity_readings = [65.0, 64.8, 65.2];
    for (i, &humid) in humidity_readings.iter().enumerate() {
        gateway.push("humidity", humid, base_time + i as u64 * 1000)?;
    }
    println!("Humidity: {} values buffered", gateway.pending("humidity")?);

    // Pressure readings
    let pressure_readings = [1013.25, 1013.30];
    for (i, &pressure) in pressure_readings.iter().enumerate() {
        gateway.push("pressure", pressure, base_time + i as u64 * 1000)?;
    }
    println!("Pressure: {} values buffered", gateway.pending("pressure")?);

    println!("\nTotal pending values: {}", gateway.total_pending());

    // Flush and create aggregated frame
    println!("\n--- Creating aggregated frame ---");
    let frame = gateway.flush()?;

    println!("Frame created:");
    println!("  - Version: {}", frame.version);
    println!("  - Channels: {}", frame.channel_count());
    println!("  - Size: {} bytes", frame.size());

    // Show channel details
    for channel in &frame.channels {
        println!(
            "  - Channel '{}': {} bytes of encoded data",
            channel.id,
            channel.data.len()
        );
    }

    // Serialize to bytes (ready for transmission)
    let bytes = frame.to_bytes();
    println!("\nSerialized frame: {} bytes", bytes.len());
    println!("Frame bytes (hex): {:02x?}", &bytes[..bytes.len().min(32)]);
    if bytes.len() > 32 {
        println!("  ... ({} more bytes)", bytes.len() - 32);
    }

    // Demonstrate frame parsing (receiver side)
    println!("\n--- Parsing frame (receiver side) ---");
    let parsed = Frame::from_bytes(&bytes)?;
    println!("Parsed frame:");
    println!("  - Version: {}", parsed.version);
    println!("  - Channels: {}", parsed.channel_count());

    for channel in &parsed.channels {
        println!("  - Channel '{}': {} bytes", channel.id, channel.data.len());
    }

    // After flush, buffers are empty
    println!("\n--- After flush ---");
    println!("Pending values: {}", gateway.total_pending());

    // Add more data and flush again
    println!("\n--- Second transmission ---");
    gateway.push("temperature", 22.8, base_time + 10000)?;
    gateway.push("humidity", 64.5, base_time + 10000)?;

    let frame2 = gateway.flush()?;
    println!(
        "Second frame: {} bytes, {} channels",
        frame2.size(),
        frame2.channel_count()
    );

    println!("\n=== Example complete ===");
    Ok(())
}
