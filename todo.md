# ALEC — Todo & Roadmap

## Vision

Créer un codec de compression adaptatif qui combine :
- **Compression paresseuse** : transmettre la décision avant la donnée
- **Contexte partagé évolutif** : dictionnaire commun qui s'enrichit
- **Asymétrie encodeur/décodeur** : léger là où c'est nécessaire

---

## Roadmap

### v0.1.0 — Prototype fonctionnel ✅ Complété

Objectif : Prouver le concept avec une implémentation minimale.

- [x] Architecture documentée
- [x] Interfaces définies
- [x] Templates de prompts créés
- [x] **Encodeur basique**
  - [x] Encodage raw (fallback Raw32, Raw64)
  - [x] Encodage delta (i8, i16, i32)
  - [x] Encodage repeated (0 octet)
  - [x] Format de message binaire (varint)
  - [x] Encodage multi-valeurs
- [x] **Décodeur basique**
  - [x] Décodage raw
  - [x] Décodage delta
  - [x] Décodage repeated
  - [x] Décodage multi-valeurs
  - [x] Tracking des séquences
- [x] **Classifieur simple**
  - [x] Classification par seuils fixes
  - [x] 5 niveaux de priorité (P1-P5)
  - [x] Détection d'anomalies
  - [x] Seuils critiques configurables
- [x] **Contexte statique**
  - [x] Dictionnaire de patterns
  - [x] Prédiction par dernière valeur
  - [x] Export/Import du contexte
  - [x] Hash de vérification
- [x] **Tests unitaires** (44 tests)
  - [x] Roundtrip encoding/decoding
  - [x] Classification edge cases
  - [x] Varint encoding
  - [x] Channel tests
- [x] **Exemple de démonstration**
  - [x] simple_sensor.rs
  - [x] emitter_receiver.rs

### v0.2.0 — Contexte évolutif ✅ Complété

Objectif : Le dictionnaire s'enrichit automatiquement.

- [x] **Contexte dynamique** ✅
  - [x] Comptage de fréquence des patterns (Pattern.frequency, last_used)
  - [x] Promotion automatique (fréquent → code court via reorder_patterns)
  - [x] Élagage des patterns rares (prune_patterns)
- [x] **Synchronisation manuelle** ✅
  - [x] Export/import du dictionnaire
  - [x] Vérification par hash
  - [x] Diff de contexte (SyncDiff)
- [x] **Modèle prédictif amélioré** ✅
  - [x] Moyenne mobile exponentielle (EMA)
  - [ ] Régression linéaire simple
- [x] **Métriques** ✅
  - [x] Ratio de compression (CompressionMetrics)
  - [x] Taille du dictionnaire (pattern_count)
  - [x] Taux de prédiction réussie (prediction_accuracy)

### v0.3.0 — Synchronisation automatique 🔄 En cours

Objectif : Les contextes se synchronisent automatiquement.

- [x] **Sync incrémentale** ✅
  - [x] Diff de dictionnaire (SyncDiff)
  - [x] Messages SYNC (SyncMessage, SyncAnnounce, SyncRequest)
  - [x] Récupération après divergence (SyncState::Diverged)
  - [x] State machine (Synchronizer)
  - [x] Sérialisation/désérialisation messages sync
- [x] **Requêtes différées** ✅
  - [x] REQ_DETAIL (SyncMessage::ReqDetail)
  - [x] REQ_RANGE (SyncMessage::ReqRange, RangeRequest)
  - [ ] Rate limiting
- [ ] **Canal bidirectionnel**
  - [ ] SyncChannel wrapper
  - [ ] Implémentation MQTT
  - [ ] Implémentation CoAP
- [ ] **Multi-valeurs**
  - [x] Support capteurs multi-métriques (encode_multi/decode_multi)
  - [ ] Corrélations entre métriques

### v0.4.0 — Mode flotte 🔄 En cours

Objectif : Plusieurs émetteurs, un récepteur central.

- [x] **Gestion multi-émetteurs** ✅
  - [x] Contextes par émetteur (EmitterState)
  - [x] Contexte partagé de flotte (fleet_context)
  - [x] FleetManager avec configuration
  - [x] FleetStats pour statistiques
- [x] **Apprentissage collectif** ✅
  - [x] Patterns communs à la flotte (sync_fleet_patterns)
  - [x] Détection d'anomalies par comparaison (cross-fleet)
  - [x] Fleet mean et std dev
