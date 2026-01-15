# Prompt 10 — Release v1.0.0 (Adapté)

## Contexte

Préparation de la release v1.0.0 d'ALEC. Les fichiers de licence existent déjà.

**Reporté à v2 :**
- Docker
- Bindings Python
- Dashboard

---

## État actuel (à vérifier)

### ✅ Complété
- v0.1.0 Prototype fonctionnel
- v0.2.0 Contexte évolutif
- v0.3.0 Sync automatique (partiel)
- v0.4.0 Mode flotte (partiel)
- Sécurité (TLS, mTLS, audit, rate limiting)
- Robustesse (CircuitBreaker, RetryStrategy)
- Documentation mdBook

### ⚠️ À vérifier / compléter
- [ ] Headers de licence dans tous les .rs
- [ ] Cargo.toml prêt pour crates.io
- [ ] README.md avec section licensing
- [ ] CHANGELOG.md à jour
- [ ] Licences des dépendances compatibles AGPL

### 🔴 Non fait (backlog v1.x)
- Canal bidirectionnel (MQTT/CoAP wrapper)
- Scheduling dans classifier
- Dataset de test `temp_sensor_24h`
- Optimisation mémoire émetteur
- Benchmarks sur hardware cible

---

## PARTIE A : Vérification de l'existant

### 1. Vérifier les fichiers de licence

```bash
# Vérifier que les fichiers existent
ls -la LICENSE LICENSE-AGPL LICENSE-COMMERCIAL.md

# Vérifier le contenu du LICENSE principal
head -50 LICENSE
```

### 2. Vérifier les dépendances

```bash
# Installer cargo-license si pas déjà fait
cargo install cargo-license

# Lister les licences de toutes les dépendances
cargo license

# Licences OK : MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unlicense, CC0-1.0
# Licences problématiques : GPL-2.0-only (incompatible AGPL-3.0)
```

### 3. Vérifier la compilation et les tests

```bash
cargo build --release
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

---

## PARTIE B : Headers de licence

### Header à ajouter

```rust
// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 Simon Music
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

```

### Script pour ajouter les headers

Créer `scripts/add_headers.sh` :

```bash
#!/bin/bash

HEADER='// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 Simon Music
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

'

# Trouver tous les fichiers .rs dans src/
for file in $(find src -name "*.rs"); do
    # Vérifier si le header existe déjà
    if ! grep -q "ALEC - Adaptive" "$file"; then
        echo "Adding header to $file"
        # Créer fichier temporaire avec header + contenu original
        echo "$HEADER" | cat - "$file" > temp && mv temp "$file"
    else
        echo "Header already exists in $file"
    fi
done

echo "Done!"
```

Exécuter :
```bash
chmod +x scripts/add_headers.sh
./scripts/add_headers.sh
```

---

## PARTIE C : Préparer Cargo.toml

Vérifier/mettre à jour `Cargo.toml` :

```toml
[package]
name = "alec"
version = "1.0.0"
edition = "2021"
rust-version = "1.70"
authors = ["Simon Music <contact@alec-codec.com>"]
description = "Adaptive Lazy Evolving Compression - Smart codec for IoT sensor data with 90% compression ratio"
documentation = "https://docs.rs/alec"
homepage = "https://alec-codec.com"
repository = "https://github.com/zeekmartin/alec-codec"
readme = "README.md"
license = "AGPL-3.0"
keywords = ["compression", "iot", "codec", "embedded", "sensor"]
categories = ["compression", "embedded", "encoding"]

# Exclure les fichiers non nécessaires pour crates.io
exclude = [
    "docs/",
    "prompts/",
    ".github/",
    "scripts/",
    "LICENSE-COMMERCIAL.md",
    "*.prompt.md",
]

[badges]
maintenance = { status = "actively-developed" }
```

---

## PARTIE D : Mettre à jour README.md

Ajouter/vérifier la section licensing dans README.md :

```markdown
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
```

---

## PARTIE E : Publication

### 1. Dry run

```bash
# Vérifier ce qui sera publié
cargo publish --dry-run

