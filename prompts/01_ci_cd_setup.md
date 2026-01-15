# Prompt 01 — Setup CI/CD GitHub Actions

## Contexte

Le projet ALEC a besoin d'une intégration continue pour :
- Valider chaque commit/PR
- Garantir la non-régression
- Automatiser les vérifications qualité

## Objectif

Mettre en place GitHub Actions avec :
1. Build et tests sur chaque push/PR
2. Linting avec Clippy
3. Formatage avec rustfmt
4. Couverture de code (optionnel)

## Étapes

### 1. Créer le workflow principal

Créer `.github/workflows/ci.yml` :

```yaml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-action@stable
        
      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          
      - name: Build
        run: cargo build --verbose
        
      - name: Run tests
        run: cargo test --verbose
        
      - name: Run examples
        run: |
          cargo build --examples
          
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-action@stable
        with:
          components: clippy
          
      - name: Clippy
        run: cargo clippy -- -D warnings
        
  format:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-action@stable
        with:
          components: rustfmt
          
      - name: Check formatting
        run: cargo fmt -- --check
```

### 2. Créer le workflow de release (optionnel)

Créer `.github/workflows/release.yml` :

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-action@stable
      
      - name: Build release
        run: cargo build --release
        
      - name: Run tests
        run: cargo test --release
        
      - name: Create release
        uses: softprops/action-gh-release@v1
        with:
          files: target/release/alec*
          generate_release_notes: true
```

### 3. Ajouter les badges au README

```markdown
# ALEC

[![CI](https://github.com/OWNER/alec-codec/actions/workflows/ci.yml/badge.svg)](https://github.com/OWNER/alec-codec/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
```

### 4. Vérifier localement

```bash
# Simuler ce que fait la CI
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
cargo build --examples
```

### 5. Corriger les warnings existants

Les examples ont des warnings à corriger :
- `examples/simple_sensor.rs` : unused import `Priority`
- `examples/emitter_receiver.rs` : unused variable `pair`

## Livrables

- [ ] `.github/workflows/ci.yml`
- [ ] `.github/workflows/release.yml` (optionnel)
- [ ] Badges dans README.md
- [ ] Warnings corrigés
- [ ] Premier run CI vert

## Critères de succès

```bash
# Tout doit passer
cargo fmt -- --check  # OK
cargo clippy -- -D warnings  # 0 warnings
cargo test  # 44 tests OK
```

## Prochaine étape

→ `02_checksum_implementation.md`
