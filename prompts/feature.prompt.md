# Prompt Template — Nouvelle fonctionnalité

## Instructions pour l'IA

Tu es un développeur expert travaillant sur ALEC (Adaptive Lazy Evolving Compression). Avant de commencer, lis attentivement :

1. `/docs/architecture.md` — comprendre l'architecture globale
2. `/docs/intra-application.md` — comprendre les interfaces
3. `/docs/security.md` — si la feature touche à la sécurité
4. `/docs/non-regression.md` — pour les tests à écrire

---

## Contexte de la demande

### Fonctionnalité demandée
<!-- Décrire la fonctionnalité en 2-3 phrases -->

```
[TITRE DE LA FEATURE]

Description: ...

Motivation: Pourquoi cette feature est nécessaire ?
```

### Composants impactés
<!-- Cocher les composants concernés -->

- [ ] Classifieur de priorité
- [ ] Encodeur
- [ ] Décodeur
- [ ] Contexte partagé
- [ ] Canal de communication
- [ ] Gestionnaire de requêtes
- [ ] API externe
- [ ] Interface utilisateur

### Contraintes

- [ ] Doit fonctionner sur émetteur contraint (< 64KB RAM)
- [ ] Doit être rétrocompatible avec version précédente
- [ ] Impacte la sécurité (revue requise)
- [ ] Impacte les performances (benchmark requis)

---

## Format de réponse attendu

### 1. Analyse d'impact

```markdown
## Analyse

### Composants modifiés
- Composant A : [raison]
- Composant B : [raison]

### Risques identifiés
- Risque 1 : [description] → Mitigation : [solution]

### Dépendances
- Nécessite : [autre feature/lib]
- Bloque : [autre feature]
```

### 2. Spécification technique

```markdown
## Spécification

### Nouvelles structures de données
[Si applicable]

### Modifications d'interfaces
[Changements aux traits/interfaces existants]

### Nouveau flux de données
[Diagramme ou description du flux]
```

### 3. Implémentation

```markdown
## Implémentation

### Fichiers à créer
- `src/nouveau_fichier.rs` : [description]

### Fichiers à modifier
- `src/existant.rs` : [changements]

### Code
[Fournir le code complet, pas de placeholders]
```

### 4. Tests

```markdown
## Tests

### Tests unitaires
[Code des tests unitaires]

### Tests d'intégration
[Code des tests d'intégration]

### Scénarios de non-régression
[Référence aux scénarios à vérifier]
```

### 5. Documentation

```markdown
## Documentation

### Mise à jour de architecture.md
[Sections à modifier]

### Mise à jour de intra-application.md
[Nouvelles interfaces à documenter]
```

---

## Checklist avant soumission

- [ ] Le code compile sans warning
- [ ] Les tests passent
- [ ] La documentation est à jour
- [ ] Le code suit les conventions du projet
- [ ] Les métriques de performance sont respectées
- [ ] La rétrocompatibilité est assurée (si applicable)

---

## Exemple de demande complète

```
FEATURE: Support des capteurs multi-valeurs

Description: Permettre à un émetteur d'envoyer plusieurs valeurs 
dans un seul message (ex: température + humidité + pression).

Motivation: Réduire l'overhead des headers quand un capteur 
produit plusieurs métriques simultanément.

Composants impactés:
- [x] Encodeur
- [x] Décodeur
- [x] Contexte partagé (patterns multi-valeurs)

Contraintes:
- [x] Doit fonctionner sur émetteur contraint
- [x] Doit être rétrocompatible
```
