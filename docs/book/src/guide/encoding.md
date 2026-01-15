# Encoding Data

This guide covers everything you need to know about encoding data with ALEC.

## Basic Encoding

```rust
use alec::{Encoder, Classifier, Context, RawData};

let mut encoder = Encoder::new();
let classifier = Classifier::default();
let context = Context::new();

let data = RawData::new(22.5, timestamp);
let classification = classifier.classify(&data, &context);
let message = encoder.encode(&data, &classification, &context);
```

## Encoding Strategies

ALEC automatically selects the best strategy:

### Delta Encoding

Used when the value is close to the prediction:

```rust
// If predicted = 22.0 and actual = 22.1
// Delta = 0.1 → encoded as 1-4 bytes
```

### Repeated Encoding

Used when the value hasn't changed:

```rust
// If previous = 22.0 and current = 22.0
// Encoding: "same" → 1 byte
```

### Raw Encoding

Fallback for unpredictable values:

```rust
// Full 8-byte value + header
```

## With Checksum

For production, enable checksums:

```rust
let mut encoder = Encoder::with_checksum();
let bytes = encoder.encode_to_bytes(&data, &classification, &context);
// bytes now include CRC32 checksum
```

## With Metrics

Track compression performance:

```rust
use alec::CompressionMetrics;

let mut metrics = CompressionMetrics::new();
let message = encoder.encode_with_metrics(&data, &classification, &context, &mut metrics);

println!("Compression ratio: {:.1}%", metrics.compression_ratio() * 100.0);
```

## Multi-Value Encoding

Encode multiple values in one message:

```rust
use alec::protocol::Priority;

let values = vec![
    (0u16, 22.5),  // (name_id, value)
    (1, 55.0),
    (2, 1013.25),
];

let message = encoder.encode_multi(&values, source_id, timestamp, Priority::P3Normal, &context);
```

## Best Practices

1. **Reuse components**: Create encoder once, use many times
2. **Update context**: Always call `context.observe()` after encoding
3. **Enable checksums**: Use `Encoder::with_checksum()` in production
4. **Monitor metrics**: Track compression ratio to detect issues
