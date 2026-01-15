# ALEC — Adaptive Lazy Evolving Compression

## Vision

ALEC est un codec de compression hybride qui combine deux approches complémentaires :
- **Compression paresseuse (Lazy)** : transmettre la décision avant la donnée
- **Contexte partagé évolutif (Evolving)** : construire un dictionnaire commun au fil du temps

L'objectif : maximiser l'information utile transmise par bit, particulièrement dans les environnements contraints (IoT, capteurs autonomes, liaisons satellites, zones à faible connectivité).

---

## Principes fondamentaux

### 1. Économie du bit
Chaque donnée a un coût de transmission. ALEC optimise le ratio valeur/coût en permanence.

### 2. Asymétrie encodeur/décodeur
- **Mode Émetteur Léger** : encodage simple, décodage complexe (capteurs, drones, sondes)
- **Mode Récepteur Léger** : encodage complexe, décodage simple (diffusion vers appareils bas de gamme)

### 3. Transmission sur demande
Les données de détail ne sont transmises que si le récepteur les demande explicitement.

### 4. Apprentissage continu
Le dictionnaire partagé s'enrichit avec chaque échange, réduisant progressivement la bande passante nécessaire.

---

## Architecture globale

```
┌─────────────────────────────────────────────────────────────────┐
│                         ÉMETTEUR                                │
│                                                                 │
│  ┌──────────┐    ┌─────────────┐    ┌──────────┐    ┌────────┐ │
│  │ Source   │───▶│ Classifieur │───▶│ Encodeur │───▶│ Buffer │──────▶ Canal
│  │ Données  │    │ Priorité    │    │ Delta    │    │ Sortie │ │
│  └──────────┘    └──────┬──────┘    └────┬─────┘    └────────┘ │
│                         │                │                      │
│                         ▼                ▼                      │
│              ┌─────────────────────────────────────┐           │
│              │      Contexte Partagé Local         │           │
│              │  ┌─────────────┐ ┌───────────────┐  │           │
│              │  │ Dictionnaire│ │ Modèle        │  │           │
│              │  │ Patterns    │ │ Prédictif     │  │           │
│              │  └─────────────┘ └───────────────┘  │           │
│              └─────────────────────────────────────┘           │
│                              ▲                                  │
│                              │ sync                             │
└──────────────────────────────┼──────────────────────────────────┘
                               │
                    ═══════════╪═══════════  Canal bidirectionnel
                               │
                               │ requêtes + sync
┌──────────────────────────────┼──────────────────────────────────┐
│                              ▼                                  │
│              ┌─────────────────────────────────────┐           │
│              │      Contexte Partagé Local         │           │
│              │  ┌─────────────┐ ┌───────────────┐  │           │
│              │  │ Dictionnaire│ │ Modèle        │  │           │
│              │  │ Patterns    │ │ Prédictif     │  │           │
│              │  └─────────────┘ └───────────────┘  │           │
│              └─────────────────────────────────────┘           │
│                         │                │                      │
│                         ▼                ▼                      │
│  ┌────────┐    ┌──────────┐    ┌─────────────┐    ┌──────────┐ │
│  │ Buffer │───▶│ Décodeur │───▶│ Gestionnaire│───▶│ Appli-   │ │
│  │ Entrée │    │ Delta    │    │ Requêtes    │    │ cation   │ │
│  └────────┘    └──────────┘    └─────────────┘    └──────────┘ │
│                                                                 │
│                         RÉCEPTEUR                               │
└─────────────────────────────────────────────────────────────────┘
```

---

## Composants détaillés

### 1. Classifieur de Priorité

Attribue à chaque donnée un niveau de priorité :

| Niveau | Nom | Comportement | Exemple |
|--------|-----|--------------|---------|
| P1 | CRITIQUE | Envoi immédiat, accusé réception | Alerte incendie |
| P2 | IMPORTANT | Envoi immédiat, sans accusé | Anomalie détectée |
| P3 | NORMAL | Envoi si bande passante disponible | Mesure périodique |
| P4 | DIFFÉRÉ | Stocké localement, envoi sur demande | Historique détaillé |
| P5 | JETABLE | Jamais envoyé sauf demande explicite | Debug, logs verbeux |

