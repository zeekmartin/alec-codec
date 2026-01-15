# ALEC — Référence du protocole

Ce document est la spécification complète et normative du protocole ALEC.

---

## Vue d'ensemble

Le protocole ALEC définit :
1. Le format des messages binaires
2. Les types de messages et leur sémantique
3. Le protocole de synchronisation du contexte
4. Le protocole de requête/réponse

**Version du protocole** : 1.0

---

## Format des messages

### Structure générale

Tous les messages ALEC suivent cette structure :

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Octet 0    │ Octets 1-4  │ Octets 5-8  │ Octets 9-12 │ Variable        │
├────────────┼─────────────┼─────────────┼─────────────┼─────────────────┤
│ Header     │ Sequence    │ Timestamp   │ Ctx Version │ Payload         │
│ (1 octet)  │ (u32 BE)    │ (u32 BE)    │ (u32 BE)    │ (0-65535 oct.)  │
├────────────┼─────────────┼─────────────┼─────────────┼─────────────────┤
│ Obligatoire│ Obligatoire │ Obligatoire │ Obligatoire │ Selon type      │
└─────────────────────────────────────────────────────────────────────────┘

Taille totale : 13 + len(payload) octets
```

### Header (1 octet)

```
Bits:  7   6   5   4   3   2   1   0
      ├───┴───┼───┴───┴───┼───┴───┴───┤
      │Version│   Type    │ Priority  │
      │ 2 bits│  3 bits   │  3 bits   │
      └───────┴───────────┴───────────┘
```

| Champ | Bits | Valeurs |
|-------|------|---------|
| Version | 7-6 | 0-3 (actuel: 1) |
| Type | 5-3 | 0-7 (voir Types de messages) |
| Priority | 2-0 | 0-7 (voir Priorités) |

### Sequence (4 octets)

Numéro de séquence sur 32 bits, big-endian.
- Incrémenté pour chaque message envoyé
- Wraparound autorisé (0 après 0xFFFFFFFF)
- Utilisé pour la détection de perte et le rejeu

### Timestamp (4 octets)

Timestamp relatif sur 32 bits, big-endian.
- Secondes depuis le début de la session
- Ou: secondes depuis epoch Unix (tronqué)
- Négocié lors du handshake

### Context Version (4 octets)

Version du contexte utilisé pour encoder ce message.
- Permet au récepteur de détecter une désynchronisation
- Si mismatch : demande de resync

---

## Types de messages

| Code | Nom | Direction | Description |
|------|-----|-----------|-------------|
| 0 | DATA | E→R | Données encodées |
| 1 | SYNC | E↔R | Synchronisation de contexte |
| 2 | REQ | R→E | Requête |
| 3 | RESP | E→R | Réponse à une requête |
| 4 | ACK | E↔R | Accusé de réception |
| 5 | NACK | E↔R | Accusé négatif |
| 6 | HEARTBEAT | E↔R | Keep-alive |
| 7 | — | — | Réservé |

E = Émetteur, R = Récepteur

---

## Priorités

| Code | Nom | Comportement |
|------|-----|--------------|
| 0 | P1_CRITICAL | Envoi immédiat, accusé requis |
| 1 | P2_IMPORTANT | Envoi immédiat |
| 2 | P3_NORMAL | Envoi standard |
| 3 | P4_DEFERRED | Stocké, envoyé sur demande |
| 4 | P5_DISPOSABLE | Jamais envoyé spontanément |
| 5-7 | — | Réservé |

---

## Message DATA (Type 0)

### Format du payload DATA

```
┌──────────────┬───────────────┬─────────────────────────────────────────┐
│ Source ID    │ Encoding      │ Value                                   │
│ (varint)     │ (1 octet)     │ (variable)                              │
└──────────────┴───────────────┴─────────────────────────────────────────┘
```

### Source ID

Identifiant de la source de données, encodé en varint :
- 1 octet si < 128
- 2 octets si < 16384
- etc.

### Types d'encodage

| Code | Nom | Taille valeur | Description |
|------|-----|---------------|-------------|
| 0x00 | RAW64 | 8 octets | Float64 brut, big-endian |
| 0x01 | RAW32 | 4 octets | Float32 brut, big-endian |
| 0x10 | DELTA8 | 1 octet | Delta signé 8 bits |
| 0x11 | DELTA16 | 2 octets | Delta signé 16 bits, BE |
| 0x12 | DELTA32 | 4 octets | Delta signé 32 bits, BE |
| 0x20 | PATTERN | varint | Référence au dictionnaire |
| 0x21 | PATTERN_DELTA | varint + 1 | Pattern + delta 8 bits |
| 0x30 | REPEATED | 0 | Même valeur que précédent |
| 0x31 | INTERPOLATED | 0 | Valeur prédite exacte |
| 0x40 | MULTI | variable | Plusieurs valeurs (voir ci-dessous) |

### Encodage DELTA

Le delta est calculé comme :
```
delta = (actual_value - predicted_value) * scale_factor
```

Le `scale_factor` est négocié dans le contexte (défaut: 100 pour 2 décimales).

### Encodage PATTERN

Le varint référence un pattern dans le dictionnaire partagé.
Le décodeur remplace par la valeur associée au pattern.

### Encodage MULTI (0x40)

Pour les capteurs multi-valeurs :

```
┌───────────┬─────────────────────────────────────────────────────────────┐
│ Count     │ Values...                                                   │
│ (1 octet) │ (répété Count fois)                                        │
└───────────┴─────────────────────────────────────────────────────────────┘

