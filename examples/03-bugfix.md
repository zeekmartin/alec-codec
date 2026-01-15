# Exemple — Correction de bug

## Contexte

Cet exemple montre comment diagnostiquer et corriger un bug dans ALEC en suivant le workflow défini.

**Bug** : Overflow dans l'encodage delta pour grandes variations

---

## Étape 1 : Rapport de bug

```
BUG: Delta encoding overflow pour grandes variations

Symptôme observé :
Comportement attendu : Valeur reconstruite = valeur originale
Comportement actuel : Valeur corrompue quand delta > 127 ou < -128

Fréquence : [x] Toujours (quand |delta| > 127)

Contexte de reproduction :
Version ALEC : 0.2.1
Environnement : Linux x86_64, émetteur STM32F4

Étapes pour reproduire :
1. Établir un contexte avec prédiction = 100.0
2. Envoyer une valeur = 300.0
3. Delta = 200, dépasse la capacité d'un i8
4. La valeur reconstruite est incorrecte

Logs :
[2025-01-15T10:30:00] [WARN] Encoding delta: 200.0
[2025-01-15T10:30:00] [DEBUG] Using Delta8 encoding
[2025-01-15T10:30:00] [DEBUG] Encoded byte: -56 (0xC8)
[2025-01-15T10:30:01] [WARN] Decoded value: 44.0 (expected: 300.0)

Impact : [x] Critique (corruption de données)
```

---

## Étape 2 : Test de reproduction

```rust
#[test]
fn test_reproduces_delta_overflow_bug() {
    let encoder = Encoder::new();
    let decoder = Decoder::new();
    let mut context = Context::new();
    
    // Établir une prédiction à 100.0
    context.observe(1, &RawData::new(100.0, 0));
    
    // Envoyer une valeur avec grand delta (200)
    let original = RawData::new(300.0, 1);
    let encoded = encoder.encode(&original, &context);
    let decoded = decoder.decode(&encoded, &context).unwrap();
    
    // Ce test ÉCHOUE avant le fix
    assert!(
        (decoded.value - original.value).abs() < 0.01,
        "Expected 300.0, got {}",
        decoded.value
    );
}
```

**Résultat avant fix :**
```
thread 'test_reproduces_delta_overflow_bug' panicked at 'Expected 300.0, got 44'
```

---

## Étape 3 : Analyse de la cause racine

### Localisation

```
Fichier : src/encoder.rs
Fonction : choose_encoding()
Lignes : 78-95
```

### Code problématique

```rust
fn choose_encoding(&self, value: f64, prediction: Option<f64>) -> (u8, Vec<u8>) {
    if let Some(pred) = prediction {
        let delta = value - pred;
        
        // BUG ICI : On suppose que delta tient dans un i8
        // sans vérifier les bornes
        if delta.abs() < 128.0 {
            return (ENCODING_DELTA8, vec![delta as i8 as u8]);
        }
        
        // Fallback vers encodage complet
        // ... mais ce fallback n'est jamais atteint car la condition
        // est mal formulée !
    }
    
    (ENCODING_RAW, value.to_be_bytes().to_vec())
}
```

### Chaîne causale

```
1. delta = 200.0
2. Condition: 200.0.abs() < 128.0 → FALSE (OK, devrait fallback)

WAIT - relisons le code...

Ah ! Le bug est ailleurs. Regardons le code réel du rapport :
```

```rust
// Code RÉEL (trouvé après investigation)
fn choose_encoding(&self, value: f64, prediction: Option<f64>) -> (u8, Vec<u8>) {
    if let Some(pred) = prediction {
        let delta = value - pred;
        
        // BUG : Cast sans vérification de bornes !
        // delta as i8 fait un wrapping silencieux
        let delta_i8 = delta as i8;  // 200 → -56 (overflow wrap)
        
        // La condition vérifie delta_i8, pas delta original
        if delta_i8.abs() < 127 {  // -56.abs() = 56 < 127 → TRUE !
            return (ENCODING_DELTA8, vec![delta_i8 as u8]);
        }
        
        // ... fallback jamais atteint car 56 < 127
    }
    
    (ENCODING_RAW, value.to_be_bytes().to_vec())
}
```

