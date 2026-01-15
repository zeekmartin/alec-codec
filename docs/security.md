# ALEC — Considérations de sécurité

## Vue d'ensemble

ALEC manipule des données potentiellement sensibles (capteurs médicaux, industriels, etc.) sur des canaux potentiellement non sécurisés. Ce document définit les menaces, les mesures de protection et les bonnes pratiques.

---

## Modèle de menaces

### Acteurs malveillants

| Acteur | Capacité | Motivation |
|--------|----------|------------|
| Eavesdropper | Écoute passive du canal | Espionnage, vol de données |
| MITM | Interception et modification | Sabotage, injection de fausses données |
| Rogue Emitter | Émetteur compromis | Pollution du contexte partagé |
| Rogue Receiver | Récepteur compromis | Extraction de données historiques |

### Surfaces d'attaque

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│  [Émetteur]                                                 │
│      │                                                      │
│      ▼ (1) Canal de données ◄──── Écoute, injection        │
│      │                                                      │
│      ▼ (2) Canal de sync ◄──────── Désynchronisation       │
│      │                                                      │
│      ▼ (3) Canal de requêtes ◄──── Requêtes frauduleuses   │
│      │                                                      │
│  [Récepteur]                                                │
│                                                             │
│  (4) Contexte partagé ◄─────────── Empoisonnement          │
│  (5) Stockage local ◄───────────── Accès physique          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Menaces spécifiques à ALEC

### 1. Empoisonnement du contexte

**Risque** : Un attaquant envoie des patterns artificiels pour polluer le dictionnaire partagé, dégradant la compression ou créant des canaux cachés.

**Mitigation** :
- Validation des patterns avant promotion
- Seuil de fréquence minimum avant inclusion
- Signature des mises à jour de contexte
- Audit périodique du dictionnaire

### 2. Attaque par désynchronisation

**Risque** : Forcer une divergence entre les contextes émetteur/récepteur, rendant les messages indécodables (déni de service).

**Mitigation** :
- Hash de vérification dans chaque message
- Resynchronisation automatique si divergence détectée
- Fallback vers encodage sans contexte (dégradé mais fonctionnel)

### 3. Requêtes abusives

**Risque** : Un faux récepteur envoie des `REQ_DETAIL` pour extraire toutes les données P4/P5 stockées.

**Mitigation** :
- Authentification des requêtes
- Rate limiting côté émetteur
- Expiration des données P4/P5 après TTL

### 4. Inférence par analyse de trafic

**Risque** : Même chiffrées, les métadonnées (taille, fréquence, timing) révèlent des informations.

**Mitigation** :
- Padding des messages à taille fixe (optionnel, coûteux)
- Envoi de messages factices (optionnel)
- Agrégation temporelle

---

## Mécanismes de sécurité

### Couche transport

ALEC ne réinvente pas la cryptographie. Il s'appuie sur des protocoles éprouvés :

| Niveau | Protocole recommandé | Usage |
|--------|---------------------|-------|
| Chiffrement | TLS 1.3, DTLS | Canal sécurisé |
| Authentification | mTLS, PSK | Identité des pairs |
| Intégrité | HMAC-SHA256 | Vérification messages |

### Authentification des pairs

```
┌─────────────┐                      ┌─────────────┐
│  Émetteur   │                      │  Récepteur  │
│             │                      │             │
│  [Clé privée]                      │  [Clé privée]
│  [Cert récepteur]                  │  [Cert émetteur]
│             │                      │             │
└──────┬──────┘                      └──────┬──────┘
       │                                    │
       │◄────── Authentification mutuelle ──►│
       │                                    │
       │         Canal sécurisé établi       │
       │◄══════════════════════════════════►│
```

### Intégrité du contexte

Chaque mise à jour du contexte est signée :

```
ContextUpdate {
    version: u32
    timestamp: u64
    operations: Vec<Op>
    hash_resultat: [u8; 32]
    signature: [u8; 64]  // Ed25519
}
```

---

## Niveaux de sécurité

ALEC propose trois profils :

### Profil MINIMAL
- Pas de chiffrement
- Intégrité par CRC32
- Usage : environnements isolés, tests

### Profil STANDARD (par défaut)
- TLS 1.3 / DTLS
- Authentification par certificats
- Intégrité HMAC-SHA256
- Usage : production normale

### Profil RENFORCÉ
- Tout STANDARD +
- Padding des messages
- Rotation des clés fréquente
- Audit logging complet
- Usage : données sensibles (médical, critique)

---

## Gestion des clés

### Émetteurs contraints

Pour les microcontrôleurs avec peu de ressources :
- Clés pré-provisionnées en usine
- Pas de génération de clés on-device
- Rotation manuelle (remplacement physique ou OTA sécurisé)

### Récepteurs

- Stockage sécurisé (HSM recommandé en production)
- Rotation automatique possible
- Révocation via CRL ou OCSP

### Clés de contexte

Le contexte partagé lui-même peut être considéré comme un secret :
- Ne pas transmettre le dictionnaire complet en clair
- Synchronisation différentielle chiffrée
- Purge sécurisée si compromission suspectée

---

## Audit et logging

### Événements à logger (côté récepteur)

| Événement | Niveau | Données |
|-----------|--------|---------|
| Connexion émetteur | INFO | ID, timestamp, IP |
| Alerte P1 reçue | WARN | Type, valeur, source |
| Désynchronisation | ERROR | Hash attendu/reçu |
| Requête rejetée | WARN | Type, raison |
| Mise à jour contexte | INFO | Version, nb ops |

### Rétention

- Logs opérationnels : 30 jours
- Logs sécurité : 1 an minimum
- Alertes P1 : conservation illimitée

---

## Conformité

### RGPD (si données personnelles)

- Minimisation : ALEC par design (ne transmet que le nécessaire)
- Droit à l'effacement : purge du contexte possible
- Portabilité : export du dictionnaire en format standard

### Secteur médical (si applicable)

- Conformité HIPAA / HDS selon juridiction
- Chiffrement obligatoire
- Traçabilité complète

### Secteur industriel

- IEC 62443 comme référence
- Séparation des réseaux IT/OT
- Tests de pénétration recommandés

---

## Checklist de déploiement sécurisé

- [ ] Certificats générés et distribués
- [ ] TLS/DTLS configuré et testé
- [ ] Rate limiting activé côté émetteur
- [ ] TTL configuré pour données P4/P5
- [ ] Logging activé côté récepteur
- [ ] Procédure de révocation documentée
- [ ] Tests de désynchronisation effectués
- [ ] Profil de sécurité choisi et documenté

---

## Réponse aux incidents

### Compromission d'un émetteur

1. Révoquer son certificat immédiatement
2. Purger ses contributions au contexte partagé
3. Forcer resync pour tous les autres émetteurs
4. Analyser les logs pour évaluer l'impact

### Compromission du récepteur

1. Considérer tout le contexte comme compromis
2. Générer nouvelles clés pour tous les émetteurs
3. Réinitialiser les contextes (perte de l'apprentissage)
4. Audit complet avant remise en service

### Désynchronisation persistante

1. Vérifier intégrité du canal
2. Forcer `REQ_RESYNC` complet
3. Si échec : réinitialisation du contexte
4. Investiguer cause racine (attaque vs bug)
