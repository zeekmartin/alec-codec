# Prompt 10 — Release v1.0.0

## Contexte

Toutes les fonctionnalités sont implémentées. Il est temps de préparer la release v1.0.0 :
- **Dual licensing** (AGPL-3.0 + Commercial)
- Publication sur crates.io
- Bindings Python (optionnel)
- Images Docker
- Stratégie commerciale

## Objectif

Finaliser et publier ALEC v1.0.0 :
1. Mise en place du dual licensing
2. Checklist de release
3. Publication crates.io
4. Infrastructure de vente
5. Annonce

---

## PARTIE A : Dual Licensing

### Stratégie de monétisation

```
┌─────────────────────────────────────────────────────────────┐
│                    ALEC DUAL LICENSE                        │
├─────────────────────────┬───────────────────────────────────┤
│      AGPL-3.0           │       Commercial License          │
│      (Gratuit)          │          (Payant)                 │
├─────────────────────────┼───────────────────────────────────┤
│ ✓ Usage personnel       │ ✓ Usage propriétaire              │
│ ✓ Recherche/éducation   │ ✓ Firmware closed-source          │
│ ✓ Projets open source   │ ✓ SaaS sans publication           │
│ ✓ Évaluation            │ ✓ Support prioritaire             │
├─────────────────────────┼───────────────────────────────────┤
│ ✗ Produits propriétaires│ Startup (<1M€): 500€/an           │
│ ✗ SaaS sans publier     │ Business (<10M€): 2500€/an        │
│ ✗ Firmware closed       │ Enterprise (>10M€): 10000€/an     │
│                         │ OEM: Sur devis                    │
└─────────────────────────┴───────────────────────────────────┘
```

### Fichiers de licence à créer

1. **`LICENSE`** — Fichier principal expliquant le dual licensing
2. **`LICENSE-AGPL`** — Texte complet AGPL-3.0
3. **`LICENSE-COMMERCIAL.md`** — Template de contrat commercial

### Headers dans le code source

Ajouter ce header à TOUS les fichiers `.rs` :

```rust
// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 [Your Name/Company]
//
// This software is dual-licensed:
//
// 1. AGPL-3.0 for open source use
//    See: https://www.gnu.org/licenses/agpl-3.0.html
//
// 2. Commercial license for proprietary use
//    Contact: licensing@your-domain.com
//
// See LICENSE file for details.
```

Script pour ajouter les headers :

```bash
#!/bin/bash
HEADER='// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 [Your Name/Company]
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.
'

for file in $(find src -name "*.rs"); do
    if ! grep -q "ALEC - Adaptive" "$file"; then
        echo "$HEADER" | cat - "$file" > temp && mv temp "$file"
    fi
done
```

### Vérification des dépendances

**IMPORTANT** : Toutes les dépendances doivent être compatibles AGPL.

```bash
# Installer cargo-license
cargo install cargo-license

# Vérifier les licences
cargo license

# Licences OK pour AGPL : MIT, Apache-2.0, BSD, ISC, Zlib, Unlicense
# Licences problématiques : GPL-2.0-only (incompatible AGPL-3.0)
```

---

## PARTIE B : Infrastructure de vente

### Phase 1 : Simple (recommandé pour démarrer)