- [ ] **Dashboard**
  - [ ] Visualisation temps réel
  - [ ] Métriques agrégées
  - [ ] Alertes

### v1.0.0 — Production ready ✅ Complété

Objectif : Prêt pour déploiement en production.

- [x] **Sécurité** ✅
  - [x] TLS/DTLS (TlsConfig, DtlsConfig interfaces)
  - [x] Authentification mTLS (SecurityConfig, validate_fingerprint)
  - [x] Audit logging (AuditLogger, MemoryAuditLogger)
  - [x] Rate limiting (RateLimiter avec token bucket)
  - [x] SecurityContext avec intégration FleetManager
- [x] **Robustesse** ✅
  - [x] Tests de stress (tests/stress.rs - 9 tests)
  - [x] Recovery automatique (CircuitBreaker, RetryStrategy)
  - [x] Graceful degradation (DegradationLevel)
- [x] **Documentation** ✅
  - [x] Guide de déploiement (mdBook)
  - [x] API reference (rustdoc enrichi)
  - [x] Troubleshooting guide
  - [x] FAQ
  - [x] CI pour documentation
- [x] **Release v1.0.0** ✅
  - [x] Headers de licence AGPL-3.0 ajoutés
  - [x] Cargo.toml prêt pour crates.io
  - [x] README.md avec dual licensing
  - [x] Dry run cargo publish réussi

### v1.1.0 — Preload System 🔄 En cours

Objectif : Compression optimale dès le premier octet grâce aux fichiers preload.

- [x] **Phase 1 : Foundation** ✅
  - [x] Structure `PreloadFile` avec sérialisation binaire
  - [x] Format `.alec-context` (header 64 bytes + dictionary + stats + prediction)
  - [x] `save_to_file()` et `load_from_file()` sur Context
  - [x] Checksum CRC32 pour intégrité des fichiers
  - [x] `VersionCheckResult` pour vérification de synchronisation
  - [x] 12 tests d'intégration (tests/preload_tests.rs)
- [x] **Phase 2 : Demo Preloads** ✅
  - [x] Création de preloads de démonstration (temperature, humidity, counter)
  - [x] Générateur `examples/generate_preloads.rs`
  - [x] Vérification automatique des preloads générés
  - [ ] Validation avec données réelles de capteurs
  - [ ] Documentation des best practices
- [ ] **Phase 3 : Industry Preloads**
  - [ ] Agriculture (soil moisture, temperature, humidity)
  - [ ] HVAC (temperature, pressure, flow)
  - [ ] Energy (voltage, current, power)

### v1.2.0 — C/C++ FFI Bindings ✅ Complété

Objectif : Permettre l'intégration avec des projets C/C++.

- [x] **Phase 1 : Core FFI** ✅
  - [x] Crate `alec-ffi` avec structure complète
  - [x] `AlecEncoder` / `AlecDecoder` opaque handles
  - [x] `alec_encode_value()` et `alec_encode_multi()`
  - [x] `alec_decode_value()` et `alec_decode_multi()`
  - [x] Support checksum (encoder/decoder with_checksum)
  - [x] `AlecResult` enum avec codes d'erreur
- [x] **Phase 2 : Context Support** ✅
  - [x] `alec_encoder_save_context()` pour export preload
  - [x] `alec_encoder_load_context()` pour import preload
  - [x] `alec_decoder_load_context()`
  - [x] `alec_encoder_context_version()` / `alec_decoder_context_version()`
- [x] **Phase 3 : Build & Distribution** ✅
  - [x] Build static library (`libalec.a`)
  - [x] Build shared library (`libalec.so` / `libalec.dylib`)
  - [x] Header C (`include/alec.h`) avec documentation
  - [x] cbindgen pour génération automatique
  - [x] Exemple C (`examples/example.c`)
  - [x] 11 tests Rust FFI
  - [x] README avec instructions de build et linking

### v1.3.0 — Demo Infrastructure ✅ Complété

Objectif : Environnement de démonstration complet pour valider Gateway & Complexity.

- [x] **Sensor Simulator** ✅
  - [x] Génération temps réel avec variables latentes
  - [x] 15 capteurs agricoles corrélés
  - [x] Profils JSON configurables
  - [x] Endpoint Prometheus `/metrics`
  - [x] API REST `/readings`, `/sensors`, `/status`
