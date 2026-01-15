# ALEC — Charte graphique

## Identité visuelle

### Nom et logo

**ALEC** — Adaptive Lazy Evolving Compression

Le logo représente deux cercles qui se chevauchent, symbolisant le contexte partagé entre émetteur et récepteur, avec une flèche légère suggérant la transmission minimale.

```
     ╭───────╮
    ╱  ╭─────┼──╮
   │   │  ◄  │  │
    ╲  ╰─────┼──╯
     ╰───────╯
    ALEC
```

### Palette de couleurs

#### Couleurs principales

| Nom | Hex | Usage |
|-----|-----|-------|
| ALEC Blue | `#2563EB` | Couleur principale, liens, boutons |
| ALEC Dark | `#1E3A5F` | Texte principal, headers |
| ALEC Light | `#EFF6FF` | Fonds, zones de code |

#### Couleurs fonctionnelles

| Nom | Hex | Usage |
|-----|-----|-------|
| Success Green | `#10B981` | Confirmations, sync OK |
| Warning Amber | `#F59E0B` | Alertes P2, avertissements |
| Critical Red | `#EF4444` | Alertes P1, erreurs |
| Neutral Gray | `#6B7280` | Texte secondaire |

#### Couleurs de priorité

| Priorité | Couleur | Hex |
|----------|---------|-----|
| P1 CRITIQUE | Rouge vif | `#DC2626` |
| P2 IMPORTANT | Orange | `#EA580C` |
| P3 NORMAL | Bleu | `#2563EB` |
| P4 DIFFÉRÉ | Gris | `#9CA3AF` |
| P5 JETABLE | Gris clair | `#D1D5DB` |

---

## Typographie

### Polices

| Usage | Police | Fallback |
|-------|--------|----------|
| Titres | Inter Bold | system-ui, sans-serif |
| Corps de texte | Inter Regular | system-ui, sans-serif |
| Code | JetBrains Mono | Consolas, monospace |
| Données/métriques | JetBrains Mono | Consolas, monospace |

### Hiérarchie

```css
/* Titres */
h1 { font-size: 2rem; font-weight: 700; color: #1E3A5F; }
h2 { font-size: 1.5rem; font-weight: 600; color: #1E3A5F; }
h3 { font-size: 1.25rem; font-weight: 600; color: #374151; }

/* Corps */
body { font-size: 1rem; line-height: 1.6; color: #374151; }

/* Code */
code { font-size: 0.875rem; background: #EFF6FF; padding: 0.125rem 0.25rem; }
```

---

## Composants UI

### Badges de priorité

```
┌──────────────────────────────────────────────────────────┐
│                                                          │
│  [P1] CRITIQUE   Rouge, texte blanc, icône alerte       │
│                                                          │
│  [P2] IMPORTANT  Orange, texte blanc                    │
│                                                          │
│  [P3] NORMAL     Bleu, texte blanc                      │
│                                                          │
│  [P4] DIFFÉRÉ    Gris, texte sombre, bordure pointillée │
│                                                          │
│  [P5] JETABLE    Gris clair, texte gris                 │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

### Indicateurs d'état

```
┌─────────────────────────────────────────┐
│                                         │
│  ● Connecté     (vert pulsant)         │
│                                         │
│  ● Sync OK      (vert fixe)            │
│                                         │
│  ● Sync en cours (bleu pulsant)        │
│                                         │
│  ● Désynchronisé (orange clignotant)   │
│                                         │
│  ● Déconnecté   (rouge fixe)           │
│                                         │
└─────────────────────────────────────────┘
```

### Cards de métriques

```
┌─────────────────────────────────────────────────────────┐
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │ Compression │  │  Messages   │  │   Contexte  │     │
│  │             │  │             │  │             │     │
│  │    92%      │  │   1,234     │  │    v42      │     │
│  │   ▲ +3%     │  │   /jour     │  │  128 patterns│    │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
│                                                         │
│  Fond blanc, ombre légère, coins arrondis (8px)        │
└─────────────────────────────────────────────────────────┘
```

---

## Visualisations

### Graphique d'évolution du contexte

```
Dictionnaire partagé
│
│    ████████████████████████████  pattern_A (45%)
│    ███████████████               pattern_B (28%)
│    ████████                      pattern_C (15%)
│    ████                          pattern_D (8%)
│    ██                            autres (4%)
│
└────────────────────────────────────────────────
                    Fréquence
