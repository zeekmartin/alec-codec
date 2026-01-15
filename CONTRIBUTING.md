# Guide de contribution

Merci de votre intérêt pour ALEC ! Ce guide vous aidera à contribuer efficacement au projet.

---

## Table des matières

1. [Code de conduite](#code-de-conduite)
2. [Comment contribuer](#comment-contribuer)
3. [Environnement de développement](#environnement-de-développement)
4. [Conventions de code](#conventions-de-code)
5. [Processus de PR](#processus-de-pr)
6. [Templates disponibles](#templates-disponibles)

---

## Code de conduite

### Nos engagements

- Créer un environnement accueillant et inclusif
- Respecter les différents points de vue et expériences
- Accepter les critiques constructives avec grâce
- Se concentrer sur ce qui est le mieux pour la communauté

### Comportements inacceptables

- Langage ou images à caractère sexuel
- Trolling, commentaires insultants ou désobligeants
- Harcèlement public ou privé
- Publication d'informations privées sans consentement

---

## Comment contribuer

### Signaler un bug

1. **Vérifiez** qu'il n'existe pas déjà dans les [issues](https://github.com/votre-org/alec-codec/issues)
2. **Créez** une nouvelle issue avec le template "Bug Report"
3. **Incluez** :
   - Version d'ALEC
   - Environnement (OS, hardware, Rust version)
   - Étapes de reproduction minimales
   - Comportement attendu vs observé
   - Logs si disponibles

### Proposer une fonctionnalité

1. **Ouvrez** une issue "Feature Request"
2. **Décrivez** :
   - Le cas d'usage concret
   - Pourquoi les solutions existantes ne suffisent pas
   - Une proposition d'implémentation (optionnel)
3. **Attendez** la discussion avant d'implémenter

### Soumettre du code

1. **Fork** le repository
2. **Créez** une branche depuis `main`
3. **Utilisez** le template de prompt approprié
4. **Implémentez** votre changement
5. **Testez** localement
6. **Soumettez** une Pull Request

---

## Environnement de développement

### Installation

```bash
# Cloner votre fork
git clone https://github.com/VOTRE_USER/alec-codec.git
cd alec-codec

# Ajouter le remote upstream
git remote add upstream https://github.com/votre-org/alec-codec.git

# Installer les outils de développement
rustup component add clippy rustfmt
cargo install cargo-tarpaulin  # Couverture de code
```

### Commandes utiles

```bash
# Formater le code
cargo fmt

# Linter
cargo clippy -- -D warnings

# Tests
cargo test

# Tests avec logs
cargo test -- --nocapture

# Couverture
cargo tarpaulin --out Html

# Benchmarks
cargo bench

# Documentation
cargo doc --open
```

### Structure des branches

```
main              # Stable, releases
├── develop       # Intégration
├── feature/*     # Nouvelles fonctionnalités
├── bugfix/*      # Corrections de bugs
├── refactor/*    # Refactoring
└── release/*     # Préparation de release
```

---

## Conventions de code

### Style Rust

Nous suivons les conventions Rust standard :

```rust
// Nommage
let snake_case_variable = 42;
const SCREAMING_SNAKE_CASE: u32 = 100;
fn snake_case_function() {}
struct PascalCaseStruct {}
enum PascalCaseEnum {}

// Documentation
/// Description courte sur une ligne.
///
/// Description longue si nécessaire,
/// avec exemples de code.
///
/// # Examples
///
/// ```
/// let result = my_function(42);
/// assert_eq!(result, 84);
/// ```
pub fn my_function(x: i32) -> i32 {
    x * 2
}
```

### Commits

Format des messages de commit :

```
type(scope): description courte

Corps optionnel avec plus de détails.

Refs: #123
```

Types :
- `feat` : Nouvelle fonctionnalité
- `fix` : Correction de bug
- `docs` : Documentation
- `style` : Formatage (pas de changement de code)
- `refactor` : Refactoring
- `test` : Ajout de tests
- `chore` : Maintenance (CI, dépendances...)

Exemples :
```
feat(encoder): add Delta16 encoding support

Add support for 16-bit delta encoding when the delta
exceeds the i8 range but fits in i16.

Refs: #42
```

```
fix(context): prevent hash collision on sync

The hash calculation was missing the pattern length,
causing potential collisions.

Refs: #57
```

### Tests

Chaque fonctionnalité doit avoir :
- Tests unitaires pour les cas nominaux
- Tests pour les cas limites
- Tests pour les cas d'erreur

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_nominal() {
        // Arrange
        let encoder = Encoder::new();
        let data = RawData::new(42.0, 0);
        
        // Act
        let result = encoder.encode(&data);
        
        // Assert
        assert!(result.len() > 0);
    }

    #[test]
    fn test_encode_edge_case_zero() {
        let encoder = Encoder::new();
        let data = RawData::new(0.0, 0);
        
        let result = encoder.encode(&data);
        
        // Vérifier le comportement spécifique
    }

    #[test]
    fn test_encode_error_invalid_input() {
        let encoder = Encoder::new();
        let data = RawData::new(f64::NAN, 0);
        
        let result = encoder.try_encode(&data);
        
        assert!(result.is_err());
    }
}
```

---

## Processus de PR

### Avant de soumettre

- [ ] Le code compile sans warnings (`cargo build`)
- [ ] Le linter passe (`cargo clippy -- -D warnings`)
- [ ] Le code est formaté (`cargo fmt --check`)
- [ ] Les tests passent (`cargo test`)
- [ ] La couverture n'a pas diminué
- [ ] La documentation est à jour

### Template de PR

```markdown
## Description

Brève description du changement.

## Type de changement

- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation

## Checklist

- [ ] J'ai lu le CONTRIBUTING.md
- [ ] J'ai ajouté des tests
- [ ] J'ai mis à jour la documentation
- [ ] Mes commits suivent les conventions

## Issues liées

Fixes #123

## Screenshots (si applicable)

## Notes pour les reviewers
```

### Processus de review

1. **Automated checks** : CI vérifie build, tests, linting
2. **Code review** : Au moins 1 approbation requise
3. **Discussion** : Les commentaires doivent être résolus
4. **Merge** : Squash and merge par un maintainer

### Après le merge

- Votre branche sera supprimée automatiquement
- Le changement apparaîtra dans le prochain CHANGELOG
- Vous serez crédité dans les contributors

---

## Templates disponibles

Pour vous aider à structurer vos contributions, utilisez nos templates :

| Template | Usage | Chemin |
|----------|-------|--------|
| Feature | Nouvelle fonctionnalité | `prompts/feature.prompt.md` |
| Bugfix | Correction de bug | `prompts/bugfix.prompt.md` |
| Refactor | Amélioration du code | `prompts/refactor.prompt.md` |
| Security | Audit de sécurité | `prompts/security-review.prompt.md` |
| Tests | Tests de non-régression | `prompts/non-regression.prompt.md` |

### Utilisation avec Claude/LLM

Ces templates sont conçus pour être utilisés avec un assistant IA :

1. Copiez le contenu du template approprié
2. Remplissez les sections demandées
3. Soumettez à l'assistant
4. Utilisez la réponse structurée pour votre PR

### Exemples concrets

Consultez le dossier `examples/` pour voir des workflows complets :

- `01-feature-iteration.md` : Ajout d'une feature
- `02-refactor.md` : Refactoring guidé
- `03-bugfix.md` : Correction de bug

---

## Reconnaissance

Les contributeurs sont reconnus de plusieurs façons :

- **AUTHORS** : Liste des contributeurs dans le fichier AUTHORS
- **CHANGELOG** : Mention dans les notes de release
- **README** : Contributeurs majeurs dans les remerciements

---

## Questions ?

- 📖 Consultez la [FAQ](docs/faq.md)
- 💬 Ouvrez une issue avec le tag "question"
- 📧 Contactez les maintainers

Merci de contribuer à ALEC ! 🙏