### Vraie cause racine

Le cast `delta as i8` est fait **AVANT** la vérification des bornes. Le cast provoque un overflow wrap (200 → -56), et la condition `56 < 127` est vraie, donc le fallback n'est jamais utilisé.

### Pourquoi ça n'a pas été détecté

- Tests existants avec deltas < 100
- Pas de test avec valeurs extrêmes
- Pas de test de roundtrip avec grandes variations

---

## Étape 4 : Correction

### Option A (recommandée) : Vérifier avant le cast

```rust
fn choose_encoding(&self, value: f64, prediction: Option<f64>) -> (u8, Vec<u8>) {
    if let Some(pred) = prediction {
        let delta = value - pred;
        
        // Vérifier les bornes AVANT le cast
        if delta >= i8::MIN as f64 && delta <= i8::MAX as f64 {
            let delta_i8 = delta as i8;
            return (ENCODING_DELTA8, vec![delta_i8 as u8]);
        }
        
        // Essayer i16 pour deltas moyens
        if delta >= i16::MIN as f64 && delta <= i16::MAX as f64 {
            let delta_i16 = delta as i16;
            return (ENCODING_DELTA16, delta_i16.to_be_bytes().to_vec());
        }
    }
    
    // Fallback vers encodage complet
    (ENCODING_RAW, value.to_be_bytes().to_vec())
}
```

### Option B (alternative) : Utilisation de checked_cast

```rust
fn choose_encoding(&self, value: f64, prediction: Option<f64>) -> (u8, Vec<u8>) {
    if let Some(pred) = prediction {
        let delta = value - pred;
        
        // Utiliser une conversion sûre
        if let Some(delta_i8) = try_cast_i8(delta) {
            return (ENCODING_DELTA8, vec![delta_i8 as u8]);
        }
        
        if let Some(delta_i16) = try_cast_i16(delta) {
            return (ENCODING_DELTA16, delta_i16.to_be_bytes().to_vec());
        }
    }
    
    (ENCODING_RAW, value.to_be_bytes().to_vec())
}

fn try_cast_i8(value: f64) -> Option<i8> {
    if value >= i8::MIN as f64 && value <= i8::MAX as f64 && value.fract() == 0.0 {
        Some(value as i8)
    } else {
        None
    }
}
```

### Justification du choix : Option A

- Plus explicite et lisible
- Pas de fonction auxiliaire nécessaire
- Pattern courant, facilement reconnaissable
- Couvre aussi le cas i16 pour une meilleure compression

---

## Étape 5 : Tests

### Test de non-régression (le bug)

```rust
#[test]
fn test_delta_overflow_fixed() {
    let encoder = Encoder::new();
    let decoder = Decoder::new();
    let mut context = Context::new();
    
    // Établir une prédiction
    context.observe(1, &RawData::new(100.0, 0));
    
    // Test avec delta = 200 (dépassait i8)
    let original = RawData::new(300.0, 1);
    let encoded = encoder.encode(&original, &context);
    let decoded = decoder.decode(&encoded, &context).unwrap();
    
    assert!(
        (decoded.value - original.value).abs() < 0.01,
        "Delta overflow: expected {}, got {}",
        original.value,
        decoded.value
    );
}
```

### Tests des cas limites