Chaque valeur :
┌───────────────┬───────────────┬─────────────────────────────────────────┐
│ Name ID       │ Encoding      │ Value                                   │
│ (u16 BE)      │ (1 octet)     │ (variable)                              │
└───────────────┴───────────────┴─────────────────────────────────────────┘
```

---

## Message SYNC (Type 1)

### Sous-types SYNC

Le premier octet du payload indique le sous-type :

| Code | Nom | Description |
|------|-----|-------------|
| 0x00 | SYNC_FULL | Contexte complet |
| 0x01 | SYNC_DIFF | Différentiel |
| 0x02 | SYNC_HASH | Vérification hash uniquement |
| 0x03 | SYNC_RESET | Demande de réinitialisation |

### SYNC_FULL (0x00)

```
┌───────────┬──────────────┬──────────────┬───────────────────────────────┐
│ Subtype   │ Version      │ Hash         │ Dictionary                    │
│ (0x00)    │ (u32 BE)     │ (u64 BE)     │ (variable)                    │
└───────────┴──────────────┴──────────────┴───────────────────────────────┘

Dictionary :
┌───────────┬─────────────────────────────────────────────────────────────┐
│ Count     │ Entries...                                                  │
│ (u16 BE)  │ (répété Count fois)                                        │
└───────────┴─────────────────────────────────────────────────────────────┘

Entry :
┌───────────┬───────────┬─────────────────────────────────────────────────┐
│ Code      │ Len       │ Pattern                                         │
│ (varint)  │ (u8)      │ (Len octets)                                    │
└───────────┴───────────┴─────────────────────────────────────────────────┘
```

### SYNC_DIFF (0x01)

```
┌───────────┬──────────────┬──────────────┬──────────────┬─────────────────┐
│ Subtype   │ From Version │ To Version   │ Hash         │ Operations      │
│ (0x01)    │ (u32 BE)     │ (u32 BE)     │ (u64 BE)     │ (variable)      │
└───────────┴──────────────┴──────────────┴──────────────┴─────────────────┘

Operations :
┌───────────┬─────────────────────────────────────────────────────────────┐
│ Count     │ Ops...                                                      │
│ (u16 BE)  │ (répété Count fois)                                        │
└───────────┴─────────────────────────────────────────────────────────────┘

Op :
┌───────────┬───────────────────────────────────────────────────────────┐
│ Op Type   │ Data                                                      │
│ (1 octet) │ (variable selon type)                                     │
└───────────┴───────────────────────────────────────────────────────────┘

Op Types :
  0x00 ADD    : Code (varint) + Len (u8) + Pattern (Len octets)
  0x01 REMOVE : Code (varint)
  0x02 UPDATE : Code (varint) + Len (u8) + NewPattern (Len octets)
```

### SYNC_HASH (0x02)

```
┌───────────┬──────────────┬──────────────┐
│ Subtype   │ Version      │ Hash         │
│ (0x02)    │ (u32 BE)     │ (u64 BE)     │
└───────────┴──────────────┴──────────────┘
```

---

## Message REQ (Type 2)

### Sous-types REQ

| Code | Nom | Description |
|------|-----|-------------|
| 0x00 | REQ_DETAIL | Demande données détaillées |
| 0x01 | REQ_RANGE | Demande plage temporelle |
| 0x02 | REQ_RESYNC | Demande resynchronisation |
| 0x03 | REQ_STATUS | Demande statut émetteur |

### REQ_DETAIL (0x00)

```
┌───────────┬──────────────┬──────────────┐
│ Subtype   │ Event ID     │ Detail Level │
│ (0x00)    │ (u64 BE)     │ (u8)         │
└───────────┴──────────────┴──────────────┘

Detail Level :
  0x00 : Minimal
  0x01 : Standard
  0x02 : Full
  0x03 : Debug
