# ALEC — Todo & Roadmap

## Vision

Créer un codec de compression adaptatif qui combine :
- **Compression paresseuse** : transmettre la décision avant la donnée
- **Contexte partagé évolutif** : dictionnaire commun qui s'enrichit
- **Asymétrie encodeur/décodeur** : léger là où c'est nécessaire

---

## Roadmap

### v0.1.0 — Prototype fonctionnel 🎯 Actuel

Objectif : Prouver le concept avec une implémentation minimale.

- [x] Architecture documentée
- [x] Interfaces définies
- [x] Templates de prompts créés
- [ ] **Encodeur basique**
  - [ ] Encodage raw (fallback)
  - [ ] Encodage delta (i8, i16)
  - [ ] Format de message binaire
- [ ] **Décodeur basique**
  - [ ] Décodage raw
  - [ ] Décodage delta
- [ ] **Classifieur simple**
  - [ ] Classification par seuils fixes
  - [ ] 5 niveaux de priorité
- [ ] **Contexte statique**
  - [ ] Dictionnaire prédéfini
  - [ ] Prédiction par dernière valeur
- [ ] **Tests unitaires**
  - [ ] Roundtrip encoding/decoding
  - [ ] Classification edge cases
- [ ] **Exemple de démonstration**
  - [ ] Capteur de température simulé
  - [ ] Émetteur + Récepteur en local

### v0.2.0 — Contexte évolutif

Objectif : Le dictionnaire s'enrichit automatiquement.

- [ ] **Contexte dynamique**
  - [ ] Comptage de fréquence des patterns
  - [ ] Promotion automatique (fréquent → code court)
  - [ ] Élagage des patterns rares
- [ ] **Synchronisation manuelle**
  - [ ] Export/import du dictionnaire
  - [ ] Vérification par hash
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
  - [ ] Support capteurs multi-métriques
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

- [ ] Implémenter `src/encoder.rs`
  - Assigné : —
  - Estimé : 2 jours
  - Bloqué par : —

- [ ] Implémenter `src/decoder.rs`
  - Assigné : —
  - Estimé : 1 jour
  - Bloqué par : encoder.rs

- [ ] Implémenter `src/classifier.rs`
  - Assigné : —
  - Estimé : 1 jour
  - Bloqué par : —

### Moyenne priorité

- [ ] Créer dataset de test `temp_sensor_24h`
  - Assigné : —
  - Estimé : 0.5 jour

- [ ] Setup CI/CD GitHub Actions
  - Assigné : —
  - Estimé : 0.5 jour

- [ ] Écrire tests d'intégration
  - Assigné : —
  - Estimé : 1 jour
  - Bloqué par : encoder, decoder

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

Aucun bug connu pour l'instant (projet en démarrage).

---

## Décisions techniques à prendre

### En attente de décision

| Question | Options | Pour | Contre | Décision |
|----------|---------|------|--------|----------|
| Langage principal | Rust vs C | Rust: sécurité mémoire, C: portabilité | Rust: learning curve | Rust ✓ |
| Format binaire | Custom vs Protobuf vs CBOR | Custom: optimal, Standards: tooling | Custom: maintenance | À décider |
| Transport | MQTT vs CoAP vs Custom | MQTT: écosystème, CoAP: UDP natif | — | Les deux |

### Décidées

- **Rust** pour le cœur du codec (sécurité, performance)
- **Asymétrie** par défaut : émetteur léger, récepteur puissant
- **5 niveaux de priorité** : P1-P5 (extensible si besoin)

---

## Notes de réunion

### 2025-01-15 — Kickoff

Participants : —

Points discutés :
- Architecture validée
- Templates de prompts créés
- Prochaine étape : implémentation v0.1

Actions :
- [ ] Créer repo GitHub
- [ ] Setup environnement de dev
- [ ] Premier commit avec structure

---

## Changelog

### [Unreleased]

#### Added
- Documentation initiale (architecture, sécurité, non-régression)
- Templates de prompts (feature, refactor, bugfix, security, tests)
- Exemples de workflow
- Charte graphique

#### Changed
- Rien

#### Fixed
- Rien

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
