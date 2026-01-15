# ALEC — Questions fréquentes (FAQ)

---

## Général

### Qu'est-ce qui différencie ALEC des autres codecs de compression ?

ALEC combine trois approches innovantes :

1. **Compression paresseuse** : Contrairement aux codecs classiques qui compressent tout, ALEC décide d'abord *si* une donnée mérite d'être transmise, puis *comment* la transmettre.

2. **Contexte évolutif** : Le dictionnaire de compression s'enrichit automatiquement avec le temps, contrairement aux codecs statiques.

3. **Asymétrie configurable** : L'effort de compression peut être placé côté émetteur ou récepteur selon les contraintes.

### ALEC est-il avec ou sans perte ?

**Sans perte** pour les valeurs numériques. Les données reconstruites sont identiques aux données originales (à la précision configurée près).

Cependant, ALEC peut *décider de ne pas transmettre* certaines données (P4, P5). Ce n'est pas une perte de compression mais une décision de filtrage. Ces données restent disponibles sur demande.

### Quelle compression puis-je espérer ?

Cela dépend fortement des données et du temps d'apprentissage :

| Situation | Ratio typique |
|-----------|---------------|
| Données aléatoires | 0.8-1.0 (peu de gain) |
| Premier jour (apprentissage) | 0.5-0.7 |
| Après une semaine | 0.1-0.3 |
| Données très prévisibles | 0.02-0.08 |

### Quelles sont les limitations d'ALEC ?

- **Données aléatoires** : Peu de gain si les valeurs sont imprévisibles
- **Latence** : L'approche paresseuse ajoute potentiellement un aller-retour pour les détails
- **Mémoire** : Le contexte partagé consomme de la RAM (configurable, typiquement 16-64 KB)
- **Apprentissage** : L'efficacité optimale nécessite une période de rodage

---

## Technique

### Quels langages sont supportés ?

Actuellement :
- **Rust** (implémentation principale, émetteur et récepteur)
- **C** (en cours, pour émetteurs très contraints)

Prévu :
- Python (bindings pour prototypage)
- JavaScript (décodeur côté navigateur)

### ALEC fonctionne-t-il sur microcontrôleur ?

Oui, l'émetteur est conçu pour fonctionner sur des microcontrôleurs type ARM Cortex-M0+ avec :
- < 64 KB de RAM
- < 128 KB de Flash
- Pas de système d'exploitation requis (`no_std`)

### Quels protocoles de transport sont supportés ?

ALEC est agnostique du transport. Il fonctionne sur :
- MQTT / MQTT-SN
- CoAP
- HTTP/WebSocket
- TCP/UDP brut
- LoRaWAN
- Liaison série

### Comment fonctionne la synchronisation du contexte ?

1. **Initialisation** : L'émetteur envoie son contexte complet
2. **Incrémental** : Périodiquement, seuls les changements (diff) sont envoyés
3. **Vérification** : Chaque message contient la version du contexte utilisé
4. **Recovery** : En cas de désynchronisation, resync automatique

### Le contexte peut-il être pré-chargé ?

Oui ! Pour accélérer le démarrage, vous pouvez :
- Exporter le contexte d'un émetteur existant
- Le charger sur un nouvel émetteur
- Bénéficier immédiatement de la compression optimale

```rust
// Exporter
let export = context.export_full();
save_to_flash(&export);

// Importer
let loaded = load_from_flash();
context.import(&loaded);
```

### Que se passe-t-il si un message est perdu ?

- **Messages P1** : Retransmis jusqu'à acquittement
- **Messages P2-P3** : Le récepteur détecte le gap via le numéro de séquence et peut demander retransmission
- **Contexte** : Si la désynchronisation est détectée, resync automatique

Les données P4/P5 non envoyées ne sont pas concernées.

---

## Sécurité

### Les données sont-elles chiffrées ?

ALEC ne chiffre pas lui-même les données. Il est conçu pour être encapsulé dans :
- TLS 1.3 (connexions TCP)
- DTLS 1.3 (connexions UDP)

Le chiffrement est ainsi délégué à des protocoles éprouvés.

### Le contexte partagé est-il un risque de sécurité ?

Le contexte contient des patterns statistiques, pas les données elles-mêmes. Cependant :
- Un attaquant avec accès au contexte pourrait inférer certaines informations
- Il est recommandé de protéger la synchronisation du contexte (authentification, chiffrement)

