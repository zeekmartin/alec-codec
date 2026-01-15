# Prompt 00 — Analyse du projet ALEC

## Contexte

Tu arrives sur le projet ALEC (Adaptive Lazy Evolving Compression), un codec de compression adaptatif pour environnements contraints (IoT, embarqué). Le projet est en **v0.1.0** avec un prototype fonctionnel.

## Ta mission

Analyser le projet existant pour comprendre :
1. L'architecture et la structure du code
2. L'état actuel de l'implémentation
3. Les tests existants
4. La documentation disponible

## Étapes

### 1. Explorer la structure du projet

```bash
# Structure générale
tree -L 2 --dirsfirst

# Fichiers Rust
find src -name "*.rs" | head -20

# Tests et examples
ls -la tests/ examples/
```

### 2. Analyser le code source

Lire et comprendre les modules principaux :
- `src/lib.rs` — Point d'entrée, re-exports
- `src/protocol.rs` — Types de base (RawData, Priority, EncodingType)
- `src/encoder.rs` — Encodage des données
- `src/decoder.rs` — Décodage des messages
- `src/classifier.rs` — Classification P1-P5
- `src/context.rs` — Contexte partagé (dictionnaire, prédiction)
- `src/channel.rs` — Abstraction de canal
- `src/error.rs` — Types d'erreurs

### 3. Exécuter les tests

```bash
# Tests unitaires
cargo test

# Tests avec output
cargo test -- --nocapture

# Vérifier les warnings
cargo clippy -- -W clippy::all
```

### 4. Lire la documentation

Fichiers importants :
- `README.md` — Vue d'ensemble
- `docs/ARCHITECTURE.md` — Architecture détaillée
- `docs/PROTOCOL.md` — Spécification du protocole
- `todo.md` — Roadmap et tâches
- `prompts/*.md` — Templates de prompts

### 5. Produire un rapport

Après analyse, produire un résumé structuré :

```markdown
## Rapport d'analyse ALEC

### État actuel
- Version : 0.1.0
- Tests : X passent / Y total
- Couverture estimée : X%

### Points forts
- ...

### Points d'amélioration
- ...

### Risques identifiés
- ...

### Recommandations pour la suite
- ...
```

## Critères de succès

- [ ] Structure du projet comprise
- [ ] Tous les modules lus et compris
- [ ] Tests exécutés avec succès (44 tests)
- [ ] Documentation lue
- [ ] Rapport produit

## Prochaine étape

Une fois l'analyse terminée, passer au prompt `01_ci_cd_setup.md` pour mettre en place l'intégration continue.
