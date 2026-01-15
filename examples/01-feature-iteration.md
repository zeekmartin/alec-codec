# Exemple — Itération de fonctionnalité

## Contexte

Cet exemple montre comment ajouter une nouvelle fonctionnalité à ALEC en suivant le workflow défini.

**Feature** : Support des capteurs multi-valeurs

---

## Étape 1 : Demande initiale

```
FEATURE: Support des capteurs multi-valeurs

Description: Permettre à un émetteur d'envoyer plusieurs valeurs 
dans un seul message (ex: température + humidité + pression).

Motivation: Réduire l'overhead des headers quand un capteur 
produit plusieurs métriques simultanément.

Composants impactés:
- [x] Encodeur
- [x] Décodeur
- [x] Contexte partagé

Contraintes:
- [x] Doit fonctionner sur émetteur contraint
- [x] Doit être rétrocompatible
```

---

## Étape 2 : Analyse d'impact (réponse IA)

### Composants modifiés

1. **Encodeur** (`src/encoder.rs`)
   - Nouvelle méthode `encode_multi()`
   - Nouveau type de message `MessageType::MultiData`

2. **Décodeur** (`src/decoder.rs`)
   - Gestion du nouveau type de message
   - Reconstruction des valeurs multiples

3. **Contexte** (`src/context.rs`)
   - Patterns multi-valeurs (corrélations entre métriques)
   - Prédiction jointe

### Risques identifiés

| Risque | Impact | Mitigation |
|--------|--------|------------|
| Augmentation taille du contexte | Moyen | Limiter le nombre de combinaisons |
| Complexité de synchronisation | Faible | Réutiliser le mécanisme existant |
| Incompatibilité avec anciens récepteurs | Élevé | Négociation de version au handshake |

---

## Étape 3 : Spécification technique

### Nouvelles structures

```rust
/// Représente un ensemble de valeurs d'un même capteur
pub struct MultiValue {
    pub source_id: u32,
    pub timestamp: u64,
    pub values: Vec<NamedValue>,
}

pub struct NamedValue {
    pub name_id: u16,      // ID du nom dans le dictionnaire
    pub value: f64,
}

/// Pattern pour les corrélations multi-valeurs
pub struct MultiPattern {
    pub names: Vec<u16>,           // Les métriques corrélées
    pub typical_ratios: Vec<f64>,  // Ratios typiques entre elles
}
```

### Format de message

```
┌─────────┬──────────┬─────────┬─────────┬───────────────────────┐
│ Header  │ Sequence │ Timestamp│ Count   │ Values...             │
│ (0x08)  │ (u32)    │ (u32)   │ (u8)    │ (variable)            │
└─────────┴──────────┴─────────┴─────────┴───────────────────────┘

Chaque valeur :
┌───────────┬───────────────┬─────────────────────────────────────┐
│ Name ID   │ Encoding Type │ Value                               │
│ (u16)     │ (u8)          │ (variable selon encoding)           │
└───────────┴───────────────┴─────────────────────────────────────┘
```

---

## Étape 4 : Implémentation

### Fichier `src/encoder.rs`

```rust
impl Encoder {
    /// Encode plusieurs valeurs dans un seul message
    pub fn encode_multi(
        &self,
        values: &MultiValue,
        context: &Context,
    ) -> EncodedMessage {
        let mut payload = Vec::new();
        
        // Nombre de valeurs
        payload.push(values.values.len() as u8);
        
        // Encoder chaque valeur
        for named_value in &values.values {
            // Name ID (2 bytes, big endian)
            payload.extend_from_slice(&named_value.name_id.to_be_bytes());
            
            // Obtenir la prédiction pour cette métrique
            let prediction = context.predict_named(
                values.source_id, 
                named_value.name_id
            );
            
            // Choisir l'encodage optimal
            let (encoding_type, encoded_value) = self.choose_encoding(
                named_value.value,
                prediction,
            );
            
            payload.push(encoding_type);
            payload.extend(encoded_value);
        }
        
        EncodedMessage {
            header: MessageHeader {
                version: PROTOCOL_VERSION,
                message_type: MessageType::MultiData,
                priority: Priority::P3Normal,  // Calculé ailleurs
                sequence: self.next_sequence(),
                timestamp: values.timestamp,
                context_version: context.version(),
            },
            payload,
            checksum: 0,  // Calculé après
        }
    }
}
```

### Fichier `src/decoder.rs`

