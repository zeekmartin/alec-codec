# Changelog

Toutes les modifications notables de ce projet seront documentées dans ce fichier.

Le format est basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.0.0/),
et ce projet adhère au [Semantic Versioning](https://semver.org/lang/fr/).

---

## [1.3.7] — 2026-04-23

### Added
- Encoder context save/load with RAM buffers:
  `alec_encoder_context_save()`, `alec_encoder_context_load()`.
  Mirrors the decoder buffer API added in v1.3.6 and enables
  save/restore on MCUs without a filesystem.
- Firmware use case: roll back the encoder state when an encoded frame
  exceeds the LoRaWAN payload ceiling so the prediction model is not
  polluted by a frame the decoder never receives.
- `Encoder::restore_sequence()` — minimal setter used by the FFI
  save/restore path; does not change any encode behaviour.

### Wire format
- New `ALEE` binary blob (24-byte header + ALCS-wrapped Context).
  Header carries encoder sequence, force-keyframe flag,
  messages-since-keyframe, ALCS length and an xxh64 integrity check.
  The decoder-side buffer format from v1.3.6 is unchanged.

---

## [1.3.6] — 2026-04-22

### Added
- Decoder FFI: `alec_decode_multi_fixed()` for server-side frame decoding
  (extended signature with `num_channels_out`, `sequence_out`,
  `is_keyframe_out`)
- Decoder lifecycle: `alec_decoder_new_with_config()`,
  `alec_decoder_free()`, `alec_decoder_reset()`
- Decoder context persistence: `alec_decoder_context_save()`,
  `alec_decoder_context_load()` (sensor-type-agnostic wrappers over
  the existing `_export_state` / `_import_state` APIs)
- Feature flag `decoder` (default on) — decoder requires `std`,
  encoder remains `no_std`
- Round-trip encode→decode integration tests under
  `alec-ffi/tests/decoder_roundtrip.rs`

### Changed
- `alec_decode_multi_fixed()` signature now accepts `values_out` /
  `max_channels` / `num_channels_out` / `sequence_out` /
  `is_keyframe_out` instead of the v1.3.5 `channel_count` / `output` /
  `output_capacity` triple. The wire format is unchanged — only the
  FFI shape evolved.

---

## [1.3.5] - 2026-04-15

### Added
- Compact fixed-channel wire format (4B header,
  2-bit-per-channel bitmap, `0xA1`/`0xA2` markers)
- `alec_encoder_new_with_config()` FFI with
  `AlecEncoderConfig` (history_size, max_patterns,
  max_memory_bytes, keyframe_interval, smart_resync)
- `alec_heap_init_with_buffer()` for caller-managed
  heap on bare-metal targets
- `alec_encode_multi_fixed()` / `alec_decode_multi_fixed()`
  for fixed-channel positional encoding
- `alec_force_keyframe()` — force immediate Raw32 frame
- `alec_downlink_handler()` — LoRaWAN downlink handler,
  `0xFF` command triggers immediate keyframe
- `alec_decoder_gap_detected()` — surface sequence gaps
  to application layer
- `alec_decoder_export_state()` / `import_state()` —
  in-memory context persistence (ALCS format, ~1.5 KB)
- `alec_decoder_export_state_size()` — pre-flight size
  query before export
- `Context::reset_to_baseline()` — wipes `source_stats`,
  preserves patterns/dictionary
- `Context::to_preload_bytes()` / `from_preload_bytes()` —
  `no_std + alloc` in-memory serialization
- New wire format: ALCS (ALec Context State) v1
  with CRC32 ISO-HDLC checksum
- `ALEC_ERROR_CORRUPT_DATA = 9` result code
- Packet-loss recovery: periodic keyframe + sequence
  gap reset + LoRaWAN downlink smart resync
- Multi-arch bare-metal: M3 / M4 / M4F / M0+
- `log` facade (zero-cost on bare-metal, no subscriber)

### Fixed
- Sequence gap detection previously a no-op
  ("For now, just continue") — now triggers
  context reset and logs warning
- `context_version` `check_version()` was never called
  in decode path — now wired into `decode_multi_fixed`

### Notes
- Compact mode steady-state: ~8 B avg for 5 channels
  vs 6.1 B in preliminary benchmarks — the 2 B bitmap
  overhead was not accounted for in initial estimates
- Worst-case packet-loss drift:
  `keyframe_interval × reporting_interval`
  (default 50 × 10 min ≈ 8h).
  With smart-resync downlink: `1 × reporting_interval`
- ALCS format coexists with `PreloadFile` (`ALEC` magic)
  via distinct magic bytes — no backward-compat break

---

## [1.3.1] - 2026-03-10

### Fixed
- Timestamp serialized as Unix seconds instead of milliseconds (fixes 49-day overflow bug)
- encode_raw() now reads context_version from Context instead of hardcoded 0

### Changed
- Header: 13B → 10B (-3B/frame)
  - sequence: u32 → u16 (-2B on wire)
  - context_version: u32 → u24 (-1B on wire)
- name_id serialized as u8 instead of u16 in multi-channel frame (-1B/channel, -5B on 5-channel payload)
- Total saving vs 1.3.0: up to 8B on a 5-channel NB-IoT frame

### Added
- 11 regression tests in tests/protocol_header_v2.rs
- ROADMAP.md: v1.4 section documenting Pattern & Interpolated encoding (planned)

### Validated
- Nordic nRF9151 / Zephyr / NB-IoT hardware
- 17B stable from seq=13, 72-73% vs JSON equivalent, 118+ sequences

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