- [x] **Injection Service** ✅
  - [x] API FastAPI pour injection d'anomalies
  - [x] Types: noise, spike, drift, dropout
  - [x] Gestion d'état par capteur
  - [x] Auto-expiration des injections
- [x] **Grafana Dashboard** ✅
  - [x] Dashboard ALEC Demo pré-provisionné
  - [x] Panels: Entropy, Complexity, Robustness
  - [x] Heatmap de corrélation
  - [x] Détection d'anomalies
- [x] **Docker Compose** ✅
  - [x] Orchestration complète 6 services
  - [x] Health checks et dépendances
  - [x] Volumes persistants
- [x] **Documentation** ✅
  - [x] README avec architecture et quickstart
  - [x] Référence API complète
  - [x] Guide de troubleshooting

### Backlog v1.x / v2.0

- [ ] **Performance** (v1.4)
  - [ ] Optimisation mémoire émetteur
  - [ ] Benchmarks sur hardware cible
- [ ] **Packaging** (v2.0)
  - [x] Bindings C/C++ (alec-ffi) ✅
  - [ ] Bindings Python (PyO3)
  - [x] Images Docker ✅ (demo/docker-compose.yml)
  - [x] Dashboard visualisation ✅ (demo/grafana/)

---

## Tâches immédiates (Sprint actuel)

### Haute priorité

- [x] ~~Implémenter `src/encoder.rs`~~ ✅
- [x] ~~Implémenter `src/decoder.rs`~~ ✅
- [x] ~~Implémenter `src/classifier.rs`~~ ✅
- [x] ~~Implémenter vérification checksum (encoder/decoder)~~ ✅ xxHash32
- [ ] Implémenter scheduling dans classifier

### Moyenne priorité

- [ ] Créer dataset de test `temp_sensor_24h`
  - Assigné : —
  - Estimé : 0.5 jour

- [x] ~~Setup CI/CD GitHub Actions~~ ✅ (ci.yml + release.yml)

- [x] ~~Écrire tests d'intégration~~ ✅ (103 tests)

- [x] ~~Corriger warnings dans examples~~ ✅

### Basse priorité

- [ ] Logo et assets graphiques
- [x] ~~Page de documentation (mdbook ou similar)~~ ✅
- [ ] Exemple vidéo/démo

### Ajouts récents ✅

