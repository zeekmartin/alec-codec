# Prompt 10 — Release v1.0.0

## Contexte

Toutes les fonctionnalités sont implémentées. Il est temps de préparer la release v1.0.0 :
- Publication sur crates.io
- Bindings Python (optionnel)
- Images Docker
- Release notes

## Objectif

Finaliser et publier ALEC v1.0.0 :
1. Checklist de release
2. Publication crates.io
3. Bindings PyO3 (optionnel)
4. Docker images
5. Annonce

## Étapes

### 1. Checklist pré-release

```markdown
## Pre-release Checklist

### Code Quality
- [ ] All tests pass: `cargo test`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Code formatted: `cargo fmt -- --check`
- [ ] No security issues: `cargo audit`
- [ ] Benchmarks acceptable: `cargo bench`

### Documentation
- [ ] README.md up to date
- [ ] CHANGELOG.md updated
- [ ] API documentation complete: `cargo doc`
- [ ] User guide complete: `mdbook build`
- [ ] Examples all work: `cargo build --examples`

### Versioning
- [ ] Version bumped in Cargo.toml
- [ ] Version tag created: `git tag v1.0.0`
- [ ] CHANGELOG reflects all changes

### Legal
- [ ] LICENSE file present
- [ ] All dependencies have compatible licenses
- [ ] Copyright headers if required

### Metadata
- [ ] Cargo.toml has all required fields
- [ ] Repository URL correct
- [ ] Keywords and categories set
```

### 2. Préparer Cargo.toml pour publication

```toml
[package]
name = "alec"
version = "1.0.0"
edition = "2021"
rust-version = "1.70"
authors = ["Your Name <you@example.com>"]
description = "Adaptive Lazy Evolving Compression - Smart codec for IoT and constrained environments"
documentation = "https://docs.rs/alec"
homepage = "https://github.com/your-org/alec-codec"
repository = "https://github.com/your-org/alec-codec"
readme = "README.md"
license = "MIT OR Apache-2.0"
keywords = ["compression", "iot", "codec", "embedded", "sensor"]
categories = ["compression", "embedded", "encoding"]
exclude = [
    "docs/",
    "prompts/",
    "benches/",
    ".github/",
]

[badges]
maintenance = { status = "actively-developed" }
```

### 3. Mettre à jour CHANGELOG.md

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2025-XX-XX

### Added
- Encoder with multiple encoding strategies (raw, delta, repeated, multi)
- Decoder with automatic strategy detection
- Classifier with 5 priority levels (P1-P5)
- Evolving context with pattern learning
- Automatic context synchronization
- Fleet management for multi-emitter scenarios
- Security module (rate limiting, audit logging, TLS support)
- Health monitoring and circuit breaker
- Comprehensive documentation

### Changed
- N/A (initial stable release)

### Deprecated
- N/A

### Removed
- N/A

### Fixed
- N/A

### Security
- Rate limiting to prevent DoS
- Audit logging for compliance
- TLS/DTLS support for encryption

## [0.1.0] - 2025-01-15

### Added
- Initial prototype
- Basic encoder/decoder
- Simple classifier
- Static context
```

### 4. Publier sur crates.io

```bash
# Vérifier que tout est prêt
cargo publish --dry-run

# Vérifier le package
cargo package --list

# Publier
cargo publish
```

### 5. Créer les bindings Python (optionnel)

Créer `alec-python/` :

```bash
mkdir alec-python
cd alec-python
cargo init --lib
```

`alec-python/Cargo.toml` :

```toml
[package]
name = "alec-python"
version = "1.0.0"
edition = "2021"

[lib]
name = "alec"
crate-type = ["cdylib"]

[dependencies]
alec = { path = "../", version = "1.0" }
pyo3 = { version = "0.20", features = ["extension-module"] }
```

`alec-python/src/lib.rs` :

```rust
use pyo3::prelude::*;
use alec::{Encoder as RustEncoder, Decoder as RustDecoder, 
           Context as RustContext, Classifier as RustClassifier,
           RawData as RustRawData};

#[pyclass]
struct Encoder {
    inner: RustEncoder,
}

#[pymethods]
impl Encoder {
    #[new]
    fn new() -> Self {
        Self { inner: RustEncoder::new() }
    }
    
    fn encode(&mut self, value: f64, timestamp: u64, 
              classifier: &Classifier, context: &Context) -> Vec<u8> {
        let data = RustRawData::new(value, timestamp);
        let classification = classifier.inner.classify(&data, &context.inner);
        self.inner.encode(&data, &classification, &context.inner).to_bytes()
    }
}

#[pyclass]
struct Decoder {
    inner: RustDecoder,
}

#[pymethods]
impl Decoder {
    #[new]
    fn new() -> Self {
        Self { inner: RustDecoder::new() }
    }
    
