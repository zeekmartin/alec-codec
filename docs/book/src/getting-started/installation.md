# Installation

## Requirements

- Rust 1.70 or later
- No external dependencies (pure Rust)

## Adding ALEC to Your Project

Add ALEC to your `Cargo.toml`:

```toml
[dependencies]
alec = "0.1"
```

### Feature Flags

ALEC supports optional features:

```toml
[dependencies]
alec = { version = "0.1", features = ["tls"] }
```

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `std` | Standard library support (default) | None |
| `logging` | Enable logging support | None |
| `timestamps` | Automatic timestamp handling | None |
| `tls` | TLS/DTLS support | `rustls`, `webpki-roots` |
| `full` | All features | All above |

### Minimal Build (no_std)

For embedded systems without standard library:

```toml
[dependencies]
alec = { version = "0.1", default-features = false }
```

## Verifying Installation

Create a simple test:

```rust
use alec::{Encoder, Decoder, Context, Classifier, RawData};

fn main() {
    let mut encoder = Encoder::new();
    let mut decoder = Decoder::new();
    let classifier = Classifier::default();
    let context = Context::new();

    let data = RawData::new(22.5, 0);
    let classification = classifier.classify(&data, &context);
    let message = encoder.encode(&data, &classification, &context);

    println!("ALEC installed successfully!");
    println!("Encoded {} bytes", message.len());
}
```

Run with:

```bash
cargo run
```

## Next Steps

- [Quick Start](./quick-start.md) - Get up and running in 5 minutes
- [Basic Concepts](./concepts.md) - Understand how ALEC works
