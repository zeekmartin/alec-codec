# Prompt 02 — Implémentation du Checksum

## Contexte

L'encodeur et le décodeur ont des champs `_include_checksum` et `_verify_checksum` qui sont préparés mais non implémentés. Le checksum permet de détecter les corruptions de données en transit.

## Objectif

Implémenter la vérification d'intégrité des messages avec :
1. Calcul de checksum à l'encodage
2. Vérification à la réception
3. Gestion des erreurs de checksum

## Spécification

### Algorithme

Utiliser **xxHash** (rapide, bonne distribution) ou **CRC32** (standard, simple).

Recommandation : xxHash via la crate `xxhash-rust` (déjà dans les dépendances).

### Format du message avec checksum

```
+----------------+------------------+------------+
|     Header     |     Payload      |  Checksum  |
|    (12 bytes)  |   (variable)     |  (4 bytes) |
+----------------+------------------+------------+
```

Le checksum couvre Header + Payload.

## Étapes

### 1. Modifier `protocol.rs`

Ajouter une constante et une méthode :

```rust
/// Taille du checksum en bytes
pub const CHECKSUM_SIZE: usize = 4;

impl EncodedMessage {
    /// Calculer le checksum du message
    pub fn compute_checksum(&self) -> u32 {
        use xxhash_rust::xxh32::xxh32;
        
        let mut data = Vec::with_capacity(MessageHeader::SIZE + self.payload.len());
        data.extend(self.header.to_bytes());
        data.extend(&self.payload);
        
        xxh32(&data, 0) // seed = 0
    }
    
    /// Sérialiser avec checksum
    pub fn to_bytes_with_checksum(&self) -> Vec<u8> {
        let mut bytes = self.to_bytes();
        let checksum = self.compute_checksum();
        bytes.extend(checksum.to_be_bytes());
        bytes
    }
    
    /// Désérialiser avec vérification du checksum
    pub fn from_bytes_with_checksum(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < MessageHeader::SIZE + CHECKSUM_SIZE {
            return Err(DecodeError::BufferTooShort { 
                needed: MessageHeader::SIZE + CHECKSUM_SIZE,
                available: bytes.len() 
            });
        }
        
        let checksum_offset = bytes.len() - CHECKSUM_SIZE;
        let expected = u32::from_be_bytes(
            bytes[checksum_offset..].try_into().unwrap()
        );
        
        let message = Self::from_bytes(&bytes[..checksum_offset])
            .ok_or(DecodeError::InvalidHeader)?;
            
        let actual = message.compute_checksum();
        
        if actual != expected {
            return Err(DecodeError::InvalidChecksum { expected, actual });
        }
        
        Ok(message)
    }
}
```

### 2. Modifier `encoder.rs`

Renommer `_include_checksum` → `include_checksum` et l'utiliser :

```rust
pub struct Encoder {
    sequence: u32,
    include_checksum: bool,
}

impl Encoder {
    /// Encode et retourne les bytes (avec ou sans checksum)
    pub fn encode_to_bytes(
        &mut self,
        data: &RawData,
        classification: &Classification,
        context: &Context,
    ) -> Vec<u8> {
        let message = self.encode(data, classification, context);
        
        if self.include_checksum {
            message.to_bytes_with_checksum()
        } else {
            message.to_bytes()
        }
    }
}
```

### 3. Modifier `decoder.rs`

Renommer `_verify_checksum` → `verify_checksum` et l'utiliser :

```rust
pub struct Decoder {
    verify_checksum: bool,
    last_sequence: Option<u32>,
}

impl Decoder {
    /// Décoder depuis des bytes (avec vérification optionnelle)
    pub fn decode_bytes(&mut self, bytes: &[u8], context: &Context) -> Result<DecodedData> {
        let message = if self.verify_checksum {
            EncodedMessage::from_bytes_with_checksum(bytes)?
        } else {
            EncodedMessage::from_bytes(bytes)
                .ok_or(DecodeError::InvalidHeader)?
        };
        
        self.decode(&message, context)
    }
}
```

### 4. Ajouter des tests

```rust
#[test]
fn test_checksum_roundtrip() {
    let mut encoder = Encoder::with_checksum();
    let mut decoder = Decoder::with_checksum_verification();
    let classifier = Classifier::default();
    let context = Context::new();
    
    let data = RawData::new(42.0, 0);
    let classification = classifier.classify(&data, &context);
    let bytes = encoder.encode_to_bytes(&data, &classification, &context);
    
    let decoded = decoder.decode_bytes(&bytes, &context).unwrap();
    assert!((decoded.value - data.value).abs() < 0.001);
}

#[test]
fn test_checksum_corruption_detected() {
    let mut encoder = Encoder::with_checksum();
    let mut decoder = Decoder::with_checksum_verification();
    let classifier = Classifier::default();
    let context = Context::new();
    
    let data = RawData::new(42.0, 0);
    let classification = classifier.classify(&data, &context);
    let mut bytes = encoder.encode_to_bytes(&data, &classification, &context);
    
    // Corrompre un byte
    bytes[5] ^= 0xFF;
    
    let result = decoder.decode_bytes(&bytes, &context);
    assert!(matches!(result, Err(AlecError::Decode(DecodeError::InvalidChecksum { .. }))));
}
```

## Livrables

- [ ] `protocol.rs` : méthodes checksum
- [ ] `encoder.rs` : `encode_to_bytes()` avec checksum
- [ ] `decoder.rs` : `decode_bytes()` avec vérification
- [ ] Tests de checksum (au moins 3)
- [ ] Documentation mise à jour

## Critères de succès

```bash
cargo test checksum  # Tous les tests checksum passent
cargo test  # Pas de régression (46+ tests)
```

## Prochaine étape

→ `03_metrics_compression.md`