```rust
impl Decoder {
    /// Décode un message multi-valeurs
    pub fn decode_multi(
        &self,
        message: &EncodedMessage,
        context: &Context,
    ) -> Result<MultiValue, DecodeError> {
        let payload = &message.payload;
        let mut cursor = 0;
        
        // Nombre de valeurs
        let count = payload[cursor] as usize;
        cursor += 1;
        
        let mut values = Vec::with_capacity(count);
        
        for _ in 0..count {
            // Name ID
            let name_id = u16::from_be_bytes([
                payload[cursor], 
                payload[cursor + 1]
            ]);
            cursor += 2;
            
            // Encoding type
            let encoding_type = payload[cursor];
            cursor += 1;
            
            // Décoder la valeur
            let (value, bytes_read) = self.decode_value(
                encoding_type,
                &payload[cursor..],
                context.predict_named(message.source_id(), name_id),
            )?;
            cursor += bytes_read;
            
            values.push(NamedValue { name_id, value });
        }
        
        Ok(MultiValue {
            source_id: message.source_id(),
            timestamp: message.header.timestamp,
            values,
        })
    }
}
```

---

## Étape 5 : Tests

### Test unitaire de roundtrip

```rust
#[test]
fn test_multi_value_roundtrip() {
    let encoder = Encoder::new();
    let decoder = Decoder::new();
    let context = Context::new();
    
    let original = MultiValue {
        source_id: 1,
        timestamp: 1234567890,
        values: vec![
            NamedValue { name_id: 1, value: 22.5 },  // température
            NamedValue { name_id: 2, value: 65.0 },  // humidité
            NamedValue { name_id: 3, value: 1013.25 }, // pression
        ],
    };
    
    let encoded = encoder.encode_multi(&original, &context);
    let decoded = decoder.decode_multi(&encoded, &context).unwrap();
    
    assert_eq!(decoded.source_id, original.source_id);
    assert_eq!(decoded.values.len(), original.values.len());
    for (dec, orig) in decoded.values.iter().zip(&original.values) {
        assert_eq!(dec.name_id, orig.name_id);
        assert!((dec.value - orig.value).abs() < 0.001);
    }
}
```

### Test de compression

```rust
#[test]
fn test_multi_value_compression_better_than_separate() {
    let encoder = Encoder::new();
    let context = Context::trained();  // Contexte avec patterns appris
    
    // Trois valeurs séparées
    let separate_size: usize = [22.5, 65.0, 1013.25]
        .iter()
        .map(|&v| encoder.encode_single(v, &context).len())
        .sum();
    
    // Les mêmes en multi-valeur
    let multi = MultiValue {
        source_id: 1,
        timestamp: 0,
        values: vec![
            NamedValue { name_id: 1, value: 22.5 },
            NamedValue { name_id: 2, value: 65.0 },
            NamedValue { name_id: 3, value: 1013.25 },
        ],
    };
    let multi_size = encoder.encode_multi(&multi, &context).len();
    
    // Multi-valeur doit être plus compact (1 seul header)
    assert!(multi_size < separate_size);
}
```

### Test de compatibilité

```rust
#[test]
fn test_old_receiver_ignores_multi_value() {
    let encoder = Encoder::new();
    let old_decoder = DecoderV1::new();  // Ancienne version
    let context = Context::new();
    
    let multi = MultiValue { /* ... */ };
    let encoded = encoder.encode_multi(&multi, &context);
    
    // L'ancien décodeur doit ignorer gracieusement
    let result = old_decoder.decode(&encoded, &context);
    assert!(matches!(result, Err(DecodeError::UnknownMessageType)));
}
```

---

## Étape 6 : Documentation mise à jour

### Ajout dans architecture.md

```markdown
### Messages multi-valeurs (v0.3+)

ALEC supporte l'envoi de plusieurs métriques dans un seul message,
réduisant l'overhead des headers pour les capteurs multi-métriques.

**Cas d'usage :**
- Station météo (temp, humidité, pression, vent)
- Capteur industriel (vibration X, Y, Z)
- Moniteur médical (pouls, SpO2, température)

**Gain typique :**
- 3 valeurs : ~40% réduction vs messages séparés
- 10 valeurs : ~60% réduction vs messages séparés
```

---

## Résumé du workflow

1. ✅ Demande formulée avec le template
2. ✅ Analyse d'impact documentée
3. ✅ Spécification technique rédigée
4. ✅ Code implémenté
5. ✅ Tests écrits et passent
6. ✅ Documentation mise à jour
7. ⏳ Review et merge
