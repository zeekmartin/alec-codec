# Prompt 04 — Contexte Évolutif (v0.2.0)

## Contexte

Le contexte actuel est statique : les patterns sont ajoutés mais jamais optimisés. Pour v0.2.0, le contexte doit évoluer automatiquement :
- Promouvoir les patterns fréquents (codes plus courts)
- Élager les patterns rares (économiser la mémoire)
- Améliorer les prédictions

## Objectif

Transformer le contexte statique en contexte adaptatif qui :
1. Compte la fréquence d'utilisation des patterns
2. Réorganise le dictionnaire (fréquent = ID court)
3. Supprime les patterns inutilisés
4. Améliore le modèle prédictif

## Spécification

### Comptage de fréquence

Chaque pattern a un compteur d'utilisation :
```rust
struct PatternEntry {
    pattern: Pattern,
    frequency: u32,
    last_used: u64,  // timestamp
}
```

### Promotion/Rétrogradation

- **Promotion** : Si `frequency > threshold_high`, attribuer un ID plus court
- **Rétrogradation** : Si `frequency < threshold_low` après N observations, reléguer

### Élagage

Supprimer les patterns où :
- `last_used` > max_age (ex: 1 heure)
- `frequency` < min_frequency (ex: 2)
- Dictionnaire plein et nouveau pattern plus pertinent

## Étapes

### 1. Modifier la structure Pattern

Dans `src/context.rs` :

```rust
/// A pattern with usage statistics
#[derive(Debug, Clone)]
pub struct PatternEntry {
    /// The pattern data
    pub pattern: Pattern,
    /// Usage frequency counter
    pub frequency: u32,
    /// Last time this pattern was used (observation count)
    pub last_used: u64,
    /// When the pattern was created
    pub created_at: u64,
}

impl PatternEntry {
    pub fn new(pattern: Pattern, timestamp: u64) -> Self {
        Self {
            pattern,
            frequency: 1,
            last_used: timestamp,
            created_at: timestamp,
        }
    }
    
    pub fn touch(&mut self, timestamp: u64) {
        self.frequency = self.frequency.saturating_add(1);
        self.last_used = timestamp;
    }
    
    /// Calculate a score for this pattern (higher = more valuable)
    pub fn score(&self, current_time: u64) -> f64 {
        let recency = 1.0 / (1.0 + (current_time - self.last_used) as f64 / 1000.0);
        let freq_score = (self.frequency as f64).ln();
        freq_score * recency
    }
}
```

### 2. Ajouter la configuration d'évolution

```rust
/// Configuration for context evolution
#[derive(Debug, Clone)]
pub struct EvolutionConfig {
    /// Maximum number of patterns to keep
    pub max_patterns: usize,
    /// Minimum frequency to keep a pattern
    pub min_frequency: u32,
    /// Maximum age (in observations) before pruning
    pub max_age: u64,
    /// How often to run evolution (every N observations)
    pub evolution_interval: u64,
    /// Threshold for promotion (frequency)
    pub promotion_threshold: u32,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            max_patterns: 256,
            min_frequency: 2,
            max_age: 10000,
            evolution_interval: 100,
            promotion_threshold: 10,
        }
    }
}
```

### 3. Implémenter l'évolution du contexte

```rust
impl Context {
    /// Run context evolution (pruning + reordering)
    pub fn evolve(&mut self) {
        let current_time = self.observation_count;
        
        // 1. Prune old/unused patterns
        self.prune_patterns(current_time);
        
        // 2. Reorder by score (frequent patterns get lower IDs)
        self.reorder_patterns(current_time);
        
        // 3. Increment version
        self.version += 1;
    }
    
    fn prune_patterns(&mut self, current_time: u64) {
        let config = &self.evolution_config;
        
        self.patterns.retain(|_, entry| {
            let age = current_time - entry.last_used;
            entry.frequency >= config.min_frequency && age <= config.max_age
        });
    }
    
    fn reorder_patterns(&mut self, current_time: u64) {
        // Collect and sort by score
        let mut entries: Vec<_> = self.patterns.drain().collect();
        entries.sort_by(|a, b| {
            b.1.score(current_time)
                .partial_cmp(&a.1.score(current_time))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Reassign IDs (best patterns get lowest IDs)
        for (new_id, (_, entry)) in entries.into_iter().enumerate() {
            self.patterns.insert(new_id as u32, entry);
        }
    }
    
    /// Observe data and potentially trigger evolution
    pub fn observe(&mut self, data: &RawData) {
        self.observation_count += 1;
        
        // Update source tracking
        self.update_source(data);
        
        // Check if evolution is needed
        if self.observation_count % self.evolution_config.evolution_interval == 0 {
            self.evolve();
        }
    }
}
```

### 4. Améliorer le modèle prédictif

Remplacer "dernière valeur" par "moyenne mobile exponentielle" :

```rust
/// Source tracking with EMA prediction
#[derive(Debug, Clone)]
pub struct SourceState {
    /// Last observed value
    pub last_value: f64,
    /// Exponential moving average
    pub ema: f64,
    /// EMA alpha (smoothing factor, 0-1)
    pub ema_alpha: f64,
    /// Observation count for this source
    pub count: u64,
}

impl SourceState {
    pub fn new(value: f64) -> Self {
        Self {
            last_value: value,
            ema: value,
            ema_alpha: 0.3,  // Configurable
            count: 1,
        }
    }
    
    pub fn update(&mut self, value: f64) {
        self.last_value = value;
        self.ema = self.ema_alpha * value + (1.0 - self.ema_alpha) * self.ema;
        self.count += 1;
    }
    
    /// Predict next value using EMA
    pub fn predict(&self) -> f64 {
        self.ema
    }
}
```

### 5. Ajouter des tests

```rust
#[test]
fn test_pattern_pruning() {
    let mut context = Context::with_config(EvolutionConfig {
        min_frequency: 3,
        max_age: 100,
        ..Default::default()
    });
    
    // Add pattern used only once
    context.register_pattern(Pattern::new(42.0));
    
    // Simulate time passing
    for _ in 0..150 {
        context.observe(&RawData::new(20.0, 0));
    }
    
    // Pattern should be pruned
    assert_eq!(context.pattern_count(), 0);
}

#[test]
fn test_pattern_promotion() {
    let mut context = Context::new();
    
    // Use same value many times
    for i in 0..100 {
        context.observe(&RawData::new(42.0, i));
    }
    
    // Should have high-scoring pattern with low ID
    let pattern = context.get_pattern(0);
    assert!(pattern.is_some());
}

#[test]
fn test_ema_prediction() {
    let mut context = Context::new();
    
    // Observe trending values
    for i in 0..10 {
        context.observe(&RawData::new(20.0 + i as f64, i as u64));
    }
    
    // EMA should predict around recent values, not exactly last
    let prediction = context.predict(0).unwrap();
    assert!(prediction.value > 25.0 && prediction.value < 29.0);
}
```

## Livrables

- [ ] `PatternEntry` avec fréquence et timestamp
- [ ] `EvolutionConfig` configurable
- [ ] `Context::evolve()` avec pruning et reordering
- [ ] `SourceState` avec EMA
- [ ] Tests d'évolution (au moins 5)
- [ ] Mise à jour de `todo.md`

## Critères de succès

```bash
cargo test context  # Tous les tests passent
cargo test evolution  # Tests spécifiques évolution
```

Le contexte doit :
- Élager les patterns inutilisés
- Garder les patterns fréquents avec des IDs courts
- Prédire avec EMA plutôt que dernière valeur

## Prochaine étape

→ `05_synchronisation_auto.md` (v0.3.0)
