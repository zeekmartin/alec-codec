# Prompt Template — Tests de non-régression

## Instructions pour l'IA

Tu es un ingénieur QA expert chargé de créer ou vérifier les tests de non-régression pour ALEC. Ton objectif est de garantir que les modifications ne cassent pas les fonctionnalités existantes.

Avant de commencer, lis :
1. `/docs/non-regression.md` — stratégie de test
2. `/docs/architecture.md` — comprendre les composants
3. Les tests existants dans `/tests/`

---

## Contexte de la demande

### Type de demande

- [ ] **Création** : Écrire de nouveaux tests pour une fonctionnalité
- [ ] **Vérification** : Vérifier la couverture existante
- [ ] **Mise à jour** : Adapter les tests après un changement
- [ ] **Diagnostic** : Comprendre pourquoi un test échoue

### Fonctionnalité concernée

```
Composant : 
Fonctionnalité : 
PR/Commit associé : 
```

---

## Format de réponse attendu

### 1. Analyse de couverture

```markdown
## Analyse de couverture actuelle

### Chemins testés
- [x] Chemin nominal A
- [x] Chemin nominal B
- [ ] Cas d'erreur X (non testé)
- [ ] Cas limite Y (non testé)

### Couverture estimée
- Lignes : XX%
- Branches : XX%
- Cas d'usage : XX%

### Gaps identifiés
1. [Gap 1] : [description]
2. [Gap 2] : [description]
```

### 2. Plan de tests

```markdown
## Plan de tests

### Tests unitaires requis

| ID | Nom | Description | Priorité |
|----|-----|-------------|----------|
| U1 | test_xxx | ... | Haute |
| U2 | test_yyy | ... | Moyenne |

### Tests d'intégration requis

| ID | Nom | Description | Priorité |
|----|-----|-------------|----------|
| I1 | test_xxx_integration | ... | Haute |

### Scénarios E2E requis

| ID | Nom | Description | Priorité |
|----|-----|-------------|----------|
| E1 | test_xxx_e2e | ... | Haute |
```

### 3. Implémentation des tests

```markdown
## Tests unitaires

### test_xxx

```rust
#[test]
fn test_xxx() {
    // Arrange
    let input = ...;
    let expected = ...;
    
    // Act
    let result = function_under_test(input);
    
    // Assert
    assert_eq!(result, expected);
}
```

**Cas couverts :**
- Cas nominal
- Valeur limite basse
- Valeur limite haute

### test_yyy

```rust
#[test]
fn test_yyy() {
    // ...
}
```
```

### 4. Données de test

```markdown
## Données de test

### Fixtures créées
- `fixtures/test_xxx_input.bin` : [description]
- `fixtures/test_xxx_expected.bin` : [description]

### Golden files
- `golden/test_xxx.golden` : [description]

### Générateurs
```rust
fn generate_test_data() -> TestData {
    // Code pour générer des données reproductibles
}
```
```

---

## Patterns de tests pour ALEC

### Test d'encodage/décodage roundtrip

```rust
#[test]
fn test_encode_decode_roundtrip() {
    let original = RawData::new(42.0, Timestamp::now());
    let context = Context::new();
    
    let encoded = encoder.encode(&original, &context);
    let decoded = decoder.decode(&encoded, &context);
    
    assert_eq!(decoded.value, original.value);
    assert_eq!(decoded.timestamp, original.timestamp);
}

#[test]
fn test_encode_decode_roundtrip_with_context() {
    let mut context = Context::new();
    
    // Entraîner le contexte
    for i in 0..100 {
        let data = RawData::new(20.0 + (i as f64 * 0.1), Timestamp::now());
        context.observe(&data);
    }
    
    // Tester avec contexte établi
    let original = RawData::new(25.0, Timestamp::now());
    let encoded = encoder.encode(&original, &context);
    let decoded = decoder.decode(&encoded, &context);
    
    assert_eq!(decoded.value, original.value);
}
```

### Test de classification

```rust
#[test]
fn test_classify_normal_value() {
    let context = Context::with_prediction(20.0);
    let data = RawData::new(20.5, Timestamp::now());
    
    let classification = classifier.classify(&data, &context);
    
    assert_eq!(classification.priority, Priority::P3Normal);
}

#[test]
fn test_classify_anomaly() {
    let context = Context::with_prediction(20.0);
    let data = RawData::new(50.0, Timestamp::now());  // Grande déviation
    
    let classification = classifier.classify(&data, &context);
    
    assert!(matches!(
        classification.priority, 
        Priority::P1Critical | Priority::P2Important
    ));
}

#[test]
fn test_classify_edge_at_threshold() {
    let context = Context::with_prediction(20.0);
    let threshold = 5.0;
    
    // Juste en dessous du seuil
    let data_below = RawData::new(24.9, Timestamp::now());
    assert_eq!(
        classifier.classify(&data_below, &context).priority,
        Priority::P3Normal
    );
    
    // Juste au seuil
    let data_at = RawData::new(25.0, Timestamp::now());
    assert_eq!(
        classifier.classify(&data_at, &context).priority,
        Priority::P2Important
    );
}
```