**Critères de classification :**
- Écart à la valeur prédite (delta)
- Seuils configurables par type de donnée
- Urgence temporelle
- Coût de non-transmission

### 2. Contexte Partagé

Structure de données synchronisée entre émetteur et récepteur.

```
ContextePartagé {
    dictionnaire: Map<Pattern, CodeCourt>
    modele_predictif: ModeleStatistique
    historique_recent: CircularBuffer<Mesure>
    metadata: {
        version: u32
        derniere_sync: Timestamp
        hash_verification: u64
    }
}
```

**Évolution du dictionnaire :**
1. Comptage de fréquence des patterns
2. Promotion : pattern fréquent → code plus court
3. Élagage : pattern rare → suppression
4. Synchronisation périodique (différentielle)

### 3. Encodeur Delta

Encode uniquement la différence avec ce que le récepteur "sait déjà".

**Stratégies d'encodage :**
- **Delta numérique** : valeur - valeur_prédite
- **Delta symbolique** : référence au pattern le plus proche
- **Delta temporel** : écart au rythme habituel

**Format de message :**
```
┌─────────┬──────────┬─────────┬─────────────┐
│ Header  │ Priorité │ Type    │ Payload     │
│ 1 octet │ 3 bits   │ 5 bits  │ variable    │
└─────────┴──────────┴─────────┴─────────────┘
```

### 4. Gestionnaire de Requêtes

Côté récepteur, gère les demandes de données complémentaires.

**Types de requêtes :**
- `REQ_DETAIL` : demande les données P4/P5 associées à un événement
- `REQ_RANGE` : demande un historique temporel
- `REQ_RESYNC` : force une resynchronisation du contexte

---

## Flux de données

### Flux nominal (mesure normale)

```
1. Capteur génère mesure M
2. Classifieur compare M au modèle prédictif
3. Delta faible → P3 (NORMAL)
4. Encodeur : code_court + delta_minimal
5. Envoi : 4 octets au lieu de 60
6. Récepteur décode via contexte partagé
7. Mise à jour du modèle prédictif (deux côtés)
```

### Flux anomalie

```
1. Capteur génère mesure M
2. Classifieur détecte écart significatif
3. Delta élevé → P2 (IMPORTANT)
4. Encodeur : type_anomalie + valeur_brute + timestamp
5. Envoi immédiat : 12 octets
6. Récepteur reçoit, peut demander REQ_DETAIL
7. Si demande : envoi données P4 associées
```

### Flux synchronisation

```
1. Timer déclenche sync (ou seuil de divergence)
2. Émetteur calcule diff du dictionnaire
3. Envoi : hash_attendu + ajouts + suppressions
4. Récepteur vérifie cohérence
5. Si incohérence : REQ_RESYNC complet
```

---

## Modes de fonctionnement

### Mode Autonome
Un émetteur, un récepteur. Configuration par défaut.

### Mode Flotte
Plusieurs émetteurs partagent un contexte commun avec un récepteur central. Le contexte "apprend" des patterns de toute la flotte.

### Mode Mesh
Plusieurs nœuds qui peuvent être émetteurs et récepteurs. Contexte distribué avec synchronisation pair-à-pair.

---

## Métriques clés

| Métrique | Description | Cible |
|----------|-------------|-------|
| Ratio de compression | Taille transmise / taille brute | < 0.1 après rodage |
| Latence P1 | Temps alerte → réception | < 100ms |
| Taux de requêtes | % de P4 effectivement demandés | < 20% |
| Divergence contexte | Écart entre dictionnaires | < 1% |
| Énergie/bit utile | mJ par bit d'information transmis | Minimiser |

---

## Contraintes techniques

### Côté émetteur (léger)
- RAM : < 64 KB pour le contexte
- CPU : Compatible microcontrôleurs (ARM Cortex-M0+)
- Pas de dépendances externes lourdes

### Côté récepteur (peut être lourd)
- Pas de contrainte stricte
- Peut utiliser ML pour améliorer les prédictions
- Stockage historique illimité

---

## Évolutions futures

1. **v0.1** : Prototype fonctionnel, dictionnaire statique
2. **v0.2** : Dictionnaire évolutif, sync manuelle
3. **v0.3** : Sync automatique, mode flotte
4. **v1.0** : Production-ready, mode mesh, ML côté récepteur
