# Suggested README Updates for alec-codec

> **Destination**: `alec-codec` repository → Merge into existing `README.md`

These sections should be added to or updated in the main README.md.

---

## Section: C/C++ Bindings (Add after Rust usage section)

```markdown
## C/C++ Bindings

ALEC provides C-compatible bindings for integration with C/C++ projects and embedded systems.

### Building

```bash
cargo build --release -p alec-ffi
```

### Quick Example

```c
#include "alec.h"

AlecEncoder* enc = alec_encoder_new();

double values[] = {22.5, 23.0, 22.8};
uint8_t buffer[256];
size_t len;

alec_encode_multi(enc, values, 3, 0, NULL, buffer, sizeof(buffer), &len);
printf("Compressed %zu values to %zu bytes\n", 3, len);

alec_encoder_free(enc);
```

See [docs/FFI.md](docs/FFI.md) for complete documentation.
```

---

## Section: Preload Files (Add new section)

```markdown
## Preload Files

Skip the learning phase with pre-trained context files:

```rust
use alec::Context;

// Load a preload for temperature sensors
let context = Context::load_from_file("temperature.alec-context")?;

// Compression is immediately optimal
let mut encoder = Encoder::new();
let compressed = encoder.encode(&data, &context);
```

Available preloads:
- `temperature.alec-context` - Temperature sensors (typical HVAC/environment)
- `humidity.alec-context` - Humidity sensors
- `counter.alec-context` - Monotonic counters

See [contexts/demo/](contexts/demo/) for examples.
```

---

## Section: Cross-Compilation (Add to Installation or new section)

```markdown
## Cross-Compilation

### ARM Cortex-M (Embedded)

```bash
rustup target add thumbv7em-none-eabihf
cargo build --release --target thumbv7em-none-eabihf
```

### ESP32

```bash
# Requires esp-rs toolchain
cargo build --release --target xtensa-esp32-none-elf
```

### Memory Requirements

| Component | Flash | RAM |
|-----------|-------|-----|
| Core codec | ~20 KB | ~2 KB |
| With FFI | ~25 KB | ~3 KB |
| Per context | - | ~1-2 KB |
```

---

## Section: Project Structure (Update existing or add)

```markdown
## Project Structure

```
alec-codec/
├── src/              # Core Rust library
├── alec-ffi/         # C/C++ bindings
│   ├── src/lib.rs    # FFI implementation
│   ├── include/      # C headers
│   └── examples/     # C examples
├── examples/         # Rust examples
├── tests/            # Integration tests
├── contexts/         # Preload files
│   └── demo/         # Demo preloads
└── docs/             # Documentation
```
```

---

## Section: Badges (Add to top of README)

```markdown
[![Crates.io](https://img.shields.io/crates/v/alec.svg)](https://crates.io/crates/alec)
[![Documentation](https://docs.rs/alec/badge.svg)](https://docs.rs/alec)
[![License](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)
```

---

## Section: Links (Add to bottom)

```markdown
## Links

- [Documentation](https://alec-codec.com/docs)
- [API Reference](https://docs.rs/alec)
- [C/C++ Bindings](docs/FFI.md)
- [GitHub Issues](https://github.com/zeekmartin/alec-codec/issues)
- [Commercial Licensing](https://alec-codec.com/licensing)
```

---

## Notes

1. The existing README should already have basic usage - these additions complement it
2. Keep examples minimal in README, link to docs for details
3. The FFI section is important for discoverability
4. Preloads are a key differentiator - highlight them
