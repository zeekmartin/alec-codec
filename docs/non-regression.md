# ALEC — Stratégie de non-régression

## Philosophie

Les tests de non-régression garantissent que chaque modification du code ne casse pas les fonctionnalités existantes. Pour ALEC, c'est critique : une régression peut rendre des milliers de capteurs incapables de communiquer.

---

## Niveaux de tests

### Niveau 1 : Tests unitaires

Tests isolés de chaque composant.

```
tests/
├── unit/
│   ├── classifier/
│   │   ├── test_priority_assignment.rs
│   │   ├── test_threshold_detection.rs
│   │   └── test_edge_cases.rs
│   ├── encoder/
│   │   ├── test_delta_numeric.rs
│   │   ├── test_delta_symbolic.rs
│   │   └── test_message_format.rs
│   ├── context/
│   │   ├── test_dictionary_ops.rs
│   │   ├── test_pattern_promotion.rs
│   │   └── test_sync_diff.rs
│   └── decoder/
│       ├── test_reconstruction.rs
│       └── test_error_handling.rs
```

**Couverture cible** : > 80% des lignes de code

### Niveau 2 : Tests d'intégration

Tests des interactions entre composants.

```
tests/
├── integration/
│   ├── test_encode_decode_roundtrip.rs
│   ├── test_context_sync.rs
│   ├── test_request_response.rs
│   └── test_priority_flow.rs
```

### Niveau 3 : Tests end-to-end

Tests du système complet avec émetteur et récepteur réels (ou simulés).

```
tests/
├── e2e/
│   ├── test_simple_sensor.rs
│   ├── test_anomaly_detection.rs
│   ├── test_context_evolution.rs
│   ├── test_fleet_mode.rs
│   └── test_recovery_scenarios.rs
```

### Niveau 4 : Tests de performance

Benchmarks pour détecter les régressions de performance.

```
benches/
├── bench_encoding_speed.rs
├── bench_decoding_speed.rs
├── bench_compression_ratio.rs
├── bench_memory_usage.rs
└── bench_context_operations.rs
```

---

## Scénarios de référence

### Dataset de test standard

Jeux de données représentatifs pour garantir la reproductibilité :

| Dataset | Description | Taille | Source |
|---------|-------------|--------|--------|
| `temp_sensor_24h` | Température capteur, 24h, 1 mesure/min | 1440 points | Synthétique |
| `vibration_machine` | Vibrations industrielles, patterns répétitifs | 10000 points | Réel anonymisé |
| `gps_fleet_10` | Positions GPS de 10 véhicules, 1 journée | 50000 points | Synthétique |
| `medical_glucose` | Glycémie patient, 7 jours | 2000 points | Synthétique |
| `anomaly_injection` | Dataset normal + anomalies injectées | Variable | Généré |

### Scénarios critiques

```yaml
scenarios:
  - name: "Basic roundtrip"
    description: "Encode → transmit → decode sans perte"
    input: temp_sensor_24h
    expected: reconstruction parfaite
    
  - name: "Context cold start"
    description: "Premier échange, contexte vide"
    input: 100 premières mesures
    expected: fallback vers encodage brut, puis amélioration
    
  - name: "Context after learning"
    description: "Après 1000 échanges"
    input: 100 mesures similaires
    expected: compression > 90%
    
  - name: "Anomaly detection"
    description: "Détection correcte des anomalies"
    input: anomaly_injection
    expected: toutes anomalies classées P1/P2
    
  - name: "Sync recovery"
    description: "Récupération après désynchronisation"
    input: context corrompu + 100 mesures
    expected: resync automatique, pas de perte
    
  - name: "Request fulfillment"
    description: "Requête de détails honorée"
    input: événement P2 + REQ_DETAIL
    expected: données P4 transmises correctement
```

---

## Métriques surveillées

### Métriques fonctionnelles

| Métrique | Seuil acceptable | Action si dépassé |
|----------|------------------|-------------------|
| Taux de reconstruction | 100% (lossless) | Bloquer le merge |
| Anomalies détectées | 100% des injectées | Bloquer le merge |
| Sync réussies | > 99.9% | Investigation |

### Métriques de performance

