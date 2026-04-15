# ALEC — Adaptive Lazy Evolving Compression

<p align="center">
  <img src="docs/assets/alec-logo.svg" alt="ALEC Logo" width="200"/>
</p>

<p align="center">
  <a href="https://github.com/zeekmartin/alec-codec/actions/workflows/ci.yml"><img src="https://github.com/zeekmartin/alec-codec/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-AGPL--3.0-blue.svg" alt="License"></a>
  <a href="https://crates.io/crates/alec"><img src="https://img.shields.io/crates/v/alec.svg" alt="Crates.io"></a>
</p>

<p align="center">
  <strong>A smart compression codec for bandwidth-constrained environments</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#use-cases">Use Cases</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#embedded--nostd">Embedded / no_std</a> •
  <a href="#documentation">Documentation</a> •
  <a href="#contributing">Contributing</a>
</p>

---

## Why ALEC?

In many environments, **every bit counts**:
- 🛰️ Satellite communications at a few kbps
- 🌿 Battery-powered IoT sensors lasting years
- 🌍 Rural areas with limited satellite connectivity
- 🌊 Underwater acoustic links
- 🏭 Industrial networks with restricted bandwidth

ALEC addresses these challenges with an innovative approach: **transmit only what has value**.

---

## Features

### 🦥 Lazy Compression

ALEC doesn't transmit all data — it first sends **the decision**, then details only if needed.

```
Without ALEC:  [Complete data] ──────────────────────▶ 1000 bytes
With ALEC:     [Alert: anomaly detected] ────────────▶ 12 bytes
               [Details on demand] ──────────────────▶ 500 bytes (if requested)
```

### 🔄 Evolving Context

Encoder and decoder build a **shared dictionary** that improves over time.

```
Week 1:  "temperature=22.3°C" ──────────────────────▶ 20 bytes
Week 4:  [code_7][+0.3] ───────────────────────────▶ 3 bytes
```

### ⚖️ Smart Asymmetry

Computational effort is placed **where resources exist**.

| Mode | Encoder | Decoder | Use Case |
|------|---------|---------|----------|
| Standard | Light | Heavy | IoT sensors, drones |
| Reversed | Heavy | Light | Broadcast distribution |

### 📊 Priority Classification

Each data point receives a priority that determines its handling:

| Priority | Behavior | Example |
|----------|----------|---------|
| P1 CRITICAL | Immediate send + acknowledgment | Fire alert |
| P2 IMPORTANT | Immediate send | Anomaly detected |
| P3 NORMAL | Standard send | Periodic measurement |
| P4 DEFERRED | On demand only | Detailed history |
| P5 DISPOSABLE | Never sent | Debug logs |

---

## Use Cases

### 🚜 Connected Agriculture

Field sensors monitor moisture, temperature, and nutrients. With ALEC, they run 10 years on battery by transmitting only alerts and anomalies.

### 🏥 Rural Telemedicine

A portable ultrasound in a remote area first sends "suspected cardiac anomaly" in 50 bytes. The remote doctor decides if they need the full image.

### 🚛 Vehicle Fleets

500 trucks report their position. After a few weeks, the system knows the usual routes and only transmits deviations.

### 🛰️ Space Observation

A satellite photographs Earth. It only sends significant changes compared to previous images.

➡️ [See all detailed use cases](docs/applications.md)

---

## Ecosystem

ALEC consists of multiple crates:

| Crate | Description | Features |
|-------|-------------|----------|
| `alec` | Core compression codec | Encoder, Decoder, Context |
| `alec-ffi` | C/C++ bindings | FFI interface, embedded targets |
| `alec-gateway` | Multi-sensor orchestration | Channel management, Frame aggregation |
| `alec-gateway[metrics]` | Entropy observability | TC, H_joint, Resilience R |
| `alec-complexity` | Anomaly detection | Baseline, Z-scores, Events |

### Quick Install

```toml
# Core codec only
[dependencies]
alec = "1.2"

# C FFI (std)
[dependencies]
alec-ffi = "1.2"

# C FFI for embedded (bare-metal, no RTOS)
[dependencies]
alec-ffi = { version = "1.2", default-features = false, features = ["bare-metal"] }

# C FFI for Zephyr RTOS
[dependencies]
alec-ffi = { version = "1.2", default-features = false, features = ["zephyr"] }
```