```rust
#[test]
fn test_delta_at_i8_boundary() {
    let encoder = Encoder::new();
    let decoder = Decoder::new();
    let mut context = Context::new();
    context.observe(1, &RawData::new(0.0, 0));
    
    // Juste à la limite i8
    for delta in [-128.0, -127.0, 126.0, 127.0] {
        let original = RawData::new(delta, 1);
        let encoded = encoder.encode(&original, &context);
        let decoded = decoder.decode(&encoded, &context).unwrap();
        
        assert!(
            (decoded.value - original.value).abs() < 0.01,
            "Failed for delta {}",
            delta
        );
        
        // Vérifier qu'on utilise bien Delta8
        assert_eq!(encoded.encoding_type(), ENCODING_DELTA8);
    }
}

#[test]
fn test_delta_requires_i16() {
    let encoder = Encoder::new();
    let decoder = Decoder::new();
    let mut context = Context::new();
    context.observe(1, &RawData::new(0.0, 0));
    
    // Deltas qui nécessitent i16
    for delta in [-129.0, 128.0, -1000.0, 1000.0] {
        let original = RawData::new(delta, 1);
        let encoded = encoder.encode(&original, &context);
        let decoded = decoder.decode(&encoded, &context).unwrap();
        
        assert!(
            (decoded.value - original.value).abs() < 0.01,
            "Failed for delta {}",
            delta
        );
        
        // Vérifier qu'on utilise Delta16
        assert_eq!(encoded.encoding_type(), ENCODING_DELTA16);
    }
}

#[test]
fn test_delta_requires_raw() {
    let encoder = Encoder::new();
    let decoder = Decoder::new();
    let mut context = Context::new();
    context.observe(1, &RawData::new(0.0, 0));
    
    // Deltas qui nécessitent encodage RAW
    for delta in [-100000.0, 100000.0] {
        let original = RawData::new(delta, 1);
        let encoded = encoder.encode(&original, &context);
        let decoded = decoder.decode(&encoded, &context).unwrap();
        
        assert!(
            (decoded.value - original.value).abs() < 0.01,
            "Failed for delta {}",
            delta
        );
        
        // Vérifier qu'on utilise RAW
        assert_eq!(encoded.encoding_type(), ENCODING_RAW);
    }
}

#[test]
fn test_negative_delta_near_boundary() {
    let encoder = Encoder::new();
    let decoder = Decoder::new();
    let mut context = Context::new();
    context.observe(1, &RawData::new(100.0, 0));
    
    // Valeur qui donne delta = -128 (limite exacte de i8)
    let original = RawData::new(-28.0, 1);  // -28 - 100 = -128
    let encoded = encoder.encode(&original, &context);
    let decoded = decoder.decode(&encoded, &context).unwrap();
    
    assert!((decoded.value - original.value).abs() < 0.01);
}
```

---

## Étape 6 : Prévention

### Changements pour éviter ce type de bug

1. **Ajouter un lint personnalisé** pour détecter les casts numériques non vérifiés :
   ```rust
   #![deny(clippy::cast_possible_truncation)]
   ```

2. **Ajouter des tests de fuzzing** :
   ```rust
   #[test]
   fn fuzz_encode_decode_roundtrip() {
       use quickcheck::quickcheck;
       
       quickcheck(|value: f64, prediction: f64| {
           // ... test roundtrip avec valeurs aléatoires
       });
   }
   ```

3. **Ajouter assertion dans le décodeur** :
   ```rust
   fn decode_delta8(&self, byte: u8, prediction: f64) -> f64 {
       let delta = byte as i8 as f64;
       let result = prediction + delta;
       
       debug_assert!(
           (result - prediction).abs() <= 127.0,
           "Delta8 decoded impossible value"
       );
       
       result
   }
   ```

### Autres zones potentiellement affectées

- [ ] `encode_delta16()` — vérifier les bornes i16
- [ ] `encode_varint()` — vérifier les valeurs négatives
- [ ] `decode_pattern_id()` — vérifier l'index dans le dictionnaire

---

## Résumé

| Étape | Statut |
|-------|--------|
| Reproduction | ✅ Test qui échoue |
| Analyse | ✅ Cause racine identifiée |
| Correction | ✅ Option A implémentée |
| Tests | ✅ 4 nouveaux tests |
| Prévention | ✅ Lint + fuzzing ajoutés |
| Régression | ✅ Aucun test existant cassé |

**Commit message :**
```
fix(encoder): prevent delta overflow in choose_encoding

The delta was cast to i8 before checking bounds, causing silent
overflow wrap (e.g., 200 → -56). Now bounds are checked on the
original f64 delta before casting.

Added Delta16 fallback for medium-range deltas.

Fixes #123

Tests added:
- test_delta_overflow_fixed
- test_delta_at_i8_boundary  
- test_delta_requires_i16
- test_delta_requires_raw
```