- [x] Module `metrics` pour analyse de compression
- [x] `CompressionMetrics` et `ContextMetrics`
- [x] Exemple `metrics_demo.rs`
- [x] Module `sync` pour synchronisation automatique
- [x] `SyncMessage`, `SyncDiff`, `Synchronizer`
- [x] Sérialisation messages de sync
- [x] Module `fleet` pour mode multi-émetteurs
- [x] `FleetManager`, `EmitterState`, `FleetStats`
- [x] Détection cross-fleet anomaly
- [x] Exemple `fleet_demo.rs`
- [x] Module `security` avec rate limiting et audit logging
- [x] `SecurityContext`, `RateLimiter`, `AuditLogger`
- [x] Module `tls` avec interfaces TLS/DTLS
- [x] Module `health` pour monitoring de santé
- [x] `HealthMonitor`, `HealthCheckable` trait
- [x] Module `recovery` avec `CircuitBreaker`, `RetryStrategy`
- [x] `DegradationLevel` pour graceful degradation
- [x] Tests de stress `tests/stress.rs`
- [x] Documentation mdBook complète (docs/book/)
- [x] Documentation rustdoc enrichie avec exemples
- [x] CI pour build documentation
- [x] **Release v1.0.0 préparé** ✅
  - [x] Headers licence AGPL-3.0 dans tous les src/*.rs
  - [x] Cargo.toml v1.0.0 avec metadata crates.io
  - [x] README.md avec dual licensing (AGPL + Commercial)
  - [x] Script scripts/add_headers.sh
- [x] **Preload System Phase 1** ✅
  - [x] Module `context/preload.rs` avec `PreloadFile`
  - [x] Format binaire `.alec-context` avec CRC32
  - [x] `Context::save_to_file()` et `Context::load_from_file()`
  - [x] `VersionCheckResult` pour sync checking
  - [x] 12 tests d'intégration preload
- [x] **Preload System Phase 2 - Demo Preloads** ✅
  - [x] `examples/generate_preloads.rs` générateur
  - [x] `demo_temperature_v1.alec-context` (20-25°C, MovingAverage)
  - [x] `demo_humidity_v1.alec-context` (40-60%, Periodic)
  - [x] `demo_counter_v1.alec-context` (monotonic, Linear)
- [x] **C/C++ FFI Bindings** ✅
  - [x] Crate `alec-ffi/` avec build system
  - [x] `AlecEncoder` / `AlecDecoder` opaque handles
  - [x] Encode/decode single + multi values
  - [x] Context save/load pour preloads
  - [x] Header C `include/alec.h` documenté
  - [x] Exemple C `examples/example.c`
  - [x] Static (`libalec.a`) et shared (`libalec.so`) libraries
  - [x] 11 tests FFI Rust

---

## Backlog (non priorisé)

### Fonctionnalités

- [ ] Support des timestamps relatifs
- [ ] Compression de séquences (run-length)
- [ ] Mode "replay" pour debugging
- [ ] Export vers formats standards (CSV, JSON)
- [x] Intégration Grafana ✅ (demo/grafana/ + alec-grafana/)
- [ ] Support WebSocket pour dashboard

### Technique

- [ ] Benchmarks automatisés dans CI
- [ ] Fuzzing avec cargo-fuzz
- [ ] Property-based testing avec proptest
- [ ] Documentation inline (rustdoc)
- [ ] Couverture de code > 80%

### Portabilité

- [ ] Tester sur ARM Cortex-M4
- [ ] Tester sur ESP32
- [ ] Tester sur Raspberry Pi
- [ ] Version no_std pour embedded

---

## Bugs connus

- ~~Bug #1: choose_encoding vérifie Delta avant Repeated~~ ✅ Corrigé 2025-01-15

---

## Décisions techniques à prendre

### En attente de décision

| Question | Options | Pour | Contre | Décision |
|----------|---------|------|--------|----------|
| Format binaire | Custom vs Protobuf vs CBOR | Custom: optimal, Standards: tooling | Custom: maintenance | Custom ✓ |
| Transport | MQTT vs CoAP vs Custom | MQTT: écosystème, CoAP: UDP natif | — | Les deux |

### Décidées

- **Rust** pour le cœur du codec (sécurité, performance)
- **Asymétrie** par défaut : émetteur léger, récepteur puissant
- **5 niveaux de priorité** : P1-P5 (extensible si besoin)
- **Format binaire custom** avec varint encoding

---

## Notes de réunion

### 2025-01-15 — Kickoff

Participants : —

Points discutés :
- Architecture validée
- Templates de prompts créés
- Prochaine étape : implémentation v0.1

Actions :
- [x] Créer repo GitHub
- [x] Setup environnement de dev
- [x] Premier commit avec structure
- [x] Implémentation v0.1.0 complète

---

## Changelog

### [1.2.0] - 2026-01-21 (En cours)

#### Added
- Crate `alec-ffi` pour bindings C/C++
- `AlecEncoder` et `AlecDecoder` opaque handles
- `alec_encode_value()` et `alec_encode_multi()` pour encodage
- `alec_decode_value()` et `alec_decode_multi()` pour décodage
- `alec_encoder_save_context()` et `alec_encoder_load_context()` pour preloads
- `alec_decoder_load_context()` pour preloads décodeur
- `AlecResult` enum avec codes d'erreur (ALEC_OK, ALEC_ERROR_*)
- Support checksum via `alec_encoder_new_with_checksum()`
- Header C complet `alec-ffi/include/alec.h` avec documentation
- Build script cbindgen pour génération automatique de headers
- Exemple C `alec-ffi/examples/example.c`
- Static library `libalec.a` et shared library `libalec.so`
- 11 tests FFI Rust

### [1.1.0] - 2026-01-16

#### Added
- Module `context/preload` pour fichiers preload
- `PreloadFile` struct avec sérialisation binaire `.alec-context`
- `PreloadDictEntry`, `PreloadStatistics`, `PreloadPredictionModel`
- `Context::save_to_file()` pour exporter un contexte entraîné
- `Context::load_from_file()` pour charger un preload
- `Context::context_version()` et `Context::check_version()`
- `VersionCheckResult` enum pour vérification de sync
- Checksum CRC32 pour intégrité des fichiers preload
- Dépendance `crc32fast` pour checksums
- 12 tests d'intégration `tests/preload_tests.rs`
- Exemple `generate_preloads.rs` pour générer des preloads de démo
- 3 preloads de démonstration dans `contexts/demo/`:
  - `demo_temperature_v1.alec-context` (capteur température 20-25°C)
  - `demo_humidity_v1.alec-context` (capteur humidité 40-60%)
  - `demo_counter_v1.alec-context` (compteur monotonic)

### [1.0.0] - 2026-01-15

#### Added
- Module `security` pour sécurité production
- `SecurityConfig` pour configuration TLS/mTLS/audit
- `AuditLogger` trait avec `MemoryAuditLogger` implementation
- `AuditEvent` et `AuditEventType` pour audit logging
- `RateLimiter` avec algorithme token bucket
- `SecurityContext` pour intégration session
- `process_message_secure()` sur FleetManager
- Module `tls` avec interfaces TLS/DTLS
- `TlsConfig`, `DtlsConfig`, `TlsState`
- Feature flag `tls` pour rustls/webpki-roots
- Module `health` pour monitoring de santé
- `HealthStatus`, `HealthCheck`, `HealthMonitor`, `HealthConfig`
- `HealthCheckable` trait avec implementation pour `Context`
- Module `recovery` pour résilience
- `CircuitBreaker` avec états Closed/Open/HalfOpen
- `RetryStrategy` avec Fixed/ExponentialBackoff/LinearBackoff
- `DegradationLevel` pour graceful degradation
- `with_retry()` et `with_retry_metrics()` helpers
- Tests de stress `tests/stress.rs` (9 tests performance)
- 39 nouveaux tests health/recovery (142 tests total)
- Documentation mdBook complète (docs/book/)
- Guide utilisateur, troubleshooting, FAQ
- Documentation rustdoc enrichie avec exemples
- CI pour build et vérification documentation
- **Release Preparation**
- Headers licence AGPL-3.0 dans tous les src/*.rs
- Cargo.toml v1.0.0 prêt pour crates.io
- README.md avec section dual licensing
- Script add_headers.sh pour automatisation
- Dry run cargo publish validé

### [0.4.0] - 2026-01-15

#### Added
- Module `fleet` pour gestion multi-émetteurs
- `FleetManager` avec contextes par émetteur et contexte partagé
- `EmitterState` avec statistiques (mean, std_dev, recent_values)
- `FleetStats` pour métriques fleet-wide
- Détection cross-fleet anomaly avec z-score
- Synchronisation patterns communs vers fleet context
- Méthode `pattern_hashes()` sur Context
- Exemple `fleet_demo.rs`
- 10 nouveaux tests fleet (87 tests total)

### [0.3.0] - 2026-01-15

#### Added
- Module `sync` pour synchronisation automatique des contextes
- Types `SyncMessage`, `SyncAnnounce`, `SyncRequest`, `SyncDiff`
- State machine `Synchronizer` pour gestion des états de sync
- Messages `ReqDetail` et `ReqRange` pour requêtes différées
- Sérialisation binaire des messages de synchronisation
- Méthodes helper Context: `remove_pattern`, `set_pattern`, `has_pattern`, `patterns_iter`, `pattern_ids`, `set_version`
- 14 nouveaux tests de synchronisation (77 tests total)

### [0.2.0] - 2026-01-15

#### Added
- Contexte évolutif avec `EvolutionConfig`
- Pattern scoring et reordering automatique
- Pruning des patterns peu utilisés
- Prédiction EMA (Exponential Moving Average)
- Module `metrics` avec `CompressionMetrics` et `ContextMetrics`
- Exemple `metrics_demo.rs`

### [0.1.0] - 2025-01-15

#### Added
- Encodeur complet (raw, delta, repeated, multi)
- Décodeur complet avec roundtrip vérifié
- Classifieur 5 niveaux (P1-P5)
- Contexte avec dictionnaire et prédiction
- Channel abstraction (memory, lossy)
- Vérification checksum xxHash32
- CI/CD GitHub Actions (ci.yml, release.yml)
- 44 tests unitaires
- 2 exemples (simple_sensor, emitter_receiver)
- Documentation initiale

#### Fixed
- Bug choose_encoding : Repeated vérifié avant Delta

---

## Comment contribuer

1. Choisir une tâche dans "Tâches immédiates" ou "Backlog"
2. Créer une branche `feature/nom-de-la-tache`
3. Suivre le template de prompt approprié
4. Soumettre une PR avec tests
5. Review et merge

Pour les bugs : utiliser `prompts/bugfix.prompt.md`
Pour les features : utiliser `prompts/feature.prompt.md`
Pour le refactoring : utiliser `prompts/refactor.prompt.md`
