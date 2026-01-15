# Decoding Messages

This guide covers decoding ALEC messages.

## Basic Decoding

```rust
use alec::{Decoder, Context};

let mut decoder = Decoder::new();
let context = Context::new();

let decoded = decoder.decode(&message, &context)?;
println!("Value: {}, Timestamp: {}", decoded.value, decoded.timestamp);
```

## With Checksum Verification

Enable checksum verification for production:

```rust
let mut decoder = Decoder::with_checksum_verification();

// Decoder will return error if checksum doesn't match
match decoder.decode_from_bytes(&bytes, &context) {
    Ok(data) => println!("Valid: {}", data.value),
    Err(e) => println!("Corrupted message: {}", e),
}
```

## Multi-Value Decoding

Decode messages with multiple values:

```rust
let values = decoder.decode_multi(&message, &context)?;
for (name_id, value) in values {
    println!("{}: {}", name_id, value);
}
```

## Error Handling

Common decode errors:

```rust
use alec::error::DecodeError;

match decoder.decode(&message, &context) {
    Ok(data) => { /* success */ }
    Err(DecodeError::BufferTooShort { .. }) => {
        // Message truncated
    }
    Err(DecodeError::ChecksumMismatch { .. }) => {
        // Corrupted data
    }
    Err(DecodeError::UnknownEncodingType(_)) => {
        // Protocol version mismatch
    }
    Err(e) => {
        // Other error
        println!("Decode error: {}", e);
    }
}
```

## Sequence Tracking

Track message ordering:

```rust
let seq = decoder.last_sequence();
let expected = decoder.expected_sequence();

if seq != expected {
    println!("Gap detected: expected {}, got {}", expected, seq);
}
```

## Best Practices

1. **Enable checksums**: Use `Decoder::with_checksum_verification()`
2. **Handle errors**: Don't unwrap decode results
3. **Update context**: Call `context.observe()` after decoding
4. **Monitor sequences**: Detect message loss
