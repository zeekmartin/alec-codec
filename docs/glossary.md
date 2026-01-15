# ALEC — Glossaire

Définitions des termes utilisés dans la documentation ALEC.

---

## A

### ACK (Acknowledgment)
Message confirmant la bonne réception d'un autre message. Obligatoire pour les messages P1 (critiques).

### Anomalie
Valeur qui s'écarte significativement de la prédiction du contexte. Déclenche une classification P1 ou P2.

### Asymétrie
Principe où l'effort de calcul est réparti inégalement entre émetteur et récepteur selon les contraintes de chaque côté.

---

## B

### Bande passante
Capacité de transmission d'un canal, mesurée en bits par seconde (bps).

### Buffer
Zone de mémoire temporaire pour stocker des données en attente de traitement ou de transmission.

---

## C

### Canal
Médium de communication entre émetteur et récepteur (ex: LoRa, MQTT, liaison série).

### Classification
Processus d'attribution d'une priorité (P1-P5) à une donnée en fonction de son importance.

### Codec
Combinaison d'un **co**deur (encoder) et d'un **déc**odeur (decoder).

### Compression
Réduction de la taille des données tout en préservant l'information (avec ou sans perte).

### Contexte partagé
Structure de données maintenue de façon synchrone entre émetteur et récepteur, contenant le dictionnaire de patterns et le modèle prédictif.

---

## D

### Delta
Différence entre une valeur mesurée et sa prédiction. Permet une compression efficace quand les valeurs sont prévisibles.

### Dictionnaire
Collection de patterns fréquents associés à des codes courts. Partie du contexte partagé.

### Différé (P4)
Priorité pour les données qui ne sont pas transmises spontanément mais peuvent être demandées par le récepteur.

---

## E

### Émetteur
Entité qui produit et envoie des données compressées. Typiquement un capteur ou un appareil IoT.

### Encodage
Transformation des données brutes en format compact pour transmission.

### Évolutif
Caractéristique d'un système qui s'améliore avec le temps grâce à l'apprentissage.

---

## F

### Flotte
Ensemble d'émetteurs partageant un contexte commun avec un récepteur central.

### Fallback
Mécanisme de repli vers un mode dégradé mais fonctionnel en cas d'échec du mode optimal.

---

## G

### Golden file
Fichier de référence contenant la sortie attendue d'un test, utilisé pour la validation.

---

## H

### Hash
Empreinte numérique de taille fixe calculée à partir de données. Utilisé pour vérifier l'intégrité du contexte.

### Heartbeat
Message périodique indiquant que l'émetteur est toujours actif, même sans données à envoyer.

---

## I

### IoT (Internet of Things)
Réseau d'objets connectés équipés de capteurs et capables de communiquer.

---

## J

### Jetable (P5)
Priorité la plus basse. Données jamais transmises spontanément (logs, debug).

---

## L

### Latence
Délai entre l'émission et la réception d'un message.

### Lazy (Paresseux)
Approche où les données complètes ne sont transmises que si nécessaire, après une première notification.

### LoRa / LoRaWAN
Technologie de communication sans fil longue portée et faible consommation, typique de l'IoT.

---

## M

### Message
Unité de données transmise sur le canal, composée d'un header et d'un payload.

### Modèle prédictif
Algorithme qui estime la prochaine valeur probable d'une source de données, basé sur l'historique.

### MQTT
Protocole de messagerie léger, populaire en IoT (Message Queuing Telemetry Transport).

---

## N

### NACK (Negative Acknowledgment)
Message indiquant un problème avec un message reçu (erreur de checksum, désynchronisation...).

---

## P

### P1-P5
Les cinq niveaux de priorité ALEC :
- **P1** : Critique (alerte immédiate)
- **P2** : Important (anomalie)
- **P3** : Normal (mesure standard)
- **P4** : Différé (sur demande)
- **P5** : Jetable (debug)

### Paresseux
Voir **Lazy**.

### Pattern
Séquence de données récurrente, stockée dans le dictionnaire pour compression.

### Payload
Contenu utile d'un message, hors header et métadonnées.

### Prédiction
Valeur estimée par le modèle prédictif pour une source donnée.

### Priorité
Niveau d'importance attribué à une donnée, déterminant son traitement.

### Promotion
Processus par lequel un pattern fréquent obtient un code plus court dans le dictionnaire.

---

## R

### Rate limiting
Limitation du nombre de requêtes ou messages par unité de temps.

### Récepteur
Entité qui reçoit et décode les données. Typiquement un serveur ou une gateway.

### Requête
Message du récepteur vers l'émetteur demandant des données complémentaires.

### Resync (Resynchronisation)
Processus de réalignement des contextes émetteur/récepteur après une divergence.

### Roundtrip
Cycle complet encodage → transmission → décodage → vérification.

---

## S

### Seuil
Valeur limite déclenchant un changement de classification (ex: seuil d'anomalie).

### Source
Origine des données, identifiée par un Source ID unique.

### Synchronisation
Processus d'alignement des contextes entre émetteur et récepteur.

---

## T

### Timestamp
Horodatage indiquant le moment de la mesure ou de l'événement.

### TTL (Time To Live)
Durée de vie d'une donnée avant expiration automatique.

---

## V

### Varint
Encodage d'entiers de taille variable, économisant des octets pour les petites valeurs.

### Version
Numéro incrémental identifiant l'état du contexte à un instant donné.

---

## W

### Wraparound
Retour à zéro d'un compteur après avoir atteint sa valeur maximale.

---

## Symboles et abréviations

| Abréviation | Signification |
|-------------|---------------|
| BE | Big-Endian (octet de poids fort en premier) |
| LE | Little-Endian (octet de poids faible en premier) |
| bps | bits par seconde |
| KB | Kilooctet (1024 octets) |
| MB | Mégaoctet (1024 KB) |
| ms | milliseconde |
| E→R | Direction émetteur vers récepteur |
| R→E | Direction récepteur vers émetteur |
| E↔R | Bidirectionnel |
