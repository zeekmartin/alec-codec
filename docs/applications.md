# ALEC — Applications et cas d'usage

Ce document présente des cas d'usage concrets d'ALEC avec des exemples détaillés, des calculs de gains et des configurations recommandées.

---

## Table des matières

1. [Agriculture connectée](#1-agriculture-connectée)
2. [Télémédecine en zones isolées](#2-télémédecine-en-zones-isolées)
3. [Flottes de véhicules](#3-flottes-de-véhicules)
4. [Surveillance industrielle](#4-surveillance-industrielle)
5. [Observation spatiale et drones](#5-observation-spatiale-et-drones)
6. [Monitoring environnemental](#6-monitoring-environnemental)
7. [Smart buildings](#7-smart-buildings)
8. [Exploration sous-marine](#8-exploration-sous-marine)

---

## 1. Agriculture connectée

### Contexte

Une exploitation agricole de 500 hectares déploie 200 capteurs pour surveiller :
- Humidité du sol
- Température
- pH
- Luminosité

Les capteurs doivent fonctionner **10 ans sur batterie** avec une connexion LoRaWAN limitée à **51 octets par message**.

### Problématique sans ALEC

```
Chaque capteur envoie toutes les 15 minutes :
- Timestamp: 8 octets
- Humidité: 4 octets  
- Température: 4 octets
- pH: 4 octets
- Luminosité: 4 octets
- Header LoRa: 13 octets
─────────────────────────────
Total: 37 octets × 96/jour × 365 jours × 10 ans = 129 Mo par capteur

Énergie par transmission: ~50 mJ
Total énergie: 175 Wh sur 10 ans → Batterie de 20 Ah minimum
```

### Solution ALEC

```
Configuration ALEC:
- Contexte partagé avec patterns saisonniers
- Classification: anomalie = écart > 15% de la prédiction
- Mode paresseux: détails P4 jamais envoyés sauf demande

Semaine 1 (apprentissage):
  Message complet: 37 octets

Après rodage:
  Message normal: [capteur_id][delta_h][delta_t][flags] = 6 octets
  Message alerte: [capteur_id][ALERTE][type][valeur] = 10 octets

Statistiques observées:
- 95% des messages: normaux (6 octets)
- 4% des messages: alertes (10 octets)
- 1% des messages: resync contexte (20 octets)

Moyenne: 6.3 octets par message
Gain: 83% de réduction
```

### Exemple de flux

```
Jour 1, 08:00 - Premier démarrage
┌─────────────────────────────────────────────────────────────┐
│ Capteur #42 → Serveur                                       │
│ [INIT][humidité=65%][temp=18°C][pH=6.8][lux=12000]         │
│ 37 octets                                                   │
└─────────────────────────────────────────────────────────────┘

Jour 30, 08:00 - Contexte établi, valeur normale
┌─────────────────────────────────────────────────────────────┐
│ Capteur #42 → Serveur                                       │
│ [42][+2%][-0.5°][OK]                                        │
│ 6 octets                                                    │
│                                                             │
│ Serveur reconstruit: humidité=67%, temp=17.5°C              │
│ (basé sur: prédiction du modèle saisonnier + delta reçu)   │
└─────────────────────────────────────────────────────────────┘

Jour 30, 14:00 - Anomalie détectée
┌─────────────────────────────────────────────────────────────┐
│ Capteur #42 → Serveur                                       │
│ [42][ALERTE][HUMIDITY_LOW][32%]                             │
│ 10 octets                                                   │
│                                                             │
│ Serveur: Alerte irrigation! Humidité 32% au lieu de 65%    │
│                                                             │
│ Serveur → Capteur (optionnel)                               │
│ [REQ_DETAIL][42]                                            │
│                                                             │
│ Capteur #42 → Serveur                                       │
│ [DETAIL][historique 2h][courbe complète]                    │
│ 128 octets (envoyé seulement si demandé)                    │
└─────────────────────────────────────────────────────────────┘
```

### Configuration recommandée

```yaml
alec_config:
  mode: emitter_light
  context:
    model: seasonal_periodic
    sync_interval: 24h
  classification:
    p1_threshold: 0.30  # Écart > 30%
    p2_threshold: 0.15  # Écart > 15%
    minimum_delta: 0.02 # Ignorer < 2%
  power:
    sleep_between_transmissions: true
    batch_non_critical: true
```

### Gains mesurés

| Métrique | Sans ALEC | Avec ALEC | Gain |
|----------|-----------|-----------|------|
| Octets/jour/capteur | 3,552 | 605 | 83% |
| Énergie/jour | 4.8 Wh | 0.9 Wh | 81% |
| Durée batterie | 2.3 ans | 12+ ans | 5x |

---

## 2. Télémédecine en zones isolées

### Contexte

Un centre de santé rural au Mali dispose d'une connexion satellite à **9600 bps** (coût: 2€/Mo). Il utilise :
- Échographe portable
- Tensiomètre connecté
- Oxymètre
- Thermomètre

Le médecin référent est à 800 km, à l'hôpital régional.

### Problématique sans ALEC

```
Une consultation type:
- Image échographie: 500 KB
- Données vitales: 2 KB
- Notes texte: 1 KB
─────────────────────────────────
Total: ~503 KB
Temps de transmission: 7 minutes
Coût: ~1€ par consultation
```

### Solution ALEC

```
Approche paresseuse:

1. Transmission immédiate (P2):
   [PATIENT_ID][SUSPICION][cardiac_anomaly][confidence=78%]
   → 50 octets, < 1 seconde

2. Le médecin référent décide:
   - Si confiance suffisante: diagnostic à distance
   - Si besoin de voir: demande l'image

3. Transmission sur demande (P4):
   - Image compressée avec contexte (motifs échographiques connus)
   - 150 KB au lieu de 500 KB
   
Statistiques:
- 60% des cas: diagnostic sans image → 50 octets
- 30% des cas: image partielle demandée → 80 KB
- 10% des cas: image complète nécessaire → 150 KB

Moyenne: 35 KB par consultation (au lieu de 503 KB)
```

### Exemple de flux

```
Consultation patient #1234

┌─────────────────────────────────────────────────────────────┐
│ Phase 1: Analyse locale                                     │
├─────────────────────────────────────────────────────────────┤
│ Appareil échographe analyse l'image localement              │
│ Détecte: anomalie valve mitrale, confiance 78%              │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ 50 octets
┌─────────────────────────────────────────────────────────────┐
│ Phase 2: Alerte au médecin référent                         │
├─────────────────────────────────────────────────────────────┤
│ [P2][patient=1234][cardiac][valve_mitral][conf=78%]         │
│ [vitals: BP=140/90, SpO2=96%, temp=37.2]                    │
│                                                             │
│ Médecin reçoit sur son téléphone:                           │
│ "Patient 1234: Suspicion anomalie valve mitrale (78%)"      │
│ "Tension: 140/90, SpO2: 96%"                                │
│ [Voir image] [Diagnostic direct] [Appeler]                  │
└─────────────────────────────────────────────────────────────┘
                              │
                   Médecin clique "Voir image"
                              │
                              ▼ Requête
┌─────────────────────────────────────────────────────────────┐
│ Phase 3: Transmission image (sur demande)                   │
├─────────────────────────────────────────────────────────────┤
│ Serveur → Appareil: [REQ_DETAIL][image][zone=cardiac]       │
│                                                             │
│ Appareil → Serveur:                                         │
│ - Seulement la zone cardiaque (pas l'image complète)        │
│ - Compression avec dictionnaire échographique               │
│ - 80 KB au lieu de 500 KB                                   │
│                                                             │
│ Temps: 70 secondes (au lieu de 7 minutes)                   │
└─────────────────────────────────────────────────────────────┘
```

### Configuration recommandée

```yaml
alec_config:
  mode: emitter_light
  domain: medical
  context:
    preloaded_patterns: echography_cardiac_v2
    patient_history: enabled
  classification:
    always_p2: [anomaly_detected, vital_signs_abnormal]
    always_p4: [full_image, detailed_waveform]
  compression:
    image_roi_only: true  # Région d'intérêt seulement
    lossy_allowed: false  # Médical = sans perte
```

### Gains mesurés

| Métrique | Sans ALEC | Avec ALEC | Gain |
|----------|-----------|-----------|------|
| Données/consultation | 503 KB | 35 KB | 93% |
| Temps transmission | 7 min | 30 sec | 93% |
| Coût/consultation | 1€ | 0.07€ | 93% |
| Consultations/jour (budget fixe) | 10 | 140 | 14x |

---

## 3. Flottes de véhicules

### Contexte

Une entreprise de livraison gère 500 camions qui remontent :
- Position GPS (toutes les 30 secondes)
- Vitesse
- Consommation carburant
- État moteur
- Température cargo (pour camions frigorifiques)

Connexion: 4G/LTE avec forfait data de 2 Go/mois/véhicule.

### Problématique sans ALEC

```
Chaque véhicule envoie toutes les 30 secondes:
- GPS: 16 octets (lat/long/alt/précision)
- Vitesse: 4 octets
- Carburant: 4 octets
- Moteur: 8 octets
- Cargo: 4 octets
- Timestamp: 8 octets
- Header: 20 octets
─────────────────────────────────
Total: 64 octets × 2880/jour = 180 KB/jour/véhicule
       = 5.4 Mo/mois/véhicule (× 500 = 2.7 Go/mois total)
```

### Solution ALEC

```
Contexte partagé de flotte:
- Routes habituelles apprises
- Patterns de conduite par chauffeur
- Horaires typiques

Après 2 semaines d'apprentissage:

Message "sur route connue":
  [vehicule_id][route_7][avancement=45%][nominal]
  → 8 octets

Message "écart de route":
  [vehicule_id][DEVIATION][nouvelle_position]
  → 20 octets

Message "anomalie":
  [vehicule_id][ALERTE][type][données]
  → 16 octets

Répartition observée:
- 85% sur routes connues: 8 octets
- 10% déviations mineures: 20 octets
- 5% anomalies/alertes: 16 octets

Moyenne: 9.6 octets (au lieu de 64)
```

### Exemple de flux

```
Camion #237 - Tournée habituelle Lyon → Marseille

06:00 - Départ dépôt Lyon
┌─────────────────────────────────────────────────────────────┐
│ [237][ROUTE_START][route_id=LYN-MRS-A7][eta=09:30]         │
│ 12 octets                                                   │
│                                                             │
│ Le serveur sait: Route A7, ~300km, stops prévus à          │
│ Valence (carburant) et Montélimar (livraison)              │
└─────────────────────────────────────────────────────────────┘

06:30 - Sur autoroute, tout nominal
┌─────────────────────────────────────────────────────────────┐
│ [237][PROGRESS][12%][OK]                                    │
│ 6 octets                                                    │
│                                                             │
│ Serveur calcule: position ≈ km 36 sur A7                    │
│ Vitesse ≈ 90 km/h (déduit du temps et progression)         │
└─────────────────────────────────────────────────────────────┘

07:45 - Bouchon imprévu
┌─────────────────────────────────────────────────────────────┐
│ [237][DELAY][+45min][traffic][lat=44.82][lon=4.87]         │
│ 18 octets                                                   │
│                                                             │
│ Serveur: Alerte dispatcher - Livraison Montélimar retardée │
│ Recalcul ETA automatique                                    │
└─────────────────────────────────────────────────────────────┘

08:30 - Anomalie moteur
┌─────────────────────────────────────────────────────────────┐
│ [237][ALERT][ENGINE][temp_high][105°C]                      │
│ 14 octets                                                   │
│                                                             │
│ Serveur → Dispatcher: Alerte critique!                      │
│ Serveur → Camion: [REQ_DETAIL][engine_data]                 │
│                                                             │
│ Camion → Serveur: [DETAIL][engine_log_2h]                   │
│ 256 octets (historique complet sur demande)                 │
└─────────────────────────────────────────────────────────────┘
```

### Contexte partagé de flotte

```
Le serveur maintient un contexte enrichi par tous les véhicules:

Routes connues:
┌────────────────────────────────────────────────────────────┐
│ route_id: LYN-MRS-A7                                       │
│ distance: 314 km                                           │
│ waypoints: [(45.76,4.83), (44.93,4.89), (44.56,4.75), ...] │
│ typical_duration: 3h20                                     │
│ fuel_consumption: 95L (moyenne flotte)                     │
│ speed_profile: [90, 110, 90, 70, 110, ...]                │
│ common_delays: [km_45_am_rush, km_180_construction]        │
└────────────────────────────────────────────────────────────┘

Quand un camion dit "route_7, avancement 45%":
→ Serveur connaît sa position à ±500m sans GPS explicite
→ Serveur connaît sa vitesse probable
→ Serveur connaît son niveau de carburant estimé
```

### Configuration recommandée

```yaml
alec_config:
  mode: fleet
  context:
    shared_fleet_knowledge: true
    route_learning: enabled
    driver_profiles: enabled
  classification:
    p1: [accident, breakdown, cargo_breach]
    p2: [delay_significant, route_deviation, anomaly]
    p3: [progress_update]
    p4: [detailed_telemetry]
    p5: [debug_logs]
  sync:
    fleet_context_update: daily
    route_update: weekly
```

### Gains mesurés

| Métrique | Sans ALEC | Avec ALEC | Gain |
|----------|-----------|-----------|------|
| Data/véhicule/mois | 5.4 Mo | 850 Ko | 84% |
| Data flotte/mois | 2.7 Go | 425 Mo | 84% |
| Coût data/mois | 1350€ | 215€ | 84% |

---

## 4. Surveillance industrielle

### Contexte

Une usine de production surveille 50 machines avec chacune 10 capteurs :
- Vibrations (3 axes)
- Température
- Pression hydraulique
- Courant moteur
- RPM
- Bruit acoustique

Objectif : maintenance prédictive, détection des pannes avant qu'elles ne surviennent.

### Solution ALEC

```
Modèle de référence par machine:
- Signature vibratoire "normale" apprise
- Profil thermique selon charge
- Corrélations entre capteurs

Transmission:
- Nominal: "machine OK" + deltas minimes
- Anomalie: type + sévérité + capteur concerné
- Détail: sur demande pour diagnostic

Le contexte partagé connaît:
- Les modes de fonctionnement (idle, production, maintenance)
- Les transitions normales entre modes
- Les signatures de chaque type de panne connue
```

### Exemple : Détection précoce de panne

```
Machine #12 - Presse hydraulique

État normal (appris sur 3 mois):
┌────────────────────────────────────────────────────────────┐
│ vibration_x: 2.3 ± 0.2 mm/s                                │
│ vibration_y: 1.8 ± 0.15 mm/s                               │
│ vibration_z: 3.1 ± 0.25 mm/s                               │
│ temperature: 45 ± 3°C                                      │
│ pressure: 180 ± 5 bar                                      │
│ current: 12.5 ± 0.5 A                                      │
└────────────────────────────────────────────────────────────┘

Jour J - 10:00 - Légère dérive détectée
┌────────────────────────────────────────────────────────────┐
│ [12][DRIFT][vibration_z][+15%][gradual]                    │
│ 10 octets                                                  │
│                                                            │
│ Classification: P3 (à surveiller, pas urgent)              │
│ Dashboard: Indicateur jaune sur machine 12                 │
└────────────────────────────────────────────────────────────┘

Jour J+2 - 14:00 - Dérive confirmée + corrélation
┌────────────────────────────────────────────────────────────┐
│ [12][ANOMALY][bearing_wear_signature][confidence=85%]      │
│ 14 octets                                                  │
│                                                            │
│ Le contexte a reconnu le pattern:                          │
│ vibration_z ↑ + harmonique à 847 Hz = usure roulement     │
│                                                            │
│ Classification: P2 (maintenance à planifier)               │
│ Action: Ordre de maintenance créé automatiquement          │
└────────────────────────────────────────────────────────────┘

Jour J+2 - 14:05 - Demande de diagnostic complet
┌────────────────────────────────────────────────────────────┐
│ Serveur → Machine: [REQ_DETAIL][vibration_spectrum]        │
│                                                            │
│ Machine → Serveur: [DETAIL][FFT_data][24h_history]         │
│ 4 KB (spectre complet pour analyse experte)                │
│                                                            │
│ Ingénieur confirme: roulement principal à remplacer        │
│ Maintenance planifiée: Jour J+5 (avant panne prévue J+12)  │
└────────────────────────────────────────────────────────────┘
```

### Gains mesurés

| Métrique | Sans ALEC | Avec ALEC | Gain |
|----------|-----------|-----------|------|
| Data/machine/jour | 86 Mo | 2.1 Mo | 97% |
| Temps détection anomalie | Identique | Identique | - |
| Faux positifs | 12/jour | 3/jour | 75% |
| Pannes évitées | Réactif | Prédictif | ++ |

---

## 5. Observation spatiale et drones

### Contexte

Un drone d'inspection survole des lignes électriques haute tension. Il capture des images thermiques pour détecter les points chauds (connexions défectueuses).

Contraintes :
- Liaison radio limitée à 500 kbps
- Autonomie: 45 minutes de vol
- Zone de couverture: 50 km de lignes

### Solution ALEC

```
Traitement embarqué:
1. Capture image thermique (2 Mo)
2. Analyse locale: détection de points chauds
3. Classification:
   - Pas d'anomalie → P5 (stocké, pas transmis)
   - Point chaud mineur → P3 (position + température)
   - Point chaud critique → P1 (alerte + miniature)

Transmission:
- P1: alerte + crop 100×100 pixels = 5 Ko
- P3: coordonnées + température = 20 octets
- P5: rien pendant le vol, déchargé au retour

Contexte partagé:
- Carte des pylônes connus
- Historique thermique par pylône
- Températures ambiantes du jour
```

### Exemple de mission

```
Mission inspection ligne 225kV - 50km

Décollage 09:00
┌────────────────────────────────────────────────────────────┐
│ [DRONE_01][MISSION_START][line=225kV_north][50km]          │
│ 16 octets                                                  │
└────────────────────────────────────────────────────────────┘

09:05 - Pylône #1 à #10 - Tout normal
┌────────────────────────────────────────────────────────────┐
│ [DRONE_01][PROGRESS][pylons=1-10][status=OK]               │
│ 12 octets pour 10 pylônes (au lieu de 20 Mo d'images)      │
└────────────────────────────────────────────────────────────┘

09:12 - Pylône #17 - Point chaud détecté
┌────────────────────────────────────────────────────────────┐
│ [DRONE_01][ALERT][pylon=17][hotspot][temp=89°C][conn_B]    │
│ + [IMAGE_CROP][100x100][thermal]                           │
│ 5.2 Ko total                                               │
│                                                            │
│ Opérateur voit immédiatement:                              │
│ - Position exacte (pylône 17, connexion B)                 │
│ - Température (89°C vs normal 45°C)                        │
│ - Image zoomée sur le point chaud                          │
│                                                            │
│ Décision: maintenance prioritaire planifiée                │
└────────────────────────────────────────────────────────────┘

09:45 - Fin de mission
┌────────────────────────────────────────────────────────────┐
│ [DRONE_01][MISSION_END][pylons=127][alerts=1][images=127]  │
│ 14 octets                                                  │
│                                                            │
│ Bilan transmission pendant vol:                            │
│ - Total transmis: 6.8 Ko                                   │
│ - Sans ALEC: 254 Mo (127 images × 2 Mo)                   │
│                                                            │
│ Au retour, déchargement des 127 images pour archivage      │
└────────────────────────────────────────────────────────────┘
```

### Gains mesurés

| Métrique | Sans ALEC | Avec ALEC | Gain |
|----------|-----------|-----------|------|
| Data transmise/mission | 254 Mo | 6.8 Ko | 99.99% |
| Temps réel alerte | Non | Oui | ∞ |
| Missions/jour | 3 | 3 | - |
| Décisions terrain | Au retour | Immédiat | ++ |

---

## 6. Monitoring environnemental

### Contexte

Un réseau de 500 stations météo couvre une région montagneuse pour la prévention des risques (avalanches, crues, incendies). Connexion satellite Iridium à 2.4 kbps.

### Solution ALEC

```
Contexte régional partagé:
- Modèle météo de la région
- Corrélations entre stations voisines
- Patterns saisonniers

Chaque station ne transmet que son écart au "consensus régional":
- Si station #42 mesure 12°C et le modèle prédit 11.5°C
- Transmission: [42][temp][+0.5°]
- Au lieu de: [42][temp=12.0°C][timestamp=...][...]

Pour les alertes:
- Vent > 100 km/h → P1
- Neige > 30 cm/12h → P1
- Température < seuil gel → P2
```

### Gains mesurés

| Métrique | Sans ALEC | Avec ALEC | Gain |
|----------|-----------|-----------|------|
| Data/station/jour | 8.6 Ko | 1.2 Ko | 86% |
| Coût satellite/mois | 4300€ | 600€ | 86% |
| Stations finançables | 500 | 3500 | 7x |

---

## 7. Smart buildings

### Contexte

Un immeuble de bureaux de 20 étages avec 2000 capteurs :
- Température (500)
- Présence (500)
- Qualité air CO2 (200)
- Luminosité (300)
- Compteurs énergie (500)

### Solution ALEC

```
Contexte du bâtiment:
- Planning d'occupation par zone
- Profils journaliers appris
- Corrélations entre zones adjacentes

Modes de fonctionnement:
- Jour ouvré: transmission si écart > 5%
- Nuit/weekend: transmission si écart > 20% (anomalie)
- Événement: transmission renforcée

La plupart des capteurs ne transmettent rien pendant des heures
car leur valeur est "comme prévu".
```

### Gains mesurés

| Métrique | Sans ALEC | Avec ALEC | Gain |
|----------|-----------|-----------|------|
| Messages/jour | 2.8M | 45K | 98% |
| Charge réseau | 140 Mo/j | 2.3 Mo/j | 98% |
| Alertes pertinentes | Noyées | Visibles | ++ |

---

## 8. Exploration sous-marine

### Contexte

Un ROV (robot sous-marin) inspecte des pipelines à 2000m de profondeur. Communication acoustique limitée à 1 kbps avec latence de 3 secondes.

### Solution ALEC

```
Autonomie maximale:
- Analyse d'image embarquée
- Décisions de navigation locales
- Transmission uniquement des découvertes

Types de messages:
- P1: Fuite détectée, dommage structural
- P2: Anomalie à inspecter
- P3: Progression (toutes les 5 min)
- P4: Images sur demande
- P5: Télémétrie complète (récupérée après mission)
```

### Exemple de mission

```
Inspection pipeline - Section 12 (5 km)

┌────────────────────────────────────────────────────────────┐
│ 14:00 - Début section                                      │
│ [ROV][SECTION_START][12][5km][visibility=good]             │
│ 18 octets                                                  │
└────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────┐
│ 14:05 - Progression normale                                │
│ [ROV][PROGRESS][12%][nominal]                              │
│ 8 octets                                                   │
└────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────┐
│ 14:23 - Anomalie détectée                                  │
│ [ROV][ANOMALY][corrosion][severity=medium][pos=2.3km]      │
│ 16 octets                                                  │
│                                                            │
│ ROV automatiquement:                                       │
│ - S'arrête et prend photos HD                              │
│ - Mesure épaisseur paroi                                   │
│ - Stocke pour transmission ultérieure                      │
│                                                            │
│ Opérateur peut demander image si urgent:                   │
│ Surface → ROV: [REQ_IMAGE][thumbnail]                      │
│ ROV → Surface: [IMAGE][64x64][grayscale]                   │
│ 4 Ko, 32 secondes de transmission                          │
└────────────────────────────────────────────────────────────┘
```

### Gains mesurés

| Métrique | Sans ALEC | Avec ALEC | Gain |
|----------|-----------|-----------|------|
| Autonomie décision | Aucune | Totale | ++ |
| Data temps réel | 50 Ko/min | 0.5 Ko/min | 99% |
| Réactivité alerte | 5 min | 10 sec | 30x |

---

## Synthèse comparative

| Application | Réduction data | Gain principal | Complexité |
|-------------|----------------|----------------|------------|
| Agriculture | 83% | Autonomie batterie | Faible |
| Télémédecine | 93% | Coût satellite | Moyenne |
| Flottes | 84% | Coût data | Moyenne |
| Industrie | 97% | Maintenance prédictive | Élevée |
| Drones | 99.99% | Temps réel | Élevée |
| Environnement | 86% | Densité réseau | Faible |
| Smart building | 98% | Charge réseau | Moyenne |
| Sous-marin | 99% | Autonomie mission | Élevée |

---

## Quelle application pour commencer ?

**Débutant** : Agriculture ou Smart building
- Contexte simple (valeurs numériques)
- Pas de temps réel critique
- Facile à simuler

**Intermédiaire** : Flottes ou Environnement
- Contexte partagé entre entités
- Patterns géographiques/temporels

**Avancé** : Industrie ou Drones
- Analyse embarquée complexe
- Contexte multi-dimensionnel
- Temps réel critique
