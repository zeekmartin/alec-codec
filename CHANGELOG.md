# Changelog

Toutes les modifications notables de ce projet seront documentées dans ce fichier.

Le format est basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.0.0/),
et ce projet adhère au [Semantic Versioning](https://semver.org/lang/fr/).

---

## [1.3.1] - 2026-03-10

### Fixed
- Timestamp stored as Unix seconds (÷1000) instead of truncated milliseconds — fixes silent wrap every 49 days
- `encode_raw()` now uses `context.version()` instead of hardcoded 0

### Changed
- `MessageHeader::sequence` reduced from u32 to u16 (-2B per frame)
- `MessageHeader::context_version` serialized as u24 (-1B per frame)
- `MessageHeader::SIZE` reduced from 13 to 10
- `name_id` serialized as u8 instead of u16 in multi-channel frame
  (-1B per channel, -5B on 5-channel payload)
- `ChannelInput.name_id` field type changed from u16 to u8
- Total header + frame saving vs 1.3.0: up to 8B on 5-channel demo

---

## [1.3.0] - 2026-03-09

### Added

- **`encode_multi_adaptive()` — adaptive per-channel compression with shared
  header** (`encoder.rs`): replaces the naive `encode_multi()` which used Raw32
  for every channel. The new method applies the full encoding decision tree
  (Repeated → Delta8 → Delta16 → Delta32 → Raw32 → Raw64) independently per
  channel, with per-channel context isolation via `source_id`. A single 13-byte
  header is shared across all channels, amortising the overhead that dominated
  single-value messages.

- **Priority-based channel inclusion**: each channel is classified (P1–P5) by
  the existing `Classifier`. P1–P3 channels are always included. P4 channels
  are included only if the frame stays under 127 bytes (BLE ATT_MTU). P5
  channels are excluded from the wire frame but their context is still updated
  for future predictions.

- **`ChannelInput` struct** (`protocol.rs`): input type for multi-channel
  encoding with `name_id`, `source_id`, and `value` fields.

- **Updated `decode_multi()`** (`decoder.rs`): now handles all per-channel
  encoding types (Delta8, Delta16, Repeated, etc.) instead of only Raw32.
  Uses `name_id` as per-channel `source_id` for context-dependent decoding.

### Changed

- **`alec_encode_multi()` FFI signature** (`alec-ffi`): breaking change.
  Now accepts `f64` values (was `f32`), per-channel `timestamps`, per-channel
  `source_ids`, and per-channel `priorities` parameters (all nullable).
  Updated C header `alec.h` to match.

- `alec` version bumped to 1.3.0
- `alec-ffi` version bumped to 1.3.0

---

## [1.2.5] - 2026-03-09

### Fixed

- **`alec-ffi` `alec_encode_value()` source_id hashing**: the `source_id`
  C string parameter was ignored (`_source_id`), and `RawData::new()` was
  called with `source_id=0` unconditionally. This pooled all channels
  (temperature, pressure, humidity, etc.) into a single context slot,
  making EMA predictions meaningless and preventing adaptive compression.
  The parameter is now hashed via `xxh64()` to a `u32` and passed to
  `RawData::with_source()` for per-channel context isolation. NULL
  source_id defaults to `0` (backward-compatible).

- **`alec-ffi` `hash_source_id()` varint overhead**: the initial fix
  truncated the full xxh64 to u32, producing values like `0xd84dd889`
  that encode as 5-byte varints in the message payload — adding 4 bytes
  of pure overhead per message vs a 1-byte varint. Now maps to the range
  1–127 via `(xxh64(bytes, 0) % 127 + 1) as u32`, guaranteeing a 1-byte
  varint. NULL source_id stays 0.

- **`EncodedMessage::encoding_type()` varint misparse** (`protocol.rs`):
  hardcoded `payload[1]` as the encoding byte, assuming source_id is
  always a 1-byte varint (`< 128`). Any source_id >= 128 caused the
  method to read a varint continuation byte as the encoding type,
  returning `None` or a wrong variant. Now properly decodes the varint
  to find the encoding byte position.

### Changed

- `alec-ffi` version bumped to 1.2.5
- `alec-ffi` version string updated to "1.2.5"
- Added `xxhash-rust` dependency to `alec-ffi` (`no_std`-compatible)

---

## [1.2.4] - 2026-03-09

### Fixed

- **`alec-ffi` ZephyrAllocator alignment**: replace `k_malloc` with
  `k_aligned_alloc` to respect Rust's layout alignment requirements.
  `k_malloc` returns 4-byte aligned memory on ARM; types such as
  `Vec<(u16, f64)>` and `BTreeMap` nodes require 8-byte alignment,
  causing misaligned access (UB) on Cortex-M33 and
  `ALEC_ERR_NULL_POINTER` (rc=5) on the first call to
  `alec_encode_multi()`. Validated on Nordic nRF9151 / Zephyr RTOS.

- **`alec-ffi` `alec_encode_multi()` type mismatch**: change `values`
  parameter from `*const f64` to `*const f32` to match the C header
  declaration (`const float*`). The previous f64 signature caused Rust
  to read 8 bytes per value from a buffer of 4-byte floats, reading
  past allocated memory (UB) on Cortex-M33 and returning
  `ALEC_ERR_NULL_POINTER` (rc=5). The f32 values are now widened to
  f64 inside the FFI shim before being passed to the encoder.

### Changed

- `alec` and `alec-ffi` version bumped to 1.2.4
- `alec-ffi` version string updated to "1.2.4"

---

## [1.2.3] - 2026-03-08

### Fixed
- **`alec-ffi` zephyr panic handler**: replaced standalone `#[panic_handler]` with one that delegates to Zephyr's `k_panic()` C function, eliminating duplicate symbol linker errors when linking with Zephyr RTOS

### Changed
- `alec` and `alec-ffi` version bumped to 1.2.3
- `alec-ffi` version string updated to "1.2.3"

---

## [1.2.2] - 2026-03-08

### Added
- **`zephyr` feature for `alec-ffi`**: Zephyr RTOS support without conflicting with Zephyr's own panic handler and heap
  - Global allocator backed by Zephyr `k_malloc`/`k_free` (extern C FFI)
  - No `embedded-alloc` dependency — uses Zephyr's native heap management
  - `alec_heap_init()` is a no-op (Zephyr manages its own heap)
  - Panic handler provided for Rust compiler satisfaction; Zephyr handles panics at C level
  - Usage: `alec-ffi = { version = "1.2.2", default-features = false, features = ["zephyr"] }`

### Changed
- `alec` and `alec-ffi` version bumped to 1.2.2
- `alec-ffi` version string updated to "1.2.2"

---

## [1.2.1] - 2026-03-08

### Added
- **`bare-metal` feature for `alec-ffi`**: Provides `#[global_allocator]` (via `embedded-alloc` `LlffHeap`) and `#[panic_handler]` for bare-metal embedded targets
  - New FFI function `alec_heap_init()` — must be called before any heap allocation
  - Heap size: 8192 bytes (sufficient for ALEC context + encode buffer)
  - Dependencies: `embedded-alloc` 0.6 (llff), `cortex-m` 0.7 (critical-section-single-core)
  - Usage: `alec-ffi = { version = "1.2.1", default-features = false, features = ["bare-metal"] }`

### Changed
- `alec` and `alec-ffi` version bumped to 1.2.1
- `alec-ffi` version string updated to "1.2.1"

---

## [1.2.0] - 2026-03-08

### Added
- **`no_std` support** for bare-metal embedded targets (ARM Cortex-M, RISC-V, etc.)
  - `#![no_std]` with `extern crate alloc` when `std` feature is disabled
  - Feature flags: `default = ["std"]`, `std = ["thiserror"]`, `no_std = []`
  - Verified on `thumbv8m.main-none-eabihf` (Nordic nRF9151 / Zephyr RTOS)
- **`alec-ffi` no_std support** with feature passthrough (`std`/`no_std`)
  - File I/O functions gated behind `std` feature
  - Core FFI functions (encode/decode) available in no_std

### Changed
- `thiserror` dependency now optional, gated behind `std` feature
  - Manual `Display` and `From` impls provided for `no_std`
- `crc32fast` replaced with `crc` v3.0 (no_std compatible, `default-features = false`)
- `std::collections::HashMap` replaced with `alloc::collections::BTreeMap` in no_std mode
  - `EncodingType` now derives `Ord` and `PartialOrd` for BTreeMap compatibility
- `std::fmt` → `core::fmt`, `std::result` → `core::result` across all core modules
- Std-only modules gated behind `#[cfg(feature = "std")]`:
  - `channel`, `fleet`, `health`, `recovery`, `security`
- Context `save_to_file`/`load_from_file` and `HealthCheckable` impl gated behind `std`
- `alec-ffi` version string updated to "1.2.0"

---

## [Unreleased]

### Added

#### ALEC Demo Infrastructure (v0.1.0)
- **Sensor Simulator Service** (`demo/simulator/`):
  - Real-time correlated sensor data generation using latent variables
  - 15 agricultural IoT sensors (temperature, humidity, soil, wind, gas, etc.)
  - Latent variable model: weather, daily_cycle, seasonal, gusts, irrigation
  - Prometheus metrics endpoint (`/metrics`)
  - JSON API for readings (`/readings`, `/sensors`, `/status`)
  - Docker container with health checks
- **Injection Service** (`demo/injection/`):
  - FastAPI-based anomaly injection for testing
  - Injection types: noise, spike, drift, dropout
  - Per-sensor injection state management
  - Auto-expiration for timed injections
  - RESTful API: `POST /inject/{sensor}/{type}`, `DELETE /inject/{sensor}`, `POST /reset`
- **Grafana Dashboard** (`demo/grafana/`):
  - Pre-provisioned ALEC Demo dashboard
  - Panels: Cluster Status, Sensor Time Series, Entropy Gauge, Complexity Gauge
  - Robustness Indicator with HEALTHY/ATTENTION/CRITICAL zones
  - Per-Sensor Entropy breakdown
  - Correlation Heatmap visualization
  - Anomaly Detection alerts
- **Docker Compose Stack** (`demo/docker-compose.yml`):
  - Full orchestration: simulator, gateway, complexity, injection, prometheus, grafana
  - Service dependencies with health checks
  - Named volumes for persistence
  - Bridge network for service discovery
- **Prometheus Configuration** (`demo/prometheus/`):
  - Scrape configs for all ALEC services
  - 5-second scrape interval for real-time monitoring
- **Documentation** (`demo/README.md`):
  - Architecture diagram
  - Quick start guide
  - API reference for all services
  - Metrics reference table
  - Troubleshooting guide

#### ALEC Grafana Monitoring Stack (v0.1.0)
- **ALEC Exporter** (`alec-grafana/exporter/`):
  - Prometheus exporter for ALEC metrics
  - Real-time compression stats, entropy, complexity
  - Health check endpoint
- **Demo Script** (`alec-grafana/scripts/demo.sh`):
  - One-command stack management (start/stop/status/logs/clean)
  - Prerequisite checking
  - Service health waiting with timeout
  - Colored terminal output

#### ALEC Testdata Crate (v0.1.0)
- **Industry Dataset Generator** (`alec-testdata/`):
  - 6 industry profiles: Agriculture, HVAC, Energy, Logistics, Healthcare, Manufacturing
  - 24 test scenarios with realistic patterns
  - Configurable anomaly injection
  - Parquet and CSV output formats
  - Compression benchmark utility

#### ALEC Gateway (v0.1.0-alpha)
- **Multi-channel management**: Handle dozens of sensor channels
- **Priority-based aggregation**: Numeric priority (0 = highest)
- **Frame packing**: Optimize for LoRaWAN/MQTT payload limits
- **Preload support**: Load pre-trained contexts per channel
- **LoRaWAN presets**: Built-in configurations for DR0-DR5

#### ALEC Metrics (Gateway feature: `metrics`)
- **Signal entropy**: Per-channel (H_i) and joint (H_joint) entropy
- **Total Correlation (TC)**: Redundancy measure across channels
- **Payload entropy**: Compressed frame randomness (H_bytes)
- **Resilience Index (R)**: Normalized redundancy (0-1)
- **Criticality ranking**: Leave-one-out ΔR for sensor importance
- **Zone classification**: healthy / attention / critical
- **Configurable alignment**: Sample-and-hold, nearest, linear interpolation
- **Sliding window**: Time-based or sample-count-based

#### ALEC Complexity (v0.1.0-alpha)
- **Baseline learning**: Statistical summary of nominal operation
- **Delta/Z-score computation**: Deviation from baseline with smoothing
- **S-lite structure analysis**: Lightweight pairwise channel dependency graph
- **Anomaly event detection**: With persistence and cooldown
  - PayloadEntropySpike
  - StructureBreak
  - RedundancyDrop
  - ComplexitySurge
  - SensorCriticalityShift
- **GenericInput adapter**: JSON-based input for standalone usage
- **GatewayInput adapter**: Direct MetricsSnapshot consumption (feature-gated)
- **Baseline update modes**: Frozen, EMA, Rolling

#### Documentation
- `docs/ARCHITECTURE.md`: System design and ADRs
- `docs/GATEWAY.md`: Gateway module documentation
- `docs/METRICS.md`: Metrics module documentation
- `docs/COMPLEXITY.md`: Complexity module documentation
- `docs/CONFIGURATION.md`: Complete configuration reference
- `docs/JSON_SCHEMAS.md`: Snapshot JSON schemas
- `docs/INTEGRATION.md`: Integration patterns
- `docs/FAQ.md`: Frequently asked questions (English)
- `docs/diagrams/`: Mermaid architecture and data flow diagrams
- `alec-gateway/README.md`: Crate-specific documentation
- `alec-complexity/README.md`: Crate-specific documentation

### Changed
- Workspace now includes `alec-gateway` and `alec-complexity` crates

---

## [0.2.0-alpha] - 2025-01-16

### Added
- **Système de Preload** : Sauvegarde et chargement de contextes pré-entraînés
  - `Context::save_to_file()` - Export du contexte entraîné vers fichier binaire
  - `Context::load_from_file()` - Import de fichier preload
  - Vérification de version entre encodeur/décodeur
  - Validation par checksum CRC32
  - Détection de corruption de fichier
- **Preloads de démonstration** :
  - `demo_temperature_v1.alec-context` - Capteurs température (20-25°C)
  - `demo_humidity_v1.alec-context` - Capteurs humidité (40-60%)
  - `demo_counter_v1.alec-context` - Compteurs monotoniques
- 12 nouveaux tests d'intégration pour le système preload
- Module `context::preload` avec structures `PreloadFile`, `DictEntry`, `SourceStatistics`, `PredictionModel`

### Changed
- Le contexte suit maintenant un numéro de version pour la synchronisation

---

## [0.1.0] - 2025-01-10

### Added
- **Encodeur complet**
  - Encodage raw (fallback)
  - Encodage delta (i8, i16)
  - Encodage repeated (valeurs identiques)
  - Support multi-valeurs
  - Checksum optionnel
  - Numéros de séquence
- **Décodeur complet**
  - Décodage de tous les types d'encodage
  - Vérification de checksum
  - Suivi de séquence
- **Classifieur de priorité**
  - Classification par déviation statistique
  - 6 niveaux de priorité (P0-P5)
  - Seuils configurables
- **Contexte partagé adaptatif**
  - Dictionnaire de patterns dynamique
  - Prédiction EMA (Exponential Moving Average)
  - Évolution automatique du dictionnaire
  - Scoring et pruning des patterns
  - Export/Import du contexte
- **Protocole de synchronisation**
  - Messages ANNOUNCE, REQUEST, DIFF
  - Synchronisation incrémentale
  - Détection de divergence
- **Gestion de flotte**
  - FleetManager pour gérer multiple émetteurs
  - Détection d'anomalies cross-fleet
  - Statistiques par émetteur
- **Sécurité**
  - Rate limiting par émetteur
  - Audit logging avec niveaux de sévérité
  - Validation des fingerprints
  - Configuration sécurisée
- **Monitoring de santé**
  - Health checks configurables
  - Statuts Healthy/Degraded/Unhealthy
  - Rapports de santé
- **Récupération d'erreurs**
  - Circuit breaker
  - Stratégies de retry (fixed, linear, exponential)
  - Niveaux de dégradation
- **Support TLS/DTLS**
  - Configuration TLS
  - Support mutual TLS
  - Configuration DTLS pour UDP
- **Métriques**
  - Ratio de compression
  - Distribution des encodages
  - Précision des prédictions
  - Génération de rapports
- **Canaux de communication**
  - Abstraction Channel trait
  - Implémentation mémoire pour tests
  - Support canaux avec perte
- **Documentation complète**
  - Architecture (`docs/architecture.md`)
  - Sécurité (`docs/security.md`)
  - Tests (`docs/non-regression.md`)
  - Getting started (`docs/getting-started.md`)
  - Référence protocole (`docs/protocol-reference.md`)
  - FAQ et Glossaire
- **148 tests unitaires et d'intégration**
- **9 tests de stress** (ignorés par défaut)

---

## Roadmap

### [0.3.0] - Planifié
- CLI tools (`alec-train`, `alec-info`, `alec-validate`)
- Preloads validés sur données réelles (agriculture, HVAC)
- Amélioration de la documentation API

### [0.4.0] - Planifié
- Bibliothèque de preloads par industrie
- Dashboard de visualisation
- Intégrations cloud (AWS IoT, Azure)

### [1.0.0] - Planifié
- Publication sur crates.io
- API stable et garantie de rétrocompatibilité
- Certification pour cas d'usage industriel
- Bindings Python

---

## Légende

- **Added** : Nouvelles fonctionnalités
- **Changed** : Changements dans les fonctionnalités existantes
- **Deprecated** : Fonctionnalités qui seront supprimées prochainement
- **Removed** : Fonctionnalités supprimées
- **Fixed** : Corrections de bugs
- **Security** : Corrections de vulnérabilités

---

## Liens

- [Repository](https://github.com/davidmartinventi/alec-codec)
- [Comparer les versions](https://github.com/davidmartinventi/alec-codec/compare)
- [Toutes les releases](https://github.com/davidmartinventi/alec-codec/releases)