    fn decode(&mut self, bytes: Vec<u8>, context: &Context) -> PyResult<(f64, u64)> {
        let message = alec::EncodedMessage::from_bytes(&bytes)
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid message"))?;
        let decoded = self.inner.decode(&message, &context.inner)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        Ok((decoded.value, decoded.timestamp))
    }
}

#[pyclass]
struct Context {
    inner: RustContext,
}

#[pymethods]
impl Context {
    #[new]
    fn new() -> Self {
        Self { inner: RustContext::new() }
    }
    
    fn observe(&mut self, value: f64, timestamp: u64) {
        let data = RustRawData::new(value, timestamp);
        self.inner.observe(&data);
    }
}

#[pyclass]
struct Classifier {
    inner: RustClassifier,
}

#[pymethods]
impl Classifier {
    #[new]
    fn new() -> Self {
        Self { inner: RustClassifier::default() }
    }
}

#[pymodule]
fn alec(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Encoder>()?;
    m.add_class::<Decoder>()?;
    m.add_class::<Context>()?;
    m.add_class::<Classifier>()?;
    Ok(())
}
```

Publier sur PyPI :

```bash
pip install maturin
maturin build --release
maturin publish
```

### 6. Créer l'image Docker

`Dockerfile` :

```dockerfile
# Build stage
FROM rust:1.75 as builder

WORKDIR /usr/src/alec
COPY . .

RUN cargo build --release --features full

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/alec/target/release/alec /usr/local/bin/

EXPOSE 8080

CMD ["alec"]
```

`docker-compose.yml` :

```yaml
version: '3.8'

services:
  alec-receiver:
    build: .
    ports:
      - "8080:8080"
    environment:
      - ALEC_LOG=info
      - ALEC_TLS=true
    volumes:
      - ./config:/etc/alec
      - alec-data:/var/lib/alec

volumes:
  alec-data:
```

```bash
# Build
docker build -t alec:1.0.0 .

# Push to registry
docker tag alec:1.0.0 ghcr.io/your-org/alec:1.0.0
docker push ghcr.io/your-org/alec:1.0.0
```

### 7. Créer la release GitHub

```bash
# Créer le tag
git tag -a v1.0.0 -m "Release v1.0.0"
git push origin v1.0.0
```

Release notes template :

```markdown
# ALEC v1.0.0

We're excited to announce the first stable release of ALEC!

## Highlights

- 🚀 **Production Ready** - Battle-tested compression codec
- 📊 **Up to 90% compression** for sensor data
- 🔒 **Security built-in** - TLS, mTLS, audit logging
- 🏭 **Fleet support** - Manage thousands of emitters
- 📚 **Comprehensive docs** - Guides, API reference, examples

## Installation

### Rust
\`\`\`toml
[dependencies]
alec = "1.0"
\`\`\`

### Python
\`\`\`bash
pip install alec
\`\`\`

### Docker
\`\`\`bash
docker pull ghcr.io/your-org/alec:1.0.0
\`\`\`

## Quick Start

\`\`\`rust
use alec::{Encoder, Decoder, Context, Classifier, RawData};

let mut encoder = Encoder::new();
let mut context = Context::new();
let classifier = Classifier::default();

let data = RawData::new(22.5, timestamp);
let classification = classifier.classify(&data, &context);
let message = encoder.encode(&data, &classification, &context);
// 24 bytes → ~4 bytes!
\`\`\`

## Documentation

- [User Guide](https://your-org.github.io/alec-codec/)
- [API Reference](https://docs.rs/alec)
- [Examples](https://github.com/your-org/alec-codec/tree/main/examples)

## What's Changed

See [CHANGELOG.md](CHANGELOG.md) for full details.

## Contributors

Thanks to everyone who contributed to this release!

---

**Full Changelog**: https://github.com/your-org/alec-codec/compare/v0.1.0...v1.0.0
```

## Livrables

- [ ] Checklist pré-release complétée
- [ ] Cargo.toml finalisé
- [ ] CHANGELOG.md à jour
- [ ] Publication crates.io
- [ ] Bindings Python (optionnel)
- [ ] Image Docker
- [ ] Release GitHub avec notes
- [ ] Annonce (blog, Twitter, Reddit)

## Critères de succès

```bash
cargo publish  # Succès
pip install alec  # Fonctionne (si Python)
docker run ghcr.io/your-org/alec:1.0.0  # Démarre
```

## 🎉 Félicitations !

ALEC v1.0.0 est publié ! Prochaines étapes suggérées :
- Surveiller les issues
- Collecter les retours utilisateurs
- Planifier v1.1.0 (nouvelles features)
- Écrire des articles/tutoriels
