# ALEC v1.2.0 Release Notes

## no_std Support for Embedded Targets

ALEC v1.2.0 brings full `no_std` support, enabling deployment on bare-metal embedded targets such as ARM Cortex-M microcontrollers running Zephyr RTOS.

### Target Platform

- **MCU**: Nordic nRF9151 (ARM Cortex-M33)
- **RTOS**: Zephyr
- **Rust target**: `thumbv8m.main-none-eabihf`

### Feature Flags

```toml
[dependencies]
alec = { version = "1.2", default-features = false, features = ["no_std"] }
```

| Feature | Description |
|---------|-------------|
| `std` (default) | Full standard library support, includes `thiserror` |
| `no_std` | Bare-metal support with `alloc` |
| `logging` | Optional logging via `log` crate |
| `timestamps` | Optional timestamps via `chrono` |
| `tls` | Optional TLS support via `rustls` |

### What's Available in no_std

**Core modules** (always available):
- `encoder` / `decoder` — Full encode/decode pipeline
- `protocol` — Message types, priorities, wire format
- `context` — Shared dictionary and prediction model
- `classifier` — Priority classification (P1–P5)
- `metrics` — Compression statistics
- `sync` — Context synchronization protocol
- `tls` — TLS/DTLS configuration types
- `error` — Error types with manual `Display` impls

**Std-only modules** (gated behind `#[cfg(feature = "std")]`):
- `channel` — Communication channel abstraction
- `fleet` — Multi-emitter fleet management
- `health` — Health monitoring
- `recovery` — Circuit breaker and retry strategies
- `security` — Rate limiting, audit logging

### Usage Example (no_std)

```rust
#![no_std]
extern crate alloc;

use alec::{Encoder, Decoder, Context, Classifier, RawData};

fn compress_sensor_reading(value: f64, timestamp: u64) -> alloc::vec::Vec<u8> {
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let context = Context::new();

    let data = RawData::new(value, timestamp);
    let classification = classifier.classify(&data, &context);
    let message = encoder.encode(&data, &classification, &context);
    message.to_bytes()
}
```

### FFI (C/C++) Bindings

`alec-ffi` also supports `no_std`:

```toml
[dependencies]
alec-ffi = { version = "1.2", default-features = false, features = ["no_std"] }
```

Core FFI functions (`alec_encode_value`, `alec_decode_value`, `alec_encode_multi`, `alec_decode_multi`) are available in no_std mode. File I/O functions (`alec_encoder_save_context`, `alec_encoder_load_context`, `alec_decoder_load_context`) require the `std` feature.

## Reference Implementation

Live demo on Nordic nRF9151 SMA-DK (NB-IoT):
https://github.com/zeekmartin/alec-nrf9151-demo

### Breaking Changes

None. The default feature set (`std`) preserves full backward compatibility. Existing users are unaffected.

### Dependencies

- `thiserror` 1.0 — now optional, gated behind `std` feature
- `crc` 3.0 — replaces `crc32fast`, `no_std` compatible (`default-features = false`)
- `xxhash-rust` 0.8 — already `no_std` compatible (unchanged)

### Verified Builds

```bash
# no_std (embedded ARM Cortex-M)
cargo check --target thumbv8m.main-none-eabihf --no-default-features --features no_std --lib
# Note: full cargo build requires libc/alloc provider (Zephyr runtime)

# std (existing functionality, all tests passing)
cargo test
cargo build --release
```
