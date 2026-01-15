# Changelog

Toutes les modifications notables de ce projet seront documentées dans ce fichier.

Le format est basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.0.0/),
et ce projet adhère au [Semantic Versioning](https://semver.org/lang/fr/).

---

## [Unreleased]

### Added
- Documentation complète du projet
  - Architecture et principes fondamentaux (`docs/architecture.md`)
  - Guide de sécurité (`docs/security.md`)
  - Stratégie de tests (`docs/non-regression.md`)
  - Charte graphique (`docs/graphic-charter.md`)
  - Communication inter-composants (`docs/intra-application.md`)
  - Applications et cas d'usage détaillés (`docs/applications.md`)
  - Guide de démarrage (`docs/getting-started.md`)
  - Référence du protocole (`docs/protocol-reference.md`)
  - FAQ (`docs/faq.md`)
  - Glossaire (`docs/glossary.md`)
- Templates de prompts pour développement assisté par IA
  - Template feature (`prompts/feature.prompt.md`)
  - Template refactoring (`prompts/refactor.prompt.md`)
  - Template bugfix (`prompts/bugfix.prompt.md`)
  - Template security review (`prompts/security-review.prompt.md`)
  - Template tests (`prompts/non-regression.prompt.md`)
- Exemples de workflows
  - Itération de fonctionnalité (`examples/01-feature-iteration.md`)
  - Refactoring (`examples/02-refactor.md`)
  - Correction de bug (`examples/03-bugfix.md`)
- README principal avec présentation du projet
- Guide de contribution (CONTRIBUTING.md)
- Roadmap et todo list (todo.md)
- Licence MIT

### Changed
- Rien

### Deprecated
- Rien

### Removed
- Rien

### Fixed
- Rien

### Security
- Rien

---

## [0.1.0] - À venir

### Prévu
- Implémentation de l'encodeur basique
  - Encodage raw (fallback)
  - Encodage delta (i8, i16)
  - Format de message binaire
- Implémentation du décodeur basique
  - Décodage raw
  - Décodage delta
- Implémentation du classifieur simple
  - Classification par seuils fixes
  - 5 niveaux de priorité (P1-P5)
- Contexte statique
  - Dictionnaire prédéfini
  - Prédiction par dernière valeur
- Tests unitaires
  - Roundtrip encoding/decoding
  - Classification edge cases
- Exemple de démonstration
  - Capteur de température simulé
  - Émetteur + Récepteur en local

---

## [0.2.0] - Planifié

### Prévu
- Contexte dynamique
  - Comptage de fréquence des patterns
  - Promotion automatique
  - Élagage des patterns rares
- Synchronisation manuelle du contexte
- Modèle prédictif amélioré
  - Moyenne mobile
  - Régression linéaire simple
- Métriques de performance

---

## [0.3.0] - Planifié

### Prévu
- Synchronisation automatique du contexte
- Requêtes différées (REQ_DETAIL, REQ_RANGE)
- Implémentation MQTT et CoAP
- Support multi-valeurs

---

## [0.4.0] - Planifié

### Prévu
- Mode flotte
- Apprentissage collectif
- Dashboard de visualisation

---

## [1.0.0] - Planifié

### Prévu
- Sécurité production (TLS/DTLS)
- Tests de stress complets
- Documentation API complète
- Packaging (crate Rust, bindings Python)
- Certification pour cas d'usage industriel

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

- [Comparer les versions](https://github.com/votre-org/alec-codec/compare)
- [Toutes les releases](https://github.com/votre-org/alec-codec/releases)