### Comment protéger contre le rejeu de messages ?

- Les numéros de séquence détectent les duplications
- Les timestamps permettent de rejeter les messages trop anciens
- Pour une protection renforcée, utilisez DTLS avec anti-replay

---

## Performance

### Quelle latence ajoute ALEC ?

- **Encodage** : < 1 ms pour une valeur simple
- **Décodage** : < 0.5 ms pour une valeur simple
- **Classification** : < 0.1 ms

La latence principale vient du transport, pas d'ALEC.

### Combien de mémoire consomme le contexte ?

Configuration par défaut :
- **Émetteur** : ~32 KB
- **Récepteur** : ~1 MB (stocke aussi l'historique)

Configurable selon les contraintes :
```rust
let context = Context::builder()
    .max_patterns(100)      // Limite le dictionnaire
    .max_memory_kb(16)      // Limite stricte
    .build();
```

### ALEC supporte-t-il le multithreading ?

Le contexte n'est pas thread-safe par défaut. Options :
- Un contexte par thread
- Wrapper avec mutex
- Version `Send + Sync` disponible avec le feature `threadsafe`

---

## Cas d'usage

### ALEC est-il adapté au streaming vidéo ?

Non. ALEC est optimisé pour :
- Données de capteurs (valeurs numériques)
- Séries temporelles
- Données discrètes et structurées

Pour la vidéo, utilisez H.264, H.265, AV1, etc.

### Puis-je utiliser ALEC pour des données binaires (images, fichiers) ?

Ce n'est pas son usage principal, mais c'est possible :
- Les données binaires peuvent être traitées comme des patterns
- L'efficacité dépendra de la répétitivité des patterns

Pour la compression générique, préférez zstd, lz4, etc.

### ALEC fonctionne-t-il en temps réel ?

Oui, pour les messages P1 et P2 :
- Envoi immédiat dès classification
- Pas de buffering
- Latence prévisible

Les messages P3 peuvent être légèrement retardés (batching optionnel).

---

## Déploiement

### Comment mettre à jour ALEC sans perdre le contexte ?

1. Exporter le contexte avant mise à jour
2. Mettre à jour le firmware/logiciel
3. Importer le contexte sauvegardé

Si les versions sont compatibles, le contexte reste utilisable.

### Puis-je avoir plusieurs récepteurs pour un émetteur ?

Oui, mais chaque récepteur maintient son propre contexte. Options :
- Un récepteur principal qui redistribue
- Synchronisation du contexte entre récepteurs (avancé)

### Comment débugger une désynchronisation ?

1. Activer les logs détaillés (`ALEC_LOG=debug`)
2. Vérifier les hash de contexte des deux côtés
3. Comparer les versions
4. Forcer une resync complète si nécessaire

```bash
ALEC_LOG=debug cargo run --example emitter
```

---

## Contribution

### Comment signaler un bug ?

1. Vérifier qu'il n'existe pas déjà dans les issues
2. Créer une issue avec :
   - Version ALEC
   - Environnement (OS, hardware)
   - Étapes de reproduction
   - Comportement attendu vs observé

### Comment proposer une fonctionnalité ?

1. Ouvrir une issue "Feature request"
2. Décrire le cas d'usage
3. Expliquer pourquoi les solutions existantes ne suffisent pas
4. Proposer une approche (optionnel)

### Le projet accepte-t-il les contributions ?

Oui ! Voir [CONTRIBUTING.md](../CONTRIBUTING.md) pour :
- Les conventions de code
- Le processus de PR
- Les templates disponibles

---

## Licence et usage commercial

### Quelle est la licence d'ALEC ?

MIT License — vous pouvez :
- Utiliser commercialement
- Modifier
- Distribuer
- Utiliser en privé

Sans garantie, avec attribution requise.

### Puis-je utiliser ALEC dans un produit commercial ?

Oui, la licence MIT le permet. Mentionnez simplement ALEC dans vos attributions.

---

## Questions non résolues ?

- 📖 Consultez la [documentation complète](../README.md)
- 💬 Ouvrez une [issue sur GitHub](https://github.com/votre-org/alec-codec/issues)
- 📧 Contactez les mainteneurs