---

## Quick Start

### Prerequisites

- Rust 1.70+ (encoder and decoder)
- Or: C compiler (embedded encoder only via `alec-ffi`)

### Installation

```bash
git clone https://github.com/zeekmartin/alec-codec.git
cd alec-codec
cargo build --release
cargo test
```

### First Example

```rust
use alec::{Encoder, Decoder, Context, RawData};

fn main() {
    let mut ctx_emitter = Context::new();
    let mut ctx_receiver = Context::new();

    let encoder = Encoder::new();
    let decoder = Decoder::new();

    for i in 0..100 {
        let data = RawData::new(20.0 + (i as f64 * 0.1), i);

        // Encode
        let message = encoder.encode(&data, &ctx_emitter);
        ctx_emitter.observe(&data);

        // Decode
        let decoded = decoder.decode(&message, &ctx_receiver).unwrap();
        ctx_receiver.observe(&decoded);

        println!("Original: {:.1}, Size: {} bytes", data.value, message.len());
    }
}
```

➡️ [Complete getting started guide](docs/getting-started.md)

---

## Embedded / no_std

ALEC supports embedded targets from version 1.2.0. The `alec-ffi` crate provides C bindings with three feature tiers:

### Feature comparison

| Feature | Allocator | Panic handler | Target |
|---------|-----------|---------------|--------|
| `std` (default) | System | System | Linux, macOS, Windows |
| `no_std` | User-provided | User-provided | Any embedded |
| `bare-metal` | `embedded-alloc` (8KB heap) | `loop {}` | Bare-metal (no RTOS) |
| `zephyr` | Zephyr `k_malloc`/`k_free` | `loop {}` | Zephyr RTOS |

### Bare-metal (no RTOS)

```toml
alec-ffi = { version = "1.2", default-features = false, features = ["bare-metal"] }
```

```bash
rustup target add thumbv8m.main-none-eabihf
cargo build --release --target thumbv8m.main-none-eabihf --no-default-features --features bare-metal
```

### Zephyr RTOS

```toml
alec-ffi = { version = "1.2", default-features = false, features = ["zephyr"] }
```

```bash
rustup target add thumbv8m.main-none-eabi
cargo build --release --target thumbv8m.main-none-eabi --no-default-features --features zephyr
```

> **Note on target selection for Zephyr:** Use `thumbv8m.main-none-eabi` (not `eabihf`). Zephyr's nRF91 toolchain compiles in `nofp` mode — using the `hf` variant causes an ABI mismatch at link time.

### CMakeLists.txt integration (Zephyr)

```cmake
add_library(alec_ffi STATIC IMPORTED GLOBAL)
set_target_properties(alec_ffi PROPERTIES
    IMPORTED_LOCATION ${CMAKE_CURRENT_SOURCE_DIR}/libalec_ffi.a
)
target_include_directories(alec_ffi INTERFACE
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)
# Do NOT use --whole-archive — causes premature static initialisation before Zephyr heap is ready
```

Add a `critical_section.c` to your Zephyr app:

```c
#include <zephyr/kernel.h>

static unsigned int cs_irq_key;

void _critical_section_1_0_acquire(void) { cs_irq_key = irq_lock(); }
void _critical_section_1_0_release(void) { irq_unlock(cs_irq_key); }
```

### Validated embedded platforms

| Platform | SoC | Feature | Status |
|----------|-----|---------|--------|
| Nordic nRF9151 SMA-DK | Cortex-M33 | `zephyr` | ✅ Validated |
| Generic Cortex-M33 | thumbv8m | `bare-metal` | ✅ Builds |

