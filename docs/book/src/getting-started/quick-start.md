# Quick Start

This guide will get you up and running with ALEC in 5 minutes.

## Basic Usage

### 1. Create Components

```rust
use alec::{Encoder, Decoder, Context, Classifier, RawData};

fn main() {
    // Create encoder (on the emitter side)
    let mut encoder = Encoder::new();

    // Create decoder (on the receiver side)
    let mut decoder = Decoder::new();

    // Create classifier (determines message priority)
    let classifier = Classifier::default();

    // Create shared context (must be synchronized)
    let mut context = Context::new();
}
```

### 2. Encode Data

```rust
// Your sensor data
let temperature = 22.5;
let timestamp = 1234567890;

// Wrap in RawData
let data = RawData::new(temperature, timestamp);

// Classify (determines priority P1-P5)
let classification = classifier.classify(&data, &context);

// Encode
let message = encoder.encode(&data, &classification, &context);

// Message is now ready to transmit!
println!("Encoded {} bytes (was 24 bytes)", message.len());
```

### 3. Decode Messages

```rust
// On the receiver side
let decoded = decoder.decode(&message, &context).unwrap();

println!("Received: {} at {}", decoded.value, decoded.timestamp);
```

### 4. Update Context

```rust
// After encoding/decoding, update the context
context.observe(&data);

// This improves future predictions!
```

## Complete Example

Here's a complete working example:

```rust
use alec::{Encoder, Decoder, Context, Classifier, RawData};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup
    let mut encoder = Encoder::new();
    let mut decoder = Decoder::new();
    let classifier = Classifier::default();
    let mut ctx_encoder = Context::new();
    let mut ctx_decoder = Context::new();

    // Simulate sending 10 temperature readings
    for i in 0..10 {
        // Sensor reading (slowly increasing temperature)
        let temp = 20.0 + (i as f64) * 0.1;
        let data = RawData::new(temp, i as u64);

        // Encode
        let classification = classifier.classify(&data, &ctx_encoder);
        let message = encoder.encode(&data, &classification, &ctx_encoder);

        // --- Network transmission would happen here ---

        // Decode
        let decoded = decoder.decode(&message, &ctx_decoder)?;

        // Verify
        assert!((decoded.value - temp).abs() < 0.001);

        // Update contexts
        ctx_encoder.observe(&data);
        ctx_decoder.observe(&decoded);

        println!(
            "Message {}: {} bytes (value: {:.1}°C)",
            i, message.len(), decoded.value
        );
    }

    Ok(())
}
```

## With Checksum Verification

For production use, enable checksums:

```rust
let mut encoder = Encoder::with_checksum();
let mut decoder = Decoder::with_checksum_verification();

// Now messages include CRC32 checksums
let bytes = encoder.encode_to_bytes(&data, &classification, &context);

// Decoder will verify checksum and return error if corrupted
let decoded = decoder.decode_from_bytes(&bytes, &context)?;
```

## What's Next?

- [Basic Concepts](./concepts.md) - Understand how ALEC works
- [Classification Guide](../guide/classification.md) - Configure priorities
- [Context Management](../guide/context.md) - Synchronize contexts
- [Fleet Mode](../advanced/fleet.md) - Manage multiple emitters
