# Changelog

Toutes les modifications notables de ce projet seront documentées dans ce fichier.

Le format est basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.0.0/),
et ce projet adhère au [Semantic Versioning](https://semver.org/lang/fr/).

---

## [Unreleased]

### Added

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
