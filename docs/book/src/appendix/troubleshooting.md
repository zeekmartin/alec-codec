# Troubleshooting

## Common Issues

### Decoder returns "Context mismatch" error

**Symptom:** `DecodeError::ContextMismatch { expected: X, actual: Y }`

**Cause:** The encoder and decoder contexts are out of sync.

**Solutions:**
1. Ensure both sides call `context.observe()` after each message
2. Implement context synchronization (see [Synchronization](../guide/synchronization.md))
3. For testing, share the same context instance

```rust
// Wrong: contexts will diverge
context_encoder.observe(&data);
// ... send message ...
// decoder doesn't observe!

// Correct: both sides observe
context_encoder.observe(&data);
// ... send message ...
context_decoder.observe(&decoded);
```

### Messages are larger than expected

**Symptom:** Encoded messages are close to raw size (24 bytes)

**Cause:** Context doesn't have enough data to predict well.

**Solutions:**
1. Allow warmup period (10-100 messages)
2. Check that you're calling `context.observe()`
3. Verify your data has predictable patterns

```rust
// Check compression after warmup
for i in 0..100 {
    let data = RawData::new(sensor_value(), i as u64);
    let msg = encoder.encode(&data, &class, &context);
    context.observe(&data);

    if i > 50 {
        println!("Compression: {} bytes", msg.len());
        // Should be smaller than 24 bytes
    }
}
```

### "Unknown encoding type" error

**Symptom:** `DecodeError::UnknownEncodingType(X)`

**Cause:** Protocol version mismatch or corrupted message.

**Solutions:**
1. Verify encoder and decoder use same ALEC version
2. Enable checksum verification:
   ```rust
   let mut decoder = Decoder::with_checksum_verification();
   ```
3. Check for network corruption

### Checksum verification failed

**Symptom:** `DecodeError::ChecksumMismatch { expected, actual }`

**Cause:** Message was corrupted during transmission.

**Solutions:**
1. Check network reliability
2. Implement retry logic:
   ```rust
   use alec::{with_retry, RetryStrategy};

   let strategy = RetryStrategy::exponential(3, Duration::from_millis(100));
   let result = with_retry(&strategy, || {
       decoder.decode(&message, &context)
   });
   ```
3. Check for buffer overflows in transmission code

### High memory usage

**Symptom:** Context memory grows unbounded

**Cause:** Too many patterns being stored.

**Solutions:**
1. Configure evolution to prune old patterns:
   ```rust
   use alec::context::EvolutionConfig;

   let context = Context::with_evolution(EvolutionConfig {
       min_frequency: 2,     // Remove rarely used patterns
       max_age: 10000,       // Remove old patterns
       evolution_interval: 100,
       ..Default::default()
   });
   ```
2. Call `context.evolve()` periodically
3. Monitor with health checks:
   ```rust
   use alec::HealthCheckable;

   let check = context.health_check();
   println!("Memory: {} bytes", check.message);
   ```

### Sequence number overflow

**Symptom:** Sequence resets to 0 unexpectedly

**Cause:** Normal behavior - sequence wraps at u32::MAX

**Solution:** This is expected. Track with:
```rust
let seq = decoder.last_sequence();
// Handle wraparound in your logic
```

## Performance Issues

### Encoding is slow

**Checklist:**
- [ ] Build with `--release` (critical!)
- [ ] Avoid creating new Encoder per message
- [ ] Reuse Context instance
- [ ] Avoid unnecessary cloning

**Expected performance:**
- Debug: ~10k msg/s
- Release: ~100k+ msg/s

```bash
# Always benchmark in release mode
cargo test --release stress -- --ignored
```

### Fleet mode is slow

**Checklist:**
- [ ] Limit `max_emitters` if you have many
- [ ] Increase `cleanup_interval`
- [ ] Use appropriate `cross_fleet_threshold`

```rust
use alec::fleet::FleetConfig;

let config = FleetConfig {
    max_emitters: 1000,        // Limit if needed
    cleanup_interval: 3600,    // Less frequent cleanup
    cross_fleet_threshold: 3.0, // Less sensitive
    ..Default::default()
};
```

### High CPU usage

**Cause:** Evolution running too frequently

**Solution:** Increase evolution interval:
```rust
let config = EvolutionConfig {
    evolution_interval: 1000,  // Every 1000 messages instead of 100
    ..Default::default()
};
```

## Circuit Breaker Issues

### Circuit breaker stays open

**Symptom:** `CircuitState::Open` for extended time

**Cause:** Too many failures without recovery

**Solutions:**
1. Check the underlying cause of failures
2. Adjust thresholds:
   ```rust
   let config = CircuitConfig {
       failure_threshold: 10,  // More tolerant
       recovery_timeout: Duration::from_secs(10),  // Faster recovery
       ..Default::default()
   };
   ```
3. Reset manually if needed:
   ```rust
   circuit_breaker.reset();
   ```

## Getting Help

If you can't resolve your issue:

1. Check [FAQ](./faq.md)
2. Search [GitHub Issues](https://github.com/your-org/alec-codec/issues)
3. Open a new issue with:
   - ALEC version (`alec::VERSION`)
   - Rust version (`rustc --version`)
   - Minimal reproduction code
   - Expected vs actual behavior
   - Relevant error messages