➡️ [See the full NB-IoT demo](https://github.com/zeekmartin/alec-nrf9151-demo)

---

## Documentation

### Core Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/ARCHITECTURE.md) | System design and ADRs |
| [Getting Started](docs/getting-started.md) | Getting started guide |
| [Protocol Reference](docs/protocol-reference.md) | Protocol specification |
| [Security](docs/security.md) | Security considerations |
| [FAQ](docs/FAQ.md) | Frequently asked questions |

### Module Documentation

| Document | Description |
|----------|-------------|
| [Gateway Guide](docs/GATEWAY.md) | Multi-sensor orchestration |
| [Metrics Guide](docs/METRICS.md) | Entropy and resilience computation |
| [Complexity Guide](docs/COMPLEXITY.md) | Baseline learning and anomaly detection |
| [Configuration](docs/CONFIGURATION.md) | Complete configuration reference |
| [Integration](docs/INTEGRATION.md) | Integration patterns |

---

## Performance

Results on reference dataset (temperature sensor, 24h, 1 measurement/min):

| Metric | Without context | After warm-up | Target |
|--------|-----------------|---------------|--------|
| Compression ratio | 0.65 | 0.08 | < 0.10 ✅ |
| P1 Latency | 45ms | 42ms | < 100ms ✅ |
| Encoder RAM | 12KB | 28KB | < 64KB ✅ |

---

## Roadmap

- [x] **v1.0** — Production ready ✅
- [x] **v1.2.0** — no_std support ✅
- [x] **v1.2.1** — bare-metal embedded (Cortex-M) ✅
- [x] **v1.2.3** — Zephyr RTOS support ✅
- [ ] **v1.3** — RIOT OS support
- [ ] **v1.4** — FreeRTOS support

➡️ [See the complete roadmap](todo.md)

---

## Contributing

Contributions are welcome! See:

- [CONTRIBUTING.md](CONTRIBUTING.md) — Contribution guide
- [prompts/](prompts/) — Templates for features, bugfixes, etc.
- [examples/](examples/) — Example workflows

---

## License

ALEC is **dual-licensed**:

### Open Source (AGPL-3.0)

Free for open source projects, research, and personal use.

```toml
[dependencies]
alec = "1.2"
```

### Commercial License

For proprietary use without open-source obligations.
Starting at $500/year for startups.

👉 **[Get a Commercial License](https://alec-codec.com/pricing)**

See [LICENSE](LICENSE) for details.

---

## Compact fixed-channel mode (v1.3.5+)

For constrained LoRaWAN deployments with hard payload
ceilings (e.g. 11 bytes), ALEC now supports a compact
fixed-channel wire format:

- 4-byte header (sequence u16 + context_version u16)
- 2-byte bitmap (2 bits per channel, up to 64 channels)
- No name_ids, no device timestamp
- Marker byte: 0xA1 (data) / 0xA2 (keyframe)
- Steady-state avg: ~8B for 5 channels
- Fits in 11-byte LoRaWAN payload ceiling

### Packet loss recovery

ALEC is a stateful differential codec. To handle
lossy networks:

- Periodic keyframe every N transmissions (default 50)
  forces Raw32 for all channels — automatic recovery
- Sequence gap detection triggers context reset
- LoRaWAN downlink smart resync: send 0xFF command
  to request immediate keyframe, reducing worst-case
  drift from N×interval to 1×interval

### Context persistence

For server-side decoder persistence across restarts:

- `alec_decoder_export_state()` → ALCS binary format
  (~1.5 KB for 5 channels)
- `alec_decoder_import_state()` → restore decoder
- Bit-exact round-trip verified
- CRC32 checksum, format-versioned (ALCS v1)

### Multi-architecture support

Verified bare-metal builds:

| Target                  | Use case    | .text (ALEC) |
|-------------------------|-------------|-------------:|
| `thumbv7m-none-eabi`    | Cortex-M3   | ~55 KB       |
| `thumbv7em-none-eabi`   | Cortex-M4   | ~55 KB       |
| `thumbv7em-none-eabihf` | Cortex-M4F  | ~56 KB       |
| `thumbv6m-none-eabi`    | Cortex-M0+  | ~53 KB       |

Note: final linked size with `--gc-sections` will be
significantly smaller.

---

## Acknowledgments

ALEC draws inspiration from:
- NASA error-correcting codes (turbo codes, LDPC)
- Dictionary compression (LZ77, LZ78)
- Efficient IoT protocols (CoAP, MQTT-SN)

---

<p align="center">
  <sub>Made with ❤️ for a world where every bit counts</sub>
</p>
