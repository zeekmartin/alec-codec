# Synchronization

Keep encoder and decoder contexts in sync.

## Why Synchronization?

Contexts must be identical for successful decoding. If they diverge:
- Decode errors occur
- Compression efficiency drops

## Manual Synchronization

Simple approach for reliable networks:

```rust
// Encoder side
let message = encoder.encode(&data, &classification, &context);
context.observe(&data);
transmit(&message);

// Decoder side
let decoded = decoder.decode(&message, &context)?;
context.observe(&decoded);  // Mirror encoder's observe
```

## Automatic Synchronization

For unreliable networks, use the `Synchronizer`:

```rust
use alec::sync::{Synchronizer, SyncConfig};

let config = SyncConfig {
    announce_interval: 100,  // Announce every 100 messages
    max_version_gap: 10,     // Request sync if >10 versions behind
    auto_sync: true,
    ..Default::default()
};

let mut synchronizer = Synchronizer::with_config(config);
```

## Sync Protocol

1. **Announce**: Periodically broadcast context state
2. **Request**: Request sync when versions diverge
3. **Diff**: Send only changed patterns

```rust
// Check if announcement needed
if let Some(announce) = synchronizer.should_announce(&context) {
    send_control_message(announce);
}

// Process received announcement
if let Some(request) = synchronizer.process_announce(&announce, &context) {
    send_control_message(request);
}

// Process sync diff
synchronizer.apply_diff(&diff, &mut context)?;
```

## Handling Divergence

When contexts diverge too much:

```rust
// Full resync
let full_context = sender_context.export_full();
receiver_context.import_full(&full_context)?;
```
