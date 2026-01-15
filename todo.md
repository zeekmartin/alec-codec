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

### v0.2.0 — Contexte évolutif 🎯 Prochain

Objectif : Le dictionnaire s'enrichit automatiquement.

- [ ] **Contexte dynamique**
  - [ ] Comptage de fréquence des patterns
  - [ ] Promotion automatique (fréquent → code court)
  - [ ] Élagage des patterns rares
- [ ] **Synchronisation manuelle**
  - [ ] Export/import du dictionnaire (partiellement fait)
  - [ ] Vérification par hash (fait)
  - [ ] Diff de contexte
- [ ] **Modèle prédictif amélioré**
  - [ ] Moyenne mobile
  - [ ] Régression linéaire simple
- [ ] **Métriques**
  - [ ] Ratio de compression
  - [ ] Taille du dictionnaire
  - [ ] Taux de prédiction réussie

### v0.3.0 — Synchronisation automatique

Objectif : Les contextes se synchronisent automatiquement.

- [ ] **Sync incrémentale**
  - [ ] Diff de dictionnaire
  - [ ] Messages SYNC
  - [ ] Récupération après divergence
- [ ] **Requêtes différées**
  - [ ] REQ_DETAIL
  - [ ] REQ_RANGE
  - [ ] Rate limiting
- [ ] **Canal bidirectionnel**
  - [ ] Implémentation MQTT
  - [ ] Implémentation CoAP
- [ ] **Multi-valeurs**
  - [x] Support capteurs multi-métriques (encode_multi/decode_multi)
  - [ ] Corrélations entre métriques

### v0.4.0 — Mode flotte

Objectif : Plusieurs émetteurs, un récepteur central.

- [ ] **Gestion multi-émetteurs**
  - [ ] Contextes par émetteur
  - [ ] Contexte partagé de flotte
- [ ] **Apprentissage collectif**
  - [ ] Patterns communs à la flotte
  - [ ] Détection d'anomalies par comparaison
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
- [ ] Implémenter vérification checksum (encoder/decoder)
- [ ] Implémenter scheduling dans classifier

### Moyenne priorité

- [ ] Créer dataset de test `temp_sensor_24h`
  - Assigné : —
  - Estimé : 0.5 jour

- [ ] Setup CI/CD GitHub Actions
  - Assigné : —
  - Estimé : 0.5 jour

- [x] ~~Écrire tests d'intégration~~ ✅ (44 tests)

- [ ] Corriger warnings dans examples
  - simple_sensor.rs: unused import Priority
  - emitter_receiver.rs: unused variable pair

### Basse priorité

- [ ] Logo et assets graphiques
- [ ] Page de documentation (mdbook ou similar)
- [ ] Exemple vidéo/démo

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

### [0.1.0] - 2025-01-15

#### Added
- Encodeur complet (raw, delta, repeated, multi)
- Décodeur complet avec roundtrip vérifié
- Classifieur 5 niveaux (P1-P5)
- Contexte avec dictionnaire et prédiction
- Channel abstraction (memory, lossy)
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