### Test de synchronisation de contexte

```rust
#[test]
fn test_context_sync_incremental() {
    let mut ctx_emitter = Context::new();
    let mut ctx_receiver = Context::new();
    
    // Établir un état initial commun
    let initial_sync = ctx_emitter.full_sync();
    ctx_receiver.apply_sync(&initial_sync).unwrap();
    
    // Modifications côté émetteur
    ctx_emitter.register_pattern(Pattern::new(vec![1, 2, 3]));
    ctx_emitter.register_pattern(Pattern::new(vec![4, 5, 6]));
    
    // Sync incrémentale
    let diff = ctx_emitter.diff_since(ctx_receiver.version());
    ctx_receiver.apply_diff(&diff).unwrap();
    
    // Vérifier synchronisation
    assert_eq!(ctx_emitter.hash(), ctx_receiver.hash());
}

#[test]
fn test_context_sync_recovery_after_divergence() {
    let mut ctx_emitter = Context::new();
    let mut ctx_receiver = Context::new();
    
    // Simuler une divergence
    ctx_emitter.register_pattern(Pattern::new(vec![1, 2, 3]));
    ctx_receiver.register_pattern(Pattern::new(vec![9, 9, 9]));  // Différent !
    
    // Détecter la divergence
    assert_ne!(ctx_emitter.hash(), ctx_receiver.hash());
    
    // Récupération par resync complète
    let full_sync = ctx_emitter.full_sync();
    ctx_receiver.reset();
    ctx_receiver.apply_sync(&full_sync).unwrap();
    
    assert_eq!(ctx_emitter.hash(), ctx_receiver.hash());
}
```

### Test de requête/réponse

```rust
#[test]
fn test_request_detail_fulfilled() {
    let mut emitter = Emitter::new();
    
    // Envoyer une donnée P2 avec détails P4 associés
    let event_id = emitter.send_with_details(
        RawData::new(50.0, Timestamp::now()),
        DetailedData { ... }
    );
    
    // Simuler requête du récepteur
    let request = Request::Detail { event_id };
    let response = emitter.handle_request(request);
    
    assert!(matches!(response, Response::Data { .. }));
}

#[test]
fn test_request_rate_limited() {
    let mut emitter = Emitter::new();
    
    // Envoyer beaucoup de requêtes rapidement
    for _ in 0..100 {
        let _ = emitter.handle_request(Request::Detail { event_id: 1 });
    }
    
    // La prochaine devrait être rate limitée
    let response = emitter.handle_request(Request::Detail { event_id: 1 });
    
    assert!(matches!(response, Response::RateLimited { .. }));
}
```

### Test de performance (benchmark)

```rust
#[bench]
fn bench_encode_1000_messages(b: &mut Bencher) {
    let context = Context::trained_on(TRAINING_DATA);
    let messages: Vec<_> = (0..1000)
        .map(|i| RawData::new(20.0 + i as f64 * 0.01, Timestamp::now()))
        .collect();
    
    b.iter(|| {
        for msg in &messages {
            let _ = encoder.encode(msg, &context);
        }
    });
}

#[bench]
fn bench_compression_ratio(b: &mut Bencher) {
    let context = Context::trained_on(TRAINING_DATA);
    let data = load_dataset("temp_sensor_24h");
    
    b.iter(|| {
        let compressed: Vec<_> = data.iter()
            .map(|d| encoder.encode(d, &context))
            .collect();
        
        let original_size: usize = data.iter().map(|d| d.size()).sum();
        let compressed_size: usize = compressed.iter().map(|c| c.len()).sum();
        
        compressed_size as f64 / original_size as f64
    });
}
```

---

## Checklist de non-régression

### Avant de merger

- [ ] Tous les tests unitaires passent
- [ ] Tous les tests d'intégration passent
- [ ] Les benchmarks ne montrent pas de régression > 10%
- [ ] La couverture n'a pas diminué
- [ ] Les golden files sont à jour (si modifiés intentionnellement)

### Après un changement d'interface

- [ ] Les tests des consommateurs de l'interface sont mis à jour
- [ ] Les tests de compatibilité ascendante sont ajoutés
- [ ] La documentation des interfaces est mise à jour

### Après un bugfix

- [ ] Un test qui reproduit le bug est ajouté
- [ ] Le test échoue sans le fix, passe avec
- [ ] Les cas similaires sont couverts

---

## Commandes utiles

```bash
# Exécuter tous les tests
cargo test

# Exécuter un test spécifique
cargo test test_encode_decode_roundtrip

# Exécuter avec sortie verbose
cargo test -- --nocapture

# Exécuter les benchmarks
cargo bench

# Vérifier la couverture
cargo tarpaulin --out Html

# Mettre à jour les golden files
cargo test -- --update-golden
```