```

Couleurs : dégradé du bleu principal vers gris

### Timeline des messages

```
Temps ─────────────────────────────────────────────────▶

  ●────●────●────●────◆────●────●────●────●────●────●
  P3   P3   P3   P3   P2   P3   P3   P3   P3   P3   P3
                      │
                      └── Anomalie détectée
                          (agrandi, couleur orange)
```

### Flux de données en temps réel

```
┌─────────────────────────────────────────────────────────┐
│                                                         │
│  Émetteur                               Récepteur       │
│     ○                                      ○            │
│     │                                      │            │
│     │  ════▶ [P3] 4 octets ════▶          │            │
│     │                                      │            │
│     │  ════▶ [P2] 12 octets ════▶         │            │
│     │                                      │            │
│     │  ◀════ [REQ_DETAIL] ◀════           │            │
│     │                                      │            │
│     │  ════▶ [P4] 128 octets ════▶        │            │
│     │                                      │            │
│                                                         │
│  Animation : messages qui traversent de gauche à droite │
│  Taille proportionnelle au nombre d'octets              │
│  Couleur selon priorité                                 │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

---

## Documentation technique

### Blocs de code

```
┌─────────────────────────────────────────────────────────┐
│ rust                                              [copy]│
├─────────────────────────────────────────────────────────┤
│                                                         │
│  fn encode_delta(&self, value: f64) -> Vec<u8> {       │
│      let predicted = self.context.predict();            │
│      let delta = value - predicted;                     │
│      self.compress_delta(delta)                         │
│  }                                                      │
│                                                         │
└─────────────────────────────────────────────────────────┘

Fond: #EFF6FF
Bordure: #BFDBFE
Header: #DBEAFE avec texte #1E3A5F
```

### Diagrammes d'architecture

Style : lignes simples, pas de 3D, pas de dégradés

```
Couleurs des éléments :
- Composants émetteur : #DBEAFE (bleu clair)
- Composants récepteur : #D1FAE5 (vert clair)
- Canal de communication : #FEF3C7 (jaune clair)
- Contexte partagé : #E0E7FF (indigo clair)
- Flèches : #6B7280 (gris)
- Texte : #1F2937 (gris foncé)
```

### Tableaux

```
┌─────────────────────────────────────────────────────────┐
│  Header fond : #EFF6FF                                  │
│  Header texte : #1E3A5F, bold                          │
│  Lignes alternées : blanc / #F9FAFB                    │
│  Bordures : #E5E7EB                                    │
│  Padding cellules : 12px horizontal, 8px vertical      │
└─────────────────────────────────────────────────────────┘
```

---

## Icônes

Jeu d'icônes recommandé : **Lucide** (open source, cohérent)

| Concept | Icône |
|---------|-------|
| Émetteur | `radio` |
| Récepteur | `antenna` |
| Contexte | `database` |
| Synchronisation | `refresh-cw` |
| Compression | `minimize-2` |
| Alerte | `alert-triangle` |
| Succès | `check-circle` |
| Erreur | `x-circle` |
| Connexion | `wifi` |
| Déconnexion | `wifi-off` |

---

## Responsive

### Breakpoints

| Nom | Largeur | Usage |
|-----|---------|-------|
| Mobile | < 640px | Dashboard simplifié |
| Tablet | 640px - 1024px | Dashboard complet, sidebar masquée |
| Desktop | > 1024px | Dashboard complet avec sidebar |

### Adaptation mobile

- Métriques empilées verticalement
- Timeline horizontale scrollable
- Graphiques simplifiés
- Navigation par tabs en bas d'écran

---

## Accessibilité

### Contraste

Tous les textes respectent WCAG AA :
- Texte normal : ratio minimum 4.5:1
- Texte large (> 18px) : ratio minimum 3:1

### États interactifs

```css
/* Focus visible */
:focus-visible {
  outline: 2px solid #2563EB;
  outline-offset: 2px;
}

/* Hover */
button:hover {
  background: #1D4ED8; /* plus sombre */
}
```

### Alternatives textuelles

- Toutes les icônes ont un `aria-label`
- Les graphiques ont une description textuelle
- Les couleurs ne sont jamais le seul indicateur (formes, textes)