| Métrique | Baseline | Tolérance | Action |
|----------|----------|-----------|--------|
| Temps encodage (1000 msg) | 50ms | +10% | Warning |
| Temps décodage (1000 msg) | 30ms | +10% | Warning |
| Ratio compression (après rodage) | 0.08 | +0.02 | Warning |
| Mémoire émetteur | 32KB | +20% | Bloquer |
| Mémoire récepteur | 1MB | +50% | Warning |

---

## Pipeline CI/CD

```yaml
# .github/workflows/ci.yml (exemple)

name: ALEC CI

on: [push, pull_request]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run unit tests
        run: cargo test --lib
      - name: Check coverage
        run: cargo tarpaulin --threshold 80

  integration-tests:
    runs-on: ubuntu-latest
    needs: unit-tests
    steps:
      - uses: actions/checkout@v4
      - name: Run integration tests
        run: cargo test --test '*'

  e2e-tests:
    runs-on: ubuntu-latest
    needs: integration-tests
    steps:
      - uses: actions/checkout@v4
      - name: Setup test environment
        run: ./scripts/setup_e2e.sh
      - name: Run E2E scenarios
        run: cargo test --features e2e

  benchmarks:
    runs-on: ubuntu-latest
    needs: e2e-tests
    steps:
      - uses: actions/checkout@v4
      - name: Run benchmarks
        run: cargo bench --bench '*' -- --save-baseline pr-${{ github.sha }}
      - name: Compare with main
        run: cargo bench --bench '*' -- --baseline main --threshold 10

  memory-check:
    runs-on: ubuntu-latest
    needs: unit-tests
    steps:
      - uses: actions/checkout@v4
      - name: Check memory usage
        run: ./scripts/check_memory.sh
```

---

## Procédure de validation

### Avant chaque commit

```bash
# Script pre-commit
#!/bin/bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --lib
```

### Avant chaque merge

1. Tous les tests CI passent
2. Revue de code par au moins 1 personne
3. Pas de régression de performance > 10%
4. Documentation mise à jour si nécessaire

### Avant chaque release

1. Suite E2E complète sur hardware cible
2. Tests de stress (24h en continu)
3. Tests de compatibilité ascendante
4. Validation des datasets de référence
5. Revue de sécurité

---

## Gestion des datasets

### Versionnage

Les datasets sont versionnés séparément du code :

```
datasets/
├── v1/
│   ├── temp_sensor_24h.bin
│   ├── vibration_machine.bin
│   └── manifest.json
├── v2/
│   └── ...
└── current -> v2
```

### Génération

Scripts reproductibles pour générer les datasets synthétiques :

```bash
# Générer le dataset température
python scripts/generate_dataset.py \
  --type temperature \
  --duration 24h \
  --interval 1m \
  --noise 0.1 \
  --output datasets/current/temp_sensor_24h.bin

# Générer les anomalies
python scripts/inject_anomalies.py \
  --input datasets/current/temp_sensor_24h.bin \
  --anomaly-rate 0.02 \
  --types spike,drift,missing \
  --output datasets/current/anomaly_injection.bin
```

---

## Debugging des régressions

### Outils disponibles

```bash
# Comparer deux versions
cargo run --bin alec-diff -- \
  --baseline v0.2.0 \
  --current HEAD \
  --dataset temp_sensor_24h

# Visualiser l'évolution du contexte
cargo run --bin alec-context-viz -- \
  --input trace.log \
  --output context_evolution.html

# Rejouer un scénario
cargo run --bin alec-replay -- \
  --scenario e2e/test_anomaly_detection \
  --verbose
```

### Checklist de debug

1. [ ] Identifier le commit qui a introduit la régression (`git bisect`)
2. [ ] Reproduire en local avec le même dataset
3. [ ] Activer les logs détaillés (`ALEC_LOG=trace`)
4. [ ] Comparer les traces avant/après
5. [ ] Écrire un test qui capture le bug
6. [ ] Fixer et vérifier que le test passe

---

## Golden files

Fichiers de référence pour valider les sorties exactes :

```
tests/golden/
├── encode_simple.golden       # Sortie attendue pour entrée simple
├── encode_with_context.golden # Sortie avec contexte établi
├── sync_diff.golden           # Format de synchronisation
└── message_format.golden      # Structure binaire des messages
```

Mise à jour des golden files (avec validation humaine) :

```bash
cargo test -- --update-golden
git diff tests/golden/
# Vérifier manuellement les changements
git add tests/golden/
```
