# Prompt Template — Refactoring

## Instructions pour l'IA

Tu es un développeur expert chargé d'améliorer la qualité du code ALEC sans changer son comportement externe. Avant de commencer :

1. Lis `/docs/architecture.md` pour comprendre le contexte
2. Lis `/docs/non-regression.md` pour les tests existants
3. **Règle d'or** : Les tests existants doivent passer sans modification

---

## Contexte du refactoring

### Type de refactoring
<!-- Cocher le type principal -->

- [ ] **Extraction** : Extraire du code en fonctions/modules
- [ ] **Simplification** : Réduire la complexité cyclomatique
- [ ] **Performance** : Optimiser sans changer l'API
- [ ] **Lisibilité** : Renommer, réorganiser, commenter
- [ ] **Découplage** : Réduire les dépendances entre modules
- [ ] **Standardisation** : Aligner sur les conventions du projet

### Zone concernée

```
Fichier(s) : 
Fonction(s) : 
Lignes approximatives : 
```

### Motivation

```
Problème actuel : [Décrire le problème de qualité]

Impact : [Maintenabilité, performance, lisibilité...]

Objectif : [Ce qu'on veut atteindre]
```

---

## Contraintes du refactoring

### Invariants à préserver

- [ ] API publique inchangée
- [ ] Format des messages binaires inchangé
- [ ] Comportement du contexte partagé inchangé
- [ ] Performance égale ou meilleure
- [ ] Consommation mémoire égale ou meilleure

### Ce qui peut changer

- [ ] Structure interne des modules
- [ ] Noms des éléments privés
- [ ] Implémentation des algorithmes (si même résultat)
- [ ] Organisation des fichiers

---

## Format de réponse attendu

### 1. Diagnostic

```markdown
## Diagnostic

### Code smells identifiés
- [Smell 1] : Ligne X — [description]
- [Smell 2] : Ligne Y — [description]

### Métriques actuelles
- Complexité cyclomatique : X
- Lignes de code : Y
- Couplage : [description]

### Métriques cibles
- Complexité cyclomatique : X' (objectif < 10 par fonction)
- Lignes de code : Y'
- Couplage : [objectif]
```

### 2. Plan de refactoring

```markdown
## Plan

### Étape 1 : [Nom]
- Action : [description]
- Risque : [faible/moyen/élevé]
- Test de validation : [quel test vérifie que ça marche]

### Étape 2 : [Nom]
...

### Ordre d'exécution
[Justifier l'ordre des étapes]
```

### 3. Code refactorisé

```markdown
## Implémentation

### Avant (pour référence)
```rust
// Code original (ne pas modifier ce bloc)
```

### Après
```rust
// Code refactorisé
```

### Diff conceptuel
[Expliquer les changements majeurs]
```

### 4. Validation

```markdown
## Validation

### Tests existants
- [ ] `test_xxx` : passe
- [ ] `test_yyy` : passe

### Nouveaux tests (si nécessaire)
[Code des tests ajoutés pour couvrir les nouveaux chemins internes]

### Vérification manuelle
[Étapes pour vérifier manuellement si nécessaire]
```

---

## Techniques de refactoring autorisées

### Extraction

```rust
// AVANT
fn process() {
    // 50 lignes de code
    // dont 20 lignes de validation
}

// APRÈS
fn process() {
    validate()?;
    // 30 lignes de code
}

fn validate() -> Result<(), Error> {
    // 20 lignes de validation
}
```

### Remplacement de conditionnel par polymorphisme

```rust
// AVANT
fn encode(data: &Data, mode: Mode) -> Vec<u8> {
    match mode {
        Mode::Raw => encode_raw(data),
        Mode::Delta => encode_delta(data),
        Mode::Pattern => encode_pattern(data),
    }
}

// APRÈS
trait Encoder {
    fn encode(&self, data: &Data) -> Vec<u8>;
}

struct RawEncoder;
struct DeltaEncoder;
struct PatternEncoder;

impl Encoder for RawEncoder { ... }
impl Encoder for DeltaEncoder { ... }
impl Encoder for PatternEncoder { ... }
```

### Inversion de dépendance

```rust
// AVANT (couplage fort)
struct Classifier {
    context: Context,  // Dépendance concrète
}

// APRÈS (découplage)
struct Classifier<C: IContext> {
    context: C,  // Dépendance abstraite
}
```

---

## Anti-patterns à éviter

❌ **Refactoring big bang** : Tout changer d'un coup
✅ **Refactoring incrémental** : Petits changements validés un par un

❌ **Refactoring spéculatif** : "On pourrait avoir besoin de..."
✅ **Refactoring justifié** : Résout un problème concret identifié

❌ **Changement de comportement déguisé** : "J'ai aussi corrigé ce bug"
✅ **Refactoring pur** : Même comportement, meilleure structure

---

## Checklist avant soumission

- [ ] Tous les tests existants passent (sans modification)
- [ ] Aucune API publique modifiée
- [ ] Le code compile sans nouveaux warnings
- [ ] La complexité a diminué (ou justification si non)
- [ ] Les noms sont plus clairs qu'avant
- [ ] Le diff est aussi petit que possible pour le changement voulu

---

## Exemple de demande

```
REFACTORING: Simplifier la fonction classify()

Zone : src/classifier.rs, lignes 45-120
Type : Simplification + Extraction

Problème actuel : 
- Fonction de 75 lignes
- Complexité cyclomatique de 15
- 6 niveaux d'indentation max

Objectif :
- Maximum 30 lignes par fonction
- Complexité < 10
- Maximum 3 niveaux d'indentation

Invariants :
- [x] API publique inchangée
- [x] Même classification pour mêmes entrées
```
