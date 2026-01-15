# Prompt Template — Correction de bug

## Instructions pour l'IA

Tu es un développeur expert chargé de diagnostiquer et corriger un bug dans ALEC. Approche méthodique requise :

1. **Reproduire** avant de corriger
2. **Comprendre** la cause racine
3. **Corriger** de manière minimale
4. **Tester** que le fix fonctionne
5. **Vérifier** qu'il n'y a pas de régression

---

## Description du bug

### Symptôme observé

```
Comportement attendu : 
[Ce qui devrait se passer]

Comportement actuel : 
[Ce qui se passe réellement]

Fréquence : [ ] Toujours  [ ] Intermittent  [ ] Rare
```

### Contexte de reproduction

```
Version ALEC : 
Environnement : [OS, hardware, configuration]
Étapes pour reproduire :
1. 
2. 
3. 

Données d'entrée (si applicable) :
[Joindre ou décrire les données]
```

### Logs et traces

```
[Coller les logs pertinents, stack traces, messages d'erreur]
```

### Impact

- [ ] **Critique** : Perte de données, crash, sécurité
- [ ] **Majeur** : Fonctionnalité cassée, contournement difficile
- [ ] **Mineur** : Gêne, contournement facile
- [ ] **Cosmétique** : Affichage, UX

---

## Format de réponse attendu

### 1. Reproduction

```markdown
## Reproduction

### Test de reproduction
```rust
#[test]
fn test_reproduces_bug_xxx() {
    // Setup qui déclenche le bug
    // Assert qui échoue actuellement
}
```

### Résultat
- [ ] Bug reproduit avec ce test
- [ ] Bug non reproductible (demander plus d'infos)
```

### 2. Analyse

```markdown
## Analyse de la cause racine

### Localisation
Fichier : `src/xxx.rs`
Fonction : `yyy()`
Ligne(s) : 42-45

### Cause racine
[Explication détaillée de pourquoi le bug se produit]

### Chaîne causale
1. [Condition initiale]
2. [Ce qui en découle]
3. [Ce qui cause le symptôme]

### Pourquoi ça n'a pas été détecté
[Tests manquants, cas limite non prévu, etc.]
```

### 3. Correction

```markdown
## Correction proposée

### Option A (recommandée)
```rust
// Code corrigé
```
Avantages : [...]
Inconvénients : [...]

### Option B (alternative)
```rust
// Code alternatif
```
Avantages : [...]
Inconvénients : [...]

### Justification du choix
[Pourquoi l'option A est préférée]
```

### 4. Tests

```markdown
## Tests

### Test de non-régression
```rust
#[test]
fn test_bug_xxx_fixed() {
    // Le test de reproduction doit maintenant passer
}
```

### Tests supplémentaires
```rust
#[test]
fn test_edge_case_related_to_xxx() {
    // Cas limites similaires à vérifier
}
```

### Tests existants impactés
- [ ] Aucun test existant modifié
- [ ] Test `test_yyy` modifié : [justification]
```

### 5. Prévention

```markdown
## Prévention

### Comment éviter ce type de bug à l'avenir
- [ ] Ajouter validation d'entrée
- [ ] Ajouter assertion/invariant
- [ ] Améliorer la documentation
- [ ] Ajouter test dans la CI

### Autres endroits potentiellement affectés
[Lister les zones de code similaires à vérifier]
```

---

## Patterns de bugs courants dans ALEC

### Bug de synchronisation de contexte

```rust
// SYMPTÔME : Messages indécodables après un certain temps
// CAUSE TYPIQUE : Divergence du dictionnaire

// VÉRIFICATIONS :
// 1. Hash de contexte dans les messages
// 2. Gestion des messages out-of-order
// 3. Timeout de resync
```

### Bug d'encodage/décodage

```rust
// SYMPTÔME : Valeurs incorrectes après décodage
// CAUSE TYPIQUE : Mismatch encodeur/décodeur

// VÉRIFICATIONS :
// 1. Endianness
// 2. Taille des types (i8 vs i16, etc.)
// 3. Gestion des valeurs négatives dans les deltas
```

### Bug de classification

```rust
// SYMPTÔME : Mauvaise priorité assignée
// CAUSE TYPIQUE : Seuils mal configurés ou comparaison incorrecte

// VÉRIFICATIONS :
// 1. Comparaisons avec flottants (epsilon)
// 2. Ordre des conditions
// 3. Gestion des valeurs NULL/NaN
```

### Bug de mémoire (émetteur contraint)

```rust
// SYMPTÔME : Crash ou comportement erratique
// CAUSE TYPIQUE : Dépassement de buffer, leak

// VÉRIFICATIONS :
// 1. Taille des allocations
// 2. Libération des buffers
// 3. Stack overflow (récursion)
```

---

## Checklist avant soumission

### Correction

- [ ] Le test de reproduction échoue AVANT le fix
- [ ] Le test de reproduction passe APRÈS le fix
- [ ] Le fix est minimal (pas de changements non liés)
- [ ] Le fix ne casse pas d'autres tests

### Qualité

- [ ] Code review effectuée
- [ ] Pas de TODO/FIXME laissés
- [ ] Commentaire explicatif si code non évident

### Documentation

- [ ] Changelog mis à jour (si version release)
- [ ] Bug tracker mis à jour
- [ ] Documentation mise à jour si comportement clarifié

---

## Exemple de rapport de bug

```
BUG: Delta encoding overflow pour grandes variations

Symptôme observé :
Comportement attendu : Delta encodé correctement
Comportement actuel : Valeur corrompue si delta > 127

Fréquence : [x] Toujours (quand delta > 127)

Étapes pour reproduire :
1. Contexte avec prediction = 100.0
2. Nouvelle valeur = 300.0
3. Delta = 200, overflow i8

Logs :
[WARN] Delta overflow: 200 truncated to -56

Impact : [x] Majeur (données corrompues)
```
