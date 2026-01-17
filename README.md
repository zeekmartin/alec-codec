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

## Quick Start

### Prerequisites

- Rust 1.70+ (encoder and decoder)
- Or: C compiler (embedded encoder only)

### Installation

```bash
# Clone the repo
git clone https://github.com/zeekmartin/alec-codec.git
cd alec-codec

# Build
cargo build --release

# Run tests
cargo test
```

### First Example

```rust
use alec::{Encoder, Decoder, Context, RawData};

fn main() {
    // Create encoder and decoder with shared context
    let mut ctx_emitter = Context::new();
    let mut ctx_receiver = Context::new();
    
    let encoder = Encoder::new();
    let decoder = Decoder::new();
    
    // Simulate measurements
    for i in 0..100 {
        let data = RawData::new(20.0 + (i as f64 * 0.1), i);
        
        // Encode
        let message = encoder.encode(&data, &ctx_emitter);
        ctx_emitter.observe(&data);
        
        // ... transmit message ...
        
        // Decode
        let decoded = decoder.decode(&message, &ctx_receiver).unwrap();
        ctx_receiver.observe(&decoded);
        
        println!("Original: {:.1}, Size: {} bytes", 
                 data.value, message.len());
    }
}
```

➡️ [Complete getting started guide](docs/getting-started.md)

---

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/architecture.md) | Technical overview |
| [Applications](docs/applications.md) | Detailed use cases |
| [Getting Started](docs/getting-started.md) | Getting started guide |
| [Protocol Reference](docs/protocol-reference.md) | Protocol specification |
| [Security](docs/security.md) | Security considerations |
| [API Reference](docs/intra-application.md) | Interfaces and APIs |
| [FAQ](docs/faq.md) | Frequently asked questions |
| [Glossary](docs/glossary.md) | Glossary of terms |

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

- [x] **v0.1** — Functional prototype ✅
- [x] **v0.2** — Evolving context ✅
- [x] **v0.3** — Automatic synchronization ✅
- [x] **v0.4** — Fleet mode ✅
- [x] **v1.0** — Production ready ✅

➡️ [See the complete roadmap](todo.md)

---

## Contributing

Contributions are welcome! See:

- [CONTRIBUTING.md](CONTRIBUTING.md) — Contribution guide
- [prompts/](prompts/) — Templates for features, bugfixes, etc.
- [examples/](examples/) — Example workflows

```bash
# Typical workflow
1. Fork the repo
2. Create a branch: git checkout -b feature/my-feature
3. Follow the appropriate template in prompts/
4. Submit a PR
```

---

## License

ALEC is **dual-licensed**:

### Open Source (AGPL-3.0)

Free for open source projects, research, and personal use.
You must open-source your code if you distribute ALEC or use it in a network service.

```toml
[dependencies]
alec = "1.0"
```

### Commercial License

For proprietary use without open-source obligations.
Starting at $500/year for startups.

👉 **[Get a Commercial License](https://alec-codec.com/pricing)**

See [LICENSE](LICENSE) for details.

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
