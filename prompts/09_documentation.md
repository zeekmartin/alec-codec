# Prompt 09 — Documentation (v1.0.0)

## Contexte

Pour une version production, ALEC a besoin d'une documentation complète :
- Guide de déploiement
- API reference
- Troubleshooting guide
- Exemples détaillés

## Objectif

Créer une documentation professionnelle :
1. Documentation Rust (rustdoc)
2. Guide utilisateur (mdBook)
3. API reference
4. Troubleshooting guide

## Étapes

### 1. Enrichir la documentation inline

Chaque fonction publique doit avoir :
- Description courte
- Description longue si nécessaire
- `# Arguments` si applicable
- `# Returns` si applicable
- `# Errors` si applicable
- `# Examples`
- `# Panics` si applicable

Exemple pour `encoder.rs` :

```rust
impl Encoder {
    /// Encode data with classification into a compact message.
    ///
    /// This method selects the optimal encoding strategy based on the
    /// context's predictions and the data's characteristics.
    ///
    /// # Arguments
    ///
    /// * `data` - The raw data to encode
    /// * `classification` - Priority classification for the data
    /// * `context` - Shared context for predictions and patterns
    ///
    /// # Returns
    ///
    /// An `EncodedMessage` containing the compressed data.
    ///
    /// # Examples
    ///
    /// ```
    /// use alec::{Encoder, Classifier, Context, RawData};
    ///
    /// let mut encoder = Encoder::new();
    /// let classifier = Classifier::default();
    /// let context = Context::new();
    ///
    /// let data = RawData::new(22.5, 0);
    /// let classification = classifier.classify(&data, &context);
    /// let message = encoder.encode(&data, &classification, &context);
    ///
    /// assert!(message.len() < 24); // Smaller than raw data
    /// ```
    pub fn encode(
        &mut self,
        data: &RawData,
        classification: &Classification,
        context: &Context,
    ) -> EncodedMessage {
        // ...
    }
}
```

### 2. Créer la structure mdBook

```bash
# Installer mdBook
cargo install mdbook

# Créer la structure
mkdir -p docs/book/src
```

Créer `docs/book/book.toml` :

```toml
[book]
authors = ["ALEC Team"]
language = "en"
multilingual = false
src = "src"
title = "ALEC User Guide"

[build]
build-dir = "../output"

[output.html]
default-theme = "light"
preferred-dark-theme = "navy"
git-repository-url = "https://github.com/your-org/alec-codec"
```

### 3. Écrire les chapitres du guide

`docs/book/src/SUMMARY.md` :

```markdown
# Summary

[Introduction](./introduction.md)

# Getting Started

- [Installation](./getting-started/installation.md)
- [Quick Start](./getting-started/quick-start.md)
- [Basic Concepts](./getting-started/concepts.md)

# User Guide

- [Encoding Data](./guide/encoding.md)
- [Decoding Messages](./guide/decoding.md)
- [Classification](./guide/classification.md)
- [Context Management](./guide/context.md)
- [Synchronization](./guide/synchronization.md)

# Advanced Topics

- [Fleet Mode](./advanced/fleet.md)
- [Security](./advanced/security.md)
- [Performance Tuning](./advanced/performance.md)
- [Custom Channels](./advanced/channels.md)

# Deployment

- [Production Checklist](./deployment/checklist.md)
- [Configuration Reference](./deployment/configuration.md)
- [Monitoring](./deployment/monitoring.md)

# Reference

- [API Documentation](./reference/api.md)
- [Protocol Specification](./reference/protocol.md)
- [Error Codes](./reference/errors.md)

# Appendix

- [Troubleshooting](./appendix/troubleshooting.md)
- [FAQ](./appendix/faq.md)
- [Changelog](./appendix/changelog.md)
```

### 4. Écrire le guide de démarrage

`docs/book/src/getting-started/quick-start.md` :

```markdown
# Quick Start

This guide will get you up and running with ALEC in 5 minutes.

## Installation

Add ALEC to your `Cargo.toml`:

\`\`\`toml
[dependencies]
alec = "0.1"
\`\`\`

## Basic Usage

### 1. Create Components

\`\`\`rust
use alec::{Encoder, Decoder, Context, Classifier, RawData};

fn main() {
    // Create encoder (on the emitter side)
    let mut encoder = Encoder::new();
    
    // Create decoder (on the receiver side)
    let mut decoder = Decoder::new();
    
    // Create classifier (determines message priority)
    let classifier = Classifier::default();
    
    // Create shared context (must be synchronized)
    let mut context = Context::new();
}
\`\`\`

### 2. Encode Data

\`\`\`rust
// Your sensor data
let temperature = 22.5;
let timestamp = 1234567890;

// Wrap in RawData
let data = RawData::new(temperature, timestamp);

// Classify (determines priority P1-P5)
let classification = classifier.classify(&data, &context);

// Encode
let message = encoder.encode(&data, &classification, &context);

// Message is now ready to transmit!
println!("Encoded {} bytes (was 24 bytes)", message.len());
\`\`\`

### 3. Decode Messages

\`\`\`rust
// On the receiver side
let decoded = decoder.decode(&message, &context)?;

println!("Received: {} at {}", decoded.value, decoded.timestamp);
\`\`\`

### 4. Update Context

\`\`\`rust
// After encoding/decoding, update the context
context.observe(&data);

// This improves future predictions!
\`\`\`

## What's Next?

- [Basic Concepts](./concepts.md) - Understand how ALEC works
- [Classification Guide](../guide/classification.md) - Configure priorities
- [Context Management](../guide/context.md) - Synchronize contexts
```

### 5. Écrire le troubleshooting guide

`docs/book/src/appendix/troubleshooting.md` :

```markdown
# Troubleshooting

## Common Issues

### Decoder returns "Context mismatch" error

**Symptom:** `DecodeError::ContextMismatch { expected: X, actual: Y }`

**Cause:** The encoder and decoder contexts are out of sync.

**Solutions:**
1. Ensure both sides call `context.observe()` after each message
2. Implement context synchronization (see [Synchronization](../guide/synchronization.md))
3. For testing, share the same context instance

### Messages are larger than expected

**Symptom:** Encoded messages are close to raw size (24 bytes)

**Cause:** Context doesn't have enough data to predict well.

**Solutions:**
1. Allow warmup period (10-100 messages)
2. Check that you're calling `context.observe()`
3. Verify your data has predictable patterns

### "Unknown encoding type" error

**Symptom:** `DecodeError::UnknownEncodingType(X)`

**Cause:** Protocol version mismatch or corrupted message.

**Solutions:**
1. Verify encoder and decoder use same ALEC version
2. Enable checksum verification: `Decoder::with_checksum_verification()`
3. Check for network corruption

### High memory usage

**Symptom:** Context memory grows unbounded

**Cause:** Too many patterns being stored.

**Solutions:**
1. Configure `EvolutionConfig::max_patterns`
2. Call `context.evolve()` periodically
3. Use smaller `max_age` for pattern pruning

## Performance Issues

### Encoding is slow

**Checklist:**
- [ ] Build with `--release`
- [ ] Avoid creating new Encoder per message
- [ ] Reuse Context instance

**Expected performance:**
- Debug: ~10k msg/s
- Release: ~100k+ msg/s

### Fleet mode is slow

**Checklist:**
- [ ] Limit `max_emitters` if you have many
- [ ] Increase `cleanup_interval`
- [ ] Use appropriate `cross_fleet_threshold`

## Getting Help

If you can't resolve your issue:

1. Check [FAQ](./faq.md)
2. Search [GitHub Issues](https://github.com/your-org/alec-codec/issues)
3. Open a new issue with:
   - ALEC version
   - Rust version
   - Minimal reproduction code
   - Expected vs actual behavior
```

### 6. Générer la documentation

```bash
# Rustdoc
cargo doc --no-deps --open

# mdBook
cd docs/book
mdbook build
mdbook serve  # For preview
```

### 7. Ajouter au CI

Dans `.github/workflows/ci.yml` :

```yaml
  docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-action@stable
      
      - name: Build rustdoc
        run: cargo doc --no-deps
        
      - name: Install mdBook
        run: cargo install mdbook
        
      - name: Build book
        run: |
          cd docs/book
          mdbook build
          
      - name: Deploy to GitHub Pages
        if: github.ref == 'refs/heads/main'
        uses: peaceiris/actions-gh-pages@v3
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./docs/book/output
```

## Livrables

- [ ] Documentation inline complète (rustdoc)
- [ ] Structure mdBook
- [ ] Guide de démarrage
- [ ] Guide de déploiement
- [ ] Troubleshooting guide
- [ ] FAQ
- [ ] CI pour documentation

## Critères de succès

```bash
cargo doc --no-deps  # Pas de warnings
cargo test --doc  # Doc tests passent
cd docs/book && mdbook build  # Build OK
```

## Prochaine étape

→ `10_release.md` (v1.0.0 - Release)
