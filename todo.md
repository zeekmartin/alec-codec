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

### v1.0.0 — Production ready

Objectif : Prêt pour déploiement en production.

- [ ] **Sécurité**
  - [ ] TLS/DTLS
  - [ ] Authentification mTLS
  - [ ] Audit logging
- [ ] **Robustesse**
  - [ ] Tests de stress
  - [ ] Recovery automatique
  - [ ] Graceful degradation
- [ ] **Performance**
  - [ ] Optimisation mémoire émetteur
  - [ ] Benchmarks sur hardware cible
- [ ] **Documentation**
  - [ ] Guide de déploiement
  - [ ] API reference
  - [ ] Troubleshooting guide
- [ ] **Packaging**
  - [ ] Crate Rust publié
  - [ ] Bindings Python
  - [ ] Images Docker

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

- [x] ~~Écrire tests d'intégration~~ ✅ (87 tests)

- [x] ~~Corriger warnings dans examples~~ ✅

### Basse priorité

- [ ] Logo et assets graphiques
- [ ] Page de documentation (mdbook ou similar)
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

---

## Backlog (non priorisé)

### Fonctionnalités

- [ ] Support des timestamps relatifs
- [ ] Compression de séquences (run-length)
- [ ] Mode "replay" pour debugging
- [ ] Export vers formats standards (CSV, JSON)
- [ ] Intégration Grafana
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

### [0.4.0] - 2026-01-15 (En cours)

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