**Outils :**
- **LemonSqueezy** ou **Paddle** (gèrent TVA mondiale, pas besoin d'entreprise)
- **Notion** pour tracker les clients
- **Email** pour le support

**Processus :**
```
1. Client visite your-domain.com/pricing
2. Clic "Buy License" → LemonSqueezy checkout
3. Paiement par carte
4. Email automatique avec :
   - Licence PDF signée numériquement
   - Lien de téléchargement (si builds privés)
   - Accès au support
5. Tu reçois notification + paiement
```

**Page pricing (exemple)** :

```markdown
# ALEC Licensing

## Open Source (Free)
- AGPL-3.0 license
- Full source code
- Community support
- [Download from crates.io]

## Commercial License
Use ALEC in proprietary products without open-sourcing your code.

| Plan | For | Price |
|------|-----|-------|
| **Startup** | Companies <€1M revenue | €500/year |
| **Business** | Companies <€10M revenue | €2,500/year |
| **Enterprise** | Larger companies | €10,000/year |
| **OEM** | Embedded in hardware | Contact us |

All plans include:
✓ Proprietary use rights
✓ Email support (48h response)
✓ Updates for 1 year
✓ Invoice for accounting

[Buy Startup] [Buy Business] [Contact for Enterprise]
```

### Phase 2 : Automatisé (quand tu as 10+ clients)

**Ajouter :**
- Portail client (accès aux téléchargements, licences, factures)
- Clés de licence (optionnel, pour tracking)
- Renouvellement automatique

### Génération de licence automatique

Créer un simple service (ou utiliser LemonSqueezy webhooks) :

```python
# Exemple de génération de licence
import hashlib
from datetime import datetime, timedelta

def generate_license(company, tier, email):
    expiry = datetime.now() + timedelta(days=365)
    
    license_text = f"""
ALEC COMMERCIAL LICENSE

Licensee: {company}
Email: {email}
Tier: {tier}
License ID: {hashlib.sha256(f"{company}{email}".encode()).hexdigest()[:16].upper()}
Valid Until: {expiry.strftime('%Y-%m-%d')}

This license grants {company} the right to use ALEC 
in proprietary products per the Commercial License Agreement.

Generated: {datetime.now().isoformat()}
"""
    return license_text
```

---

## PARTIE C : Checklist pré-release

```markdown
## Pre-release Checklist

### Licensing
- [ ] LICENSE file avec dual licensing explanation
- [ ] LICENSE-AGPL avec texte complet AGPL-3.0
- [ ] LICENSE-COMMERCIAL.md template
- [ ] Headers ajoutés à tous les fichiers .rs
- [ ] Dépendances vérifiées (cargo license)
- [ ] Page pricing prête

### Code Quality
- [ ] All tests pass: `cargo test`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Code formatted: `cargo fmt -- --check`
- [ ] No security issues: `cargo audit`
- [ ] Benchmarks acceptable: `cargo bench`

### Documentation
- [ ] README.md avec section licensing
- [ ] CHANGELOG.md updated
- [ ] API documentation complete: `cargo doc`
- [ ] User guide complete: `mdbook build`

### Infrastructure
- [ ] LemonSqueezy/Paddle account setup
- [ ] Payment receiving configured
- [ ] Email templates ready
- [ ] Domain/website live
```

### Préparer Cargo.toml pour publication

```toml
[package]
name = "alec"
version = "1.0.0"
edition = "2021"
rust-version = "1.70"
authors = ["Your Name <you@example.com>"]
description = "Adaptive Lazy Evolving Compression - Smart codec for IoT and constrained environments"
documentation = "https://docs.rs/alec"
homepage = "https://alec-codec.com"
repository = "https://github.com/your-org/alec-codec"
readme = "README.md"
license = "AGPL-3.0"
keywords = ["compression", "iot", "codec", "embedded", "sensor"]
categories = ["compression", "embedded", "encoding"]
exclude = [
    "docs/",
    "prompts/",
    "benches/",
    ".github/",
    "LICENSE-COMMERCIAL.md",
]

[badges]
maintenance = { status = "actively-developed" }
```

**Note :** Sur crates.io, on met `license = "AGPL-3.0"`. La licence commerciale est gérée séparément.

### 3. Mettre à jour README.md

Ajouter une section licensing bien visible :

```markdown
## License

ALEC is **dual-licensed**:

### Open Source (AGPL-3.0)
Free for open source projects, research, and personal use.
You must open-source your code if you distribute ALEC or use it in a network service.

### Commercial License
For proprietary use without open-source obligations.
Starting at €500/year for startups.

👉 **[Get a Commercial License](https://alec-codec.com/pricing)**

See [LICENSE](LICENSE) for details.
```

---

## PARTIE D : Publication

### 1. Publier sur crates.io

```bash
# Vérifier que tout est prêt
cargo publish --dry-run

# Vérifier le package
cargo package --list

# Publier
cargo publish
```

### 2. Créer l'image Docker

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

```bash
# Build
docker build -t alec:1.0.0 .

# Push to registry
docker tag alec:1.0.0 ghcr.io/your-org/alec:1.0.0
docker push ghcr.io/your-org/alec:1.0.0
```

### 3. Bindings Python (optionnel)

Créer `alec-python/` avec PyO3 (voir documentation PyO3).

Publication :
```bash
pip install maturin
maturin build --release
maturin publish  # Sur PyPI
```

### 4. Créer la release GitHub

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

## Licensing

ALEC is dual-licensed:
- **AGPL-3.0** - Free for open source
- **Commercial** - For proprietary use ([pricing](https://alec-codec.com/pricing))

## Installation

### Rust (AGPL-3.0)
\`\`\`toml
[dependencies]
alec = "1.0"
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

- [User Guide](https://alec-codec.com/docs)
- [API Reference](https://docs.rs/alec)
- [Commercial Licensing](https://alec-codec.com/pricing)

---

**Full Changelog**: https://github.com/your-org/alec-codec/compare/v0.1.0...v1.0.0
```

---

## PARTIE E : Lancement commercial

### Annonce (adapter selon la plateforme)

**LinkedIn/Twitter :**
```
🚀 ALEC v1.0 is here!

Smart compression codec for IoT that achieves 90% size reduction 
for sensor data.

✅ Delta encoding with adaptive context
✅ Priority classification (P1-P5)
✅ Fleet management for 1000s of sensors
✅ Production-ready security

Open source (AGPL) or commercial license.

github.com/your-org/alec-codec
```

**Hacker News :**
```
Show HN: ALEC – Adaptive compression codec for IoT (90% reduction)
```

**Reddit (r/rust, r/embedded, r/IOT) :**
```
[Show] ALEC: Adaptive Lazy Evolving Compression for IoT

Built a compression codec optimized for sensor data. 
Instead of generic compression, it learns from your data patterns.

Key features:
- Delta encoding with shared context
- 5 priority levels for classification
- Fleet mode for multi-device scenarios
- Dual licensed (AGPL + Commercial)

Looking for feedback, especially from embedded devs!
```

### Tracking des premiers clients

Créer un simple tracker (Notion, spreadsheet) :

| Date | Company | Contact | Tier | Status | Revenue |
|------|---------|---------|------|--------|---------|
| 2025-02-01 | Acme IoT | john@acme.io | Startup | Signed | €500 |

---

## Livrables

- [ ] Fichiers LICENSE, LICENSE-AGPL, LICENSE-COMMERCIAL.md
- [ ] Headers de licence dans tous les .rs
- [ ] README.md avec section licensing
- [ ] Page pricing sur le site
- [ ] Compte LemonSqueezy/Paddle configuré
- [ ] Publication crates.io
- [ ] Image Docker publiée
- [ ] Release GitHub avec notes
- [ ] Posts d'annonce préparés

## Critères de succès

```bash
cargo publish  # Succès
# Site web live avec pricing
# Premier email de licensing configuré
```

## 🎉 Félicitations !

ALEC v1.0.0 est publié avec un modèle économique viable !

**Prochaines étapes suggérées :**
- Répondre aux premiers utilisateurs
- Collecter les retours et témoignages
- Optimiser la page pricing (A/B testing)
- Écrire des articles/tutoriels (SEO)
- Premier client → case study