# Lister les fichiers du package
cargo package --list
```

### 2. Créer le tag Git

```bash
git add -A
git commit -m "chore: prepare v1.0.0 release"
git tag -a v1.0.0 -m "Release v1.0.0 - Production ready"
git push origin main --tags
```

### 3. Publier sur crates.io

```bash
# S'assurer d'être connecté
cargo login

# Publier
cargo publish
```

### 4. Créer la release GitHub

Aller sur GitHub → Releases → "Draft a new release"
- Tag: v1.0.0
- Title: ALEC v1.0.0 - Production Ready
- Coller les release notes ci-dessous

---

## PARTIE F : Release Notes

```markdown
# ALEC v1.0.0 🚀

First stable release of ALEC - Adaptive Lazy Evolving Compression for IoT.

## Highlights

- 📊 **90% compression** for sensor data
- 🎯 **5 priority levels** (P1-P5) for intelligent data routing
- 🔄 **Auto-sync contexts** between encoder/decoder
- 🏭 **Fleet management** for thousands of emitters
- 🔒 **Security built-in** - TLS, mTLS, audit logging, rate limiting
- 💪 **Resilience** - Circuit breaker, retry strategies, graceful degradation
- 📚 **Comprehensive docs** - User guide, API reference, examples

## Installation

### Rust (crates.io)

```toml
[dependencies]
alec = "1.0"
```

### From source

```bash
git clone https://github.com/zeekmartin/alec-codec
cd alec-codec
cargo build --release
```

## Quick Start

```rust
use alec::{Encoder, Decoder, Context, Priority};

fn main() {
    let mut ctx = Context::new();
    let encoder = Encoder::new(&mut ctx);
    
    // Encode sensor reading
    let msg = encoder.encode(23.5, Priority::Auto);
    println!("Compressed: {} bytes", msg.len());
    
    // Decode
    let decoder = Decoder::new(&mut ctx);
    let value = decoder.decode(&msg).unwrap();
}
```

## Licensing

ALEC is dual-licensed:

- **AGPL-3.0** - Free for open source projects
- **Commercial** - For proprietary use ([pricing](https://alec-codec.com/pricing))

## Documentation

- 📖 [User Guide](https://alec-codec.com/docs)
- 📚 [API Reference](https://docs.rs/alec)
- 💰 [Commercial Licensing](https://alec-codec.com/pricing)

## What's Next (v1.x)

- MQTT/CoAP transport wrappers
- Performance optimizations for embedded
- Python bindings
- Docker images

---

**Full Changelog**: https://github.com/zeekmartin/alec-codec/commits/v1.0.0
```

---

## Checklist finale

```markdown
## Pre-release Checklist

### Licensing
- [ ] LICENSE, LICENSE-AGPL, LICENSE-COMMERCIAL.md présents
- [ ] Headers ajoutés à tous les fichiers src/*.rs
- [ ] Dépendances vérifiées avec cargo-license
- [ ] README.md avec section licensing

### Code Quality
- [ ] `cargo test` - tous les tests passent
- [ ] `cargo clippy -- -D warnings` - pas de warnings
- [ ] `cargo fmt -- --check` - code formatté

### Cargo.toml
- [ ] version = "1.0.0"
- [ ] description, homepage, repository remplis
- [ ] license = "AGPL-3.0"
- [ ] keywords et categories définis
- [ ] exclude configuré

### Publication
- [ ] `cargo publish --dry-run` réussit
- [ ] Tag v1.0.0 créé et pushé
- [ ] Release GitHub créée
- [ ] `cargo publish` exécuté

### Post-release
- [ ] Vérifier https://crates.io/crates/alec
- [ ] Vérifier https://docs.rs/alec
- [ ] Annoncer (LinkedIn, Reddit r/rust, HN)
```

---

## Backlog v1.x / v2.0

### v1.1.0 - Transport
- [ ] MQTT wrapper (SyncChannel)
- [ ] CoAP wrapper

### v1.2.0 - Performance
- [ ] Optimisation mémoire émetteur
- [ ] Benchmarks hardware (ARM, ESP32)
- [ ] Version no_std

### v2.0.0 - Écosystème
- [ ] Docker images
- [ ] Python bindings (PyO3)
- [ ] Dashboard visualisation
- [ ] Intégration Grafana
