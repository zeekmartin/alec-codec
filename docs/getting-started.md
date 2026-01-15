# ALEC — Guide de démarrage

Ce guide vous accompagne dans vos premiers pas avec ALEC, de l'installation à votre première application fonctionnelle.

---

## Table des matières

1. [Prérequis](#prérequis)
2. [Installation](#installation)
3. [Concepts clés](#concepts-clés)
4. [Tutoriel : Premier capteur](#tutoriel--premier-capteur)
5. [Tutoriel : Communication émetteur-récepteur](#tutoriel--communication-émetteur-récepteur)
6. [Tutoriel : Contexte évolutif](#tutoriel--contexte-évolutif)
7. [Prochaines étapes](#prochaines-étapes)

---

## Prérequis

### Environnement de développement

| Composant | Version minimum | Recommandé |
|-----------|-----------------|------------|
| Rust | 1.70 | 1.75+ |
| Cargo | 1.70 | 1.75+ |
| Git | 2.0 | 2.40+ |

### Vérifier l'installation

```bash
# Vérifier Rust
rustc --version
# rustc 1.75.0 (82e1608df 2023-12-21)

# Vérifier Cargo
cargo --version
# cargo 1.75.0 (1d8b05cdd 2023-11-20)
```

### Installer Rust (si nécessaire)

```bash
# Linux / macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows
# Télécharger rustup-init.exe depuis https://rustup.rs
```

---

## Installation

### Cloner le projet

```bash
git clone https://github.com/votre-org/alec-codec.git
cd alec-codec
```

### Compiler

```bash
# Mode debug (compilation rapide)
cargo build

# Mode release (optimisé)
cargo build --release
```

### Vérifier l'installation

```bash
# Lancer les tests
cargo test

# Sortie attendue :
# running 42 tests
# test ... ok
# test result: ok. 42 passed; 0 failed
```

### Structure du projet

```
alec-codec/
├── src/
│   ├── lib.rs          # Point d'entrée de la bibliothèque
│   ├── encoder.rs      # Encodage des données
│   ├── decoder.rs      # Décodage des messages
│   ├── classifier.rs   # Classification par priorité
│   ├── context.rs      # Contexte partagé
│   └── channel.rs      # Abstraction du canal
├── examples/
│   ├── simple_sensor.rs
│   ├── emitter_receiver.rs
│   └── fleet_mode.rs
├── tests/
└── benches/
```

---

## Concepts clés

Avant de coder, comprenez ces 4 concepts fondamentaux :

### 1. Données brutes (RawData)

Ce que votre capteur produit :

```rust
let data = RawData {
    source_id: 42,        // Identifiant du capteur
    timestamp: 1705312800, // Unix timestamp
    value: 22.5,          // La valeur mesurée
};
```

### 2. Contexte partagé (Context)

Un "dictionnaire" qui grandit avec le temps :

```rust
let mut context = Context::new();

// Le contexte apprend des patterns
context.observe(&data);

// Le contexte prédit la prochaine valeur
let prediction = context.predict(42); // → ~22.5
```

### 3. Classification (Priority)

Chaque donnée reçoit une priorité :

```rust
// P1: Critique (alerte immédiate)
// P2: Important (anomalie)
// P3: Normal (mesure standard)
// P4: Différé (sur demande)
// P5: Jetable (debug)
```

### 4. Messages encodés

Ce qui transite sur le canal :

```rust
// Donnée brute: 24 octets
let data = RawData::new(22.5, timestamp);

// Message encodé: 4 octets (après apprentissage)
let message = encoder.encode(&data, &context);
```

---

## Tutoriel : Premier capteur

Créons un capteur de température simple.

### Étape 1 : Créer le fichier

```bash
# Dans le dossier examples/
touch examples/my_first_sensor.rs
```

### Étape 2 : Code minimal

```rust
// examples/my_first_sensor.rs

use alec::{RawData, Encoder, Context, Classifier};

fn main() {
    // Initialisation
    let encoder = Encoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();
    
    // Simuler 10 mesures de température
    let temperatures = vec![
        20.0, 20.2, 20.1, 20.3, 20.2,  // Valeurs normales
        20.4, 20.3, 20.5, 20.4,
        35.0  // Anomalie !
    ];
    
    println!("=== Simulation capteur température ===\n");
    
    for (i, temp) in temperatures.iter().enumerate() {
        // Créer la donnée brute
        let data = RawData::new(*temp, i as u64);
        
        // Classifier
        let classification = classifier.classify(&data, &context);
        
        // Encoder
        let message = encoder.encode(&data, &context);
        
        // Afficher les résultats
        println!(
            "Mesure {}: {:.1}°C | Priorité: {:?} | Taille: {} octets",
            i + 1,
            temp,
            classification.priority,
            message.len()
        );
        
        // Mettre à jour le contexte
        context.observe(&data);
    }
    
    println!("\n=== Fin de simulation ===");
}
```

### Étape 3 : Exécuter

```bash
cargo run --example my_first_sensor
```

### Sortie attendue

```
=== Simulation capteur température ===

Mesure 1: 20.0°C | Priorité: P3Normal | Taille: 12 octets
Mesure 2: 20.2°C | Priorité: P3Normal | Taille: 6 octets
Mesure 3: 20.1°C | Priorité: P4Deferred | Taille: 4 octets
Mesure 4: 20.3°C | Priorité: P4Deferred | Taille: 4 octets
Mesure 5: 20.2°C | Priorité: P5Disposable | Taille: 2 octets
Mesure 6: 20.4°C | Priorité: P4Deferred | Taille: 4 octets
Mesure 7: 20.3°C | Priorité: P5Disposable | Taille: 2 octets
Mesure 8: 20.5°C | Priorité: P4Deferred | Taille: 4 octets
Mesure 9: 20.4°C | Priorité: P5Disposable | Taille: 2 octets
Mesure 10: 35.0°C | Priorité: P1Critical | Taille: 14 octets

=== Fin de simulation ===
```

**Observations** :
- La taille des messages diminue après apprentissage
- L'anomalie (35°C) est détectée et classée P1
- Les valeurs identiques au passé récent sont P5 (jetables)

---

## Tutoriel : Communication émetteur-récepteur

Simulons une vraie communication entre deux entités.

### Code complet

```rust
// examples/emitter_receiver.rs

use alec::{RawData, Encoder, Decoder, Context, Classifier};
use std::collections::VecDeque;

fn main() {
    // === CÔTÉ ÉMETTEUR ===
    let encoder = Encoder::new();
    let classifier = Classifier::default();
    let mut ctx_emitter = Context::new();
    
    // === CÔTÉ RÉCEPTEUR ===
    let decoder = Decoder::new();
    let mut ctx_receiver = Context::new();
    
    // === CANAL DE COMMUNICATION (simulé) ===
    let mut channel: VecDeque<Vec<u8>> = VecDeque::new();
    
    // === SIMULATION ===
    let measurements = generate_realistic_data();
    
    println!("=== Communication Émetteur → Récepteur ===\n");
    println!("{:<6} {:<10} {:<12} {:<10} {:<10}", 
             "N°", "Original", "Priorité", "Taille", "Reconstr.");
    println!("{}", "-".repeat(50));
    
    let mut total_original = 0;
    let mut total_compressed = 0;
    
    for (i, value) in measurements.iter().enumerate() {
        // --- Émetteur ---
        let data = RawData::new(*value, i as u64);
        let classification = classifier.classify(&data, &ctx_emitter);
        
        // N'envoyer que P1, P2, P3 (pas P4, P5)
        if classification.priority.should_transmit() {
            let message = encoder.encode(&data, &ctx_emitter);
            
            total_original += 8; // f64 = 8 octets
            total_compressed += message.len();
            
            // Envoyer sur le canal
            channel.push_back(message.to_bytes());
        }
        
        ctx_emitter.observe(&data);
        
        // --- Récepteur ---
        if let Some(bytes) = channel.pop_front() {
            let message = EncodedMessage::from_bytes(&bytes);
            let decoded = decoder.decode(&message, &ctx_receiver).unwrap();
            
            ctx_receiver.observe(&decoded);
            
            println!(
                "{:<6} {:<10.2} {:<12?} {:<10} {:<10.2}",
                i + 1,
                value,
                classification.priority,
                bytes.len(),
                decoded.value
            );
        }
    }
    
    println!("{}", "-".repeat(50));
    println!("\nStatistiques:");
    println!("  Données originales: {} octets", total_original);
    println!("  Données transmises: {} octets", total_compressed);
    println!("  Ratio compression: {:.1}%", 
             (1.0 - total_compressed as f64 / total_original as f64) * 100.0);
}

fn generate_realistic_data() -> Vec<f64> {
    // Simule 24h de données (1 mesure / 15 min = 96 mesures)
    let mut data = Vec::new();
    let mut temp = 18.0;
    
    for hour in 0..24 {
        for _ in 0..4 {
            // Variation naturelle
            temp += (rand::random::<f64>() - 0.5) * 0.3;
            
            // Pattern journalier
            if hour >= 8 && hour <= 18 {
                temp += 0.1; // Plus chaud en journée
            } else {
                temp -= 0.05;
            }
            
            // Anomalie à 14h
            if hour == 14 && data.len() == 56 {
                temp += 10.0; // Spike !
            }
            
            data.push(temp);
        }
    }
    
    data
}
```

---

## Tutoriel : Contexte évolutif

Voyons comment le contexte s'améliore avec le temps.

### Code

```rust
// examples/context_evolution.rs

use alec::{Context, RawData};

fn main() {
    let mut context = Context::new();
    
    println!("=== Évolution du contexte partagé ===\n");
    
    // Phase 1: Apprentissage initial
    println!("Phase 1: Apprentissage (100 mesures)");
    for i in 0..100 {
        let value = 20.0 + (i as f64 * 0.01);
        let data = RawData::new(value, i);
        context.observe(&data);
    }
    
    println!("  Patterns appris: {}", context.pattern_count());
    println!("  Modèle prédictif: {:?}", context.model_type());
    
    // Test de prédiction
    let prediction = context.predict(0).unwrap();
    println!("  Prédiction prochaine valeur: {:.2}", prediction.value);
    println!("  Confiance: {:.0}%", prediction.confidence * 100.0);
    
    // Phase 2: Utilisation
    println!("\nPhase 2: Utilisation");
    
    let test_values = vec![21.0, 21.5, 25.0, 21.1];
    for value in test_values {
        let prediction = context.predict(0).unwrap();
        let delta = (value - prediction.value).abs();
        let relative_delta = delta / prediction.value;
        
        println!(
            "  Valeur: {:.1} | Prédit: {:.1} | Écart: {:.1}%",
            value,
            prediction.value,
            relative_delta * 100.0
        );
    }
    
    // Phase 3: Stats du contexte
    println!("\nPhase 3: État du contexte");
    println!("  Version: {}", context.version());
    println!("  Hash: {:016x}", context.hash());
    println!("  Taille mémoire: {} octets", context.memory_usage());
    
    // Exporter pour synchronisation
    let export = context.export_diff(0);
    println!("  Taille export: {} octets", export.len());
}
```

### Sortie attendue

```
=== Évolution du contexte partagé ===

Phase 1: Apprentissage (100 mesures)
  Patterns appris: 12
  Modèle prédictif: LinearRegression
  Prédiction prochaine valeur: 21.00
  Confiance: 94%

Phase 2: Utilisation
  Valeur: 21.0 | Prédit: 21.0 | Écart: 0.0%
  Valeur: 21.5 | Prédit: 21.0 | Écart: 2.4%
  Valeur: 25.0 | Prédit: 21.0 | Écart: 19.0%  ← Anomalie !
  Valeur: 21.1 | Prédit: 21.0 | Écart: 0.5%

Phase 3: État du contexte
  Version: 100
  Hash: a7f3b2c1d4e5f6a7
  Taille mémoire: 2048 octets
  Taille export: 256 octets
```

---

## Prochaines étapes

### 1. Approfondir la documentation

- [Architecture complète](architecture.md)
- [Référence du protocole](protocol-reference.md)
- [Cas d'usage détaillés](applications.md)

### 2. Expérimenter

```bash
# Lancer tous les exemples
cargo run --example simple_sensor
cargo run --example emitter_receiver
cargo run --example fleet_mode

# Lancer les benchmarks
cargo bench
```

### 3. Intégrer dans votre projet

```toml
# Cargo.toml de votre projet
[dependencies]
alec = { git = "https://github.com/votre-org/alec-codec.git" }
```

### 4. Contribuer

Consultez [CONTRIBUTING.md](../CONTRIBUTING.md) pour :
- Signaler un bug
- Proposer une fonctionnalité
- Soumettre du code

---

## Besoin d'aide ?

- 📖 [FAQ](faq.md) — Questions fréquentes
- 📚 [Glossaire](glossary.md) — Définitions des termes
- 🐛 [Issues GitHub](https://github.com/votre-org/alec-codec/issues) — Signaler un problème