```

### REQ_RANGE (0x01)

```
┌───────────┬──────────────┬──────────────┬──────────────┬──────────────┐
│ Subtype   │ Source ID    │ From TS      │ To TS        │ Max Count    │
│ (0x01)    │ (varint)     │ (u32 BE)     │ (u32 BE)     │ (u16 BE)     │
└───────────┴──────────────┴──────────────┴──────────────┴──────────────┘
```

### REQ_RESYNC (0x02)

```
┌───────────┬──────────────┐
│ Subtype   │ From Version │
│ (0x02)    │ (u32 BE)     │
└───────────┴──────────────┘
```

---

## Message RESP (Type 3)

### Format général

```
┌───────────┬──────────────┬───────────────────────────────────────────────┐
│ Status    │ Req ID       │ Data                                          │
│ (1 octet) │ (u32 BE)     │ (variable)                                    │
└───────────┴──────────────┴───────────────────────────────────────────────┘

Status :
  0x00 : OK
  0x01 : PARTIAL (données partielles)
  0x10 : ERROR_NOT_FOUND
  0x11 : ERROR_EXPIRED
  0x12 : ERROR_UNAUTHORIZED
  0x20 : RATE_LIMITED (+ délai en secondes, u16 BE)
```

---

## Message ACK (Type 4)

```
┌──────────────┐
│ Acked Seq    │
│ (u32 BE)     │
└──────────────┘
```

Acquitte le message avec le numéro de séquence indiqué.

---

## Message NACK (Type 5)

```
┌──────────────┬──────────────┬───────────────────────────────────────────┐
│ Nacked Seq   │ Reason       │ Expected (optionnel)                      │
│ (u32 BE)     │ (1 octet)    │ (variable)                                │
└──────────────┴──────────────┴───────────────────────────────────────────┘

Reason :
  0x00 : CHECKSUM_ERROR
  0x01 : CONTEXT_MISMATCH (+ expected version u32 + expected hash u64)
  0x02 : SEQUENCE_GAP (+ expected seq u32)
  0x03 : DECODE_ERROR
  0x04 : UNKNOWN_TYPE
```

---

## Message HEARTBEAT (Type 6)

```
┌──────────────┬──────────────┐
│ Uptime       │ Status Flags │
│ (u32 BE)     │ (u8)         │
└──────────────┴──────────────┘

Status Flags (bitmap) :
  Bit 0 : Battery low
  Bit 1 : Memory pressure
  Bit 2 : Queue backlog
  Bit 3 : Sync needed
  Bits 4-7 : Réservé
```

---

## Varint

Encodage d'entiers de taille variable :

```
Si valeur < 128 (0x80) :
  1 octet : valeur directe

Si valeur < 16384 (0x4000) :
  2 octets : (valeur >> 7) | 0x80, valeur & 0x7F

Si valeur < 2097152 (0x200000) :
  3 octets : similaire

Etc.
```

---

## Calcul du hash

Le hash du contexte est calculé avec xxHash64 :

```python
def compute_context_hash(dictionary):
    hasher = xxhash.xxh64()
    for code in sorted(dictionary.keys()):
        pattern = dictionary[code]
        hasher.update(code.to_bytes(4, 'big'))
        hasher.update(len(pattern).to_bytes(2, 'big'))
        hasher.update(pattern)
    return hasher.intdigest()
```

---

## Protocole de session

### Établissement

```
Émetteur                                      Récepteur
    │                                              │
    │────────── SYNC (SYNC_FULL) ─────────────────▶│
    │                                              │
    │◀───────── ACK ───────────────────────────────│
    │                                              │
    │           Session établie                    │
    │                                              │
```

### Échange nominal

```
Émetteur                                      Récepteur
    │                                              │
    │────────── DATA (P3) ────────────────────────▶│
    │                                              │
    │────────── DATA (P3) ────────────────────────▶│
    │                                              │
    │────────── DATA (P2) ────────────────────────▶│
    │                                              │
    │────────── DATA (P1) ────────────────────────▶│
    │◀───────── ACK (pour P1) ─────────────────────│
    │                                              │
```

### Resynchronisation

```
Émetteur                                      Récepteur
    │                                              │
    │────────── DATA (ctx_v=42) ──────────────────▶│
    │                                              │
    │◀───────── NACK (CONTEXT_MISMATCH, v=40) ─────│
    │                                              │
    │────────── SYNC (SYNC_DIFF, 40→42) ──────────▶│
    │                                              │
    │◀───────── ACK ───────────────────────────────│
    │                                              │
    │────────── DATA (ctx_v=42) ──────────────────▶│ OK
    │                                              │
```

---

## Limites et contraintes

| Paramètre | Limite | Notes |
|-----------|--------|-------|
| Taille message max | 65535 octets | Header exclu |
| Taille pattern max | 255 octets | |
| Patterns par contexte | 65535 | |
| Sources par session | 4 milliards | (u32) |
| Sequence wraparound | Autorisé | Détection gap ≤ 1000 |

---

## Sécurité

Le protocole ALEC est conçu pour être encapsulé dans :
- TLS 1.3 (TCP)
- DTLS 1.3 (UDP)

Les messages ne sont pas chiffrés au niveau ALEC.
L'authentification et l'intégrité sont déléguées à la couche transport.
