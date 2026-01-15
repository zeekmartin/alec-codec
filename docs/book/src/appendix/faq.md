# Frequently Asked Questions

## General

### What does ALEC stand for?

**A**daptive **L**azy **E**volving **C**ompression

- **Adaptive**: Automatically selects the best encoding strategy
- **Lazy**: Only compresses when beneficial
- **Evolving**: Learns patterns over time

### Is ALEC suitable for my use case?

ALEC is designed for:
- IoT sensor data (temperature, humidity, pressure, etc.)
- Small messages (8-64 bytes typical)
- Continuous streaming data
- Constrained memory environments

ALEC is NOT designed for:
- Large files or bulk data (use gzip, lz4, etc.)
- Random/encrypted data (no patterns to learn)
- One-time transmissions (no warmup time)

### What compression ratios can I expect?

Typical compression ratios:

| Data Pattern | Compression |
|--------------|-------------|
| Constant values | 90%+ (1 byte) |
| Slow changes | 60-80% |
| Periodic patterns | 40-60% |
| Random data | 0% (raw encoding) |

## Technical

### Do I need to synchronize contexts?

**Yes**, if encoder and decoder run on different machines.

Options:
1. **Manual**: Ensure both sides call `observe()` identically
2. **Automatic**: Use the `Synchronizer` for robust sync

### Can I use ALEC without std?

**Yes**, ALEC supports `no_std`:

```toml
[dependencies]
alec = { version = "0.1", default-features = false }
```

Note: Some features (like `TlsConfig`) require std.

### Is ALEC thread-safe?

- `Encoder` and `Decoder` are **not** thread-safe (use one per thread)
- `Context` is **not** thread-safe (wrap in `Mutex` if shared)
- `Classifier` is thread-safe (stateless)

### How much memory does ALEC use?

Default memory usage:

| Component | Memory |
|-----------|--------|
| Context | ~64 KB max |
| Encoder | <1 KB |
| Decoder | <1 KB |
| FleetManager | ~200 bytes per emitter |

### Can I use custom encoding strategies?

Currently, ALEC uses built-in strategies. Custom strategies would require modifying the library.

## Performance

### What throughput can I expect?

In release mode on modern hardware:

| Operation | Throughput |
|-----------|------------|
| Encoding | >100k msg/s |
| Decoding | >100k msg/s |
| Roundtrip | >50k msg/s |
| Fleet | >10k msg/s |

### Why is my performance low?

Common causes:
1. **Debug build**: Always benchmark with `--release`
2. **Creating new instances**: Reuse Encoder/Context
3. **Excessive cloning**: Pass by reference when possible

### How do I benchmark ALEC?

Run the stress tests:

```bash
cargo test --release stress -- --ignored
```

## Security

### Is ALEC encrypted?

**No**, ALEC is a compression codec, not encryption.

For encryption, use TLS/DTLS at the transport layer:

```toml
[dependencies]
alec = { version = "0.1", features = ["tls"] }
```

### Can attackers learn my data patterns?

Like any compression, message sizes may leak information about data patterns. For sensitive data:
1. Use encryption (TLS)
2. Consider padding messages to fixed sizes

### How does rate limiting work?

ALEC uses token bucket rate limiting:

```rust
let limiter = RateLimiter::new(100.0, 50);  // 100/sec, burst of 50

if limiter.check(emitter_id, timestamp) {
    // Allow
} else {
    // Rate limited
}
```

## Fleet Mode

### When should I use fleet mode?

Use fleet mode when:
- Managing multiple emitters (>10)
- Need cross-emitter anomaly detection
- Want automatic stale emitter cleanup

### How do I detect anomalous emitters?

```rust
let anomalies = fleet.anomalous_emitters();
for (id, score) in anomalies {
    println!("Emitter {} has z-score {}", id, score);
}
```

### What is cross-fleet anomaly detection?

It compares each emitter's values to the fleet-wide statistics:
- Calculates z-score for each emitter
- Flags emitters significantly different from the fleet mean
- Helps detect malfunctioning sensors

## Compatibility

### What Rust version is required?

Rust 1.70 or later (stable).

### Does ALEC work with async/await?

The core library is synchronous. For async usage:
- Wrap in `spawn_blocking` for Tokio
- Use in separate thread for async-std

### Can I use ALEC from other languages?

Not currently. C bindings could be added via `cbindgen` if there's demand.

## Troubleshooting

See the [Troubleshooting Guide](./troubleshooting.md) for common issues.
