# Prompt 06 — Mode Flotte (v0.4.0)

## Contexte

Pour v0.4.0, ALEC doit supporter plusieurs émetteurs vers un récepteur central :
- Contextes individuels par émetteur
- Contexte partagé de flotte
- Apprentissage collectif
- Détection d'anomalies par comparaison

## Objectif

Créer une architecture multi-émetteurs avec :
1. Gestion de contextes par émetteur
2. Agrégation des patterns communs
3. Détection d'anomalies cross-fleet
4. Dashboard de monitoring

## Spécification

### Architecture

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Émetteur 1  │     │  Émetteur 2  │     │  Émetteur N  │
│  Context_1   │     │  Context_2   │     │  Context_N   │
└──────┬───────┘     └──────┬───────┘     └──────┬───────┘
       │                    │                    │
       └────────────────────┼────────────────────┘
                            │
                   ┌────────▼────────┐
                   │   Récepteur     │
                   │  FleetManager   │
                   │                 │
                   │ ┌─────────────┐ │
                   │ │Fleet Context│ │
                   │ │  (shared)   │ │
                   │ └─────────────┘ │
                   │                 │
                   │ ┌───┐┌───┐┌───┐ │
                   │ │C1 ││C2 ││CN │ │
                   │ └───┘└───┘└───┘ │
                   └─────────────────┘
```

## Étapes

### 1. Créer `src/fleet.rs`

```rust
//! Fleet management for multi-emitter scenarios
//!
//! Manages multiple contexts and provides cross-fleet analytics.

use std::collections::HashMap;
use crate::context::Context;
use crate::protocol::{RawData, Priority, Pattern};
use crate::classifier::{Classification, Classifier};
use crate::decoder::Decoder;
use crate::error::Result;

/// Unique identifier for an emitter
pub type EmitterId = u32;

/// Fleet-wide statistics
#[derive(Debug, Clone, Default)]
pub struct FleetStats {
    /// Number of active emitters
    pub emitter_count: usize,
    /// Total messages received
    pub total_messages: u64,
    /// Messages per priority
    pub priority_distribution: HashMap<Priority, u64>,
    /// Anomalies detected
    pub anomaly_count: u64,
    /// Cross-fleet anomalies (emitter behaving differently)
    pub cross_fleet_anomalies: u64,
}

/// Manages a fleet of emitters
#[derive(Debug)]
pub struct FleetManager {
    /// Individual contexts per emitter
    emitter_contexts: HashMap<EmitterId, EmitterState>,
    /// Shared fleet-wide context (common patterns)
    fleet_context: Context,
    /// Classifier for fleet-wide analysis
    classifier: Classifier,
    /// Decoder
    decoder: Decoder,
    /// Configuration
    config: FleetConfig,
    /// Statistics
    stats: FleetStats,
}

/// State for a single emitter
#[derive(Debug)]
pub struct EmitterState {
    /// Emitter's context
    pub context: Context,
    /// Last seen timestamp
    pub last_seen: u64,
    /// Message count from this emitter
    pub message_count: u64,
    /// Recent values (for cross-fleet comparison)
    pub recent_values: Vec<f64>,
    /// Is this emitter behaving anomalously?
    pub is_anomalous: bool,
}

impl EmitterState {
    pub fn new() -> Self {
        Self {
            context: Context::new(),
            last_seen: 0,
            message_count: 0,
            recent_values: Vec::with_capacity(100),
            is_anomalous: false,
        }
    }
    
    pub fn record_value(&mut self, value: f64, timestamp: u64) {
        self.last_seen = timestamp;
        self.message_count += 1;
        
        // Keep last 100 values
        if self.recent_values.len() >= 100 {
            self.recent_values.remove(0);
        }
        self.recent_values.push(value);
    }
    
    /// Calculate mean of recent values
    pub fn mean(&self) -> Option<f64> {
        if self.recent_values.is_empty() {
            return None;
        }
        Some(self.recent_values.iter().sum::<f64>() / self.recent_values.len() as f64)
    }
    
    /// Calculate std deviation of recent values
    pub fn std_dev(&self) -> Option<f64> {
        let mean = self.mean()?;
        if self.recent_values.len() < 2 {
            return None;
        }
        
        let variance = self.recent_values.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / (self.recent_values.len() - 1) as f64;
        
        Some(variance.sqrt())
    }
}

impl Default for EmitterState {
    fn default() -> Self {
        Self::new()
    }
}

/// Fleet configuration
#[derive(Debug, Clone)]
pub struct FleetConfig {
    /// Maximum emitters to track
    pub max_emitters: usize,
    /// Timeout before considering emitter offline (in seconds)
    pub emitter_timeout: u64,
    /// Threshold for cross-fleet anomaly (std deviations)
    pub cross_fleet_threshold: f64,
    /// Minimum emitters for cross-fleet analysis
    pub min_emitters_for_comparison: usize,
    /// How often to promote patterns to fleet context
    pub fleet_sync_interval: u64,
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            max_emitters: 1000,
            emitter_timeout: 300,
            cross_fleet_threshold: 3.0,
            min_emitters_for_comparison: 3,
            fleet_sync_interval: 1000,
        }
    }
}

impl FleetManager {
    /// Create a new fleet manager
    pub fn new() -> Self {
        Self {
            emitter_contexts: HashMap::new(),
            fleet_context: Context::new(),
            classifier: Classifier::default(),
            decoder: Decoder::new(),
            config: FleetConfig::default(),
            stats: FleetStats::default(),
        }
    }
    
    /// Create with custom configuration
    pub fn with_config(config: FleetConfig) -> Self {
        Self {
            config,
            ..Self::new()
        }
    }
    
    /// Process a message from an emitter
    pub fn process_message(
        &mut self,
        emitter_id: EmitterId,
        message: &crate::protocol::EncodedMessage,
        timestamp: u64,
    ) -> Result<ProcessedMessage> {
        // Get or create emitter state
        let emitter = self.emitter_contexts
            .entry(emitter_id)
            .or_insert_with(EmitterState::new);
        
        // Decode message
        let decoded = self.decoder.decode(message, &emitter.context)?;
        
        // Update emitter state
        emitter.record_value(decoded.value, timestamp);
        emitter.context.observe(&RawData::new(decoded.value, timestamp));
        
        // Update stats
        self.stats.total_messages += 1;
        *self.stats.priority_distribution
            .entry(decoded.priority)
            .or_insert(0) += 1;
        
        // Check for cross-fleet anomaly
        let cross_fleet_anomaly = self.check_cross_fleet_anomaly(emitter_id, decoded.value);
        if cross_fleet_anomaly {
            self.stats.cross_fleet_anomalies += 1;
            emitter.is_anomalous = true;
        }
        
        // Check for regular anomaly
        if decoded.priority == Priority::P1Critical || decoded.priority == Priority::P2Important {
            self.stats.anomaly_count += 1;
        }
        
        Ok(ProcessedMessage {
            emitter_id,
            value: decoded.value,
            priority: decoded.priority,
            is_cross_fleet_anomaly: cross_fleet_anomaly,
        })
    }
    
    /// Check if this value is anomalous compared to fleet
    fn check_cross_fleet_anomaly(&self, emitter_id: EmitterId, value: f64) -> bool {
        if self.emitter_contexts.len() < self.config.min_emitters_for_comparison {
            return false;
        }
        
        // Calculate fleet-wide mean and std dev
        let other_means: Vec<f64> = self.emitter_contexts.iter()
            .filter(|(id, _)| **id != emitter_id)
            .filter_map(|(_, state)| state.mean())
            .collect();
        
        if other_means.len() < self.config.min_emitters_for_comparison - 1 {
            return false;
        }
        
        let fleet_mean = other_means.iter().sum::<f64>() / other_means.len() as f64;
        let fleet_variance = other_means.iter()
            .map(|m| (m - fleet_mean).powi(2))
            .sum::<f64>() / other_means.len() as f64;
        let fleet_std = fleet_variance.sqrt();
        
        if fleet_std < 0.001 {
            return false;
        }
        
        // Check if this value is outside threshold
        let z_score = (value - fleet_mean).abs() / fleet_std;
        z_score > self.config.cross_fleet_threshold
    }
    
    /// Get list of active emitters
    pub fn active_emitters(&self, current_time: u64) -> Vec<EmitterId> {
        self.emitter_contexts.iter()
            .filter(|(_, state)| {
                current_time - state.last_seen < self.config.emitter_timeout
            })
            .map(|(id, _)| *id)
            .collect()
    }
    
    /// Get emitter state
    pub fn get_emitter(&self, id: EmitterId) -> Option<&EmitterState> {
        self.emitter_contexts.get(&id)
    }
    
    /// Get fleet statistics
    pub fn stats(&self) -> &FleetStats {
        &self.stats
    }
    
    /// Get fleet-wide context
    pub fn fleet_context(&self) -> &Context {
        &self.fleet_context
    }
    
    /// Promote common patterns to fleet context
    pub fn sync_fleet_patterns(&mut self) {
        // Find patterns that appear in multiple emitters
        let mut pattern_counts: HashMap<u64, u32> = HashMap::new();
        
        for (_, state) in &self.emitter_contexts {
            for pattern_hash in state.context.pattern_hashes() {
                *pattern_counts.entry(pattern_hash).or_insert(0) += 1;
            }
        }
        
        // Promote patterns found in >50% of emitters
        let threshold = self.emitter_contexts.len() / 2;
        for (hash, count) in pattern_counts {
            if count as usize > threshold {
                // Find and promote this pattern
                // (simplified - would need to actually retrieve pattern)
            }
        }
    }
    
    /// Remove stale emitters
    pub fn cleanup_stale_emitters(&mut self, current_time: u64) {
        self.emitter_contexts.retain(|_, state| {
            current_time - state.last_seen < self.config.emitter_timeout * 2
        });
        self.stats.emitter_count = self.emitter_contexts.len();
    }
}

impl Default for FleetManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of processing a message
#[derive(Debug, Clone)]
pub struct ProcessedMessage {
    pub emitter_id: EmitterId,
    pub value: f64,
    pub priority: Priority,
    pub is_cross_fleet_anomaly: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fleet_manager_creation() {
        let fleet = FleetManager::new();
        assert_eq!(fleet.stats().emitter_count, 0);
    }
    
    #[test]
    fn test_emitter_state_mean() {
        let mut state = EmitterState::new();
        state.record_value(10.0, 0);
        state.record_value(20.0, 1);
        state.record_value(30.0, 2);
        
        assert_eq!(state.mean(), Some(20.0));
    }
    
    #[test]
    fn test_cross_fleet_anomaly() {
        let mut fleet = FleetManager::with_config(FleetConfig {
            min_emitters_for_comparison: 2,
            cross_fleet_threshold: 2.0,
            ..Default::default()
        });
        
        // Add normal emitters
        for i in 0..5 {
            let mut state = EmitterState::new();
            for j in 0..10 {
                state.record_value(20.0 + (j as f64 * 0.1), j);
            }
            fleet.emitter_contexts.insert(i, state);
        }
        
        // Check anomalous value
        let is_anomaly = fleet.check_cross_fleet_anomaly(99, 100.0);
        assert!(is_anomaly);
        
        // Check normal value
        let is_normal = fleet.check_cross_fleet_anomaly(99, 20.5);
        assert!(!is_normal);
    }
}
```

### 2. Créer un exemple fleet

`examples/fleet_demo.rs` :

```rust
use alec::fleet::{FleetManager, EmitterId};
use alec::{Encoder, Classifier, Context, RawData};

fn main() {
    let mut fleet = FleetManager::new();
    
    // Simulate 10 emitters
    let num_emitters = 10;
    let mut encoders: Vec<_> = (0..num_emitters)
        .map(|_| (Encoder::new(), Context::new()))
        .collect();
    
    let classifier = Classifier::default();
    
    println!("Simulating fleet of {} emitters...\n", num_emitters);
    
    // Simulate 1000 messages
    for t in 0..1000u64 {
        // Each emitter sends data
        for (emitter_id, (encoder, context)) in encoders.iter_mut().enumerate() {
            // Normal temperature with slight variation per emitter
            let base_temp = 20.0 + (emitter_id as f64 * 0.5);
            let temp = base_temp + (t as f64 * 0.01).sin();
            
            // Inject anomaly for emitter 5
            let temp = if emitter_id == 5 && t > 500 {
                temp + 20.0  // Sudden spike
            } else {
                temp
            };
            
            let data = RawData::new(temp, t);
            let classification = classifier.classify(&data, context);
            let message = encoder.encode(&data, &classification, context);
            
            let result = fleet.process_message(
                emitter_id as EmitterId,
                &message,
                t,
            );
            
            if let Ok(processed) = result {
                if processed.is_cross_fleet_anomaly {
                    println!("⚠️  Cross-fleet anomaly: Emitter {} at t={}", 
                        emitter_id, t);
                }
            }
            
            context.observe(&data);
        }
    }
    
    // Print stats
    let stats = fleet.stats();
    println!("\n=== Fleet Statistics ===");
    println!("Total messages: {}", stats.total_messages);
    println!("Anomalies detected: {}", stats.anomaly_count);
    println!("Cross-fleet anomalies: {}", stats.cross_fleet_anomalies);
}
```

## Livrables

- [ ] `src/fleet.rs` — Module fleet complet
- [ ] `FleetManager` avec gestion multi-émetteurs
- [ ] `EmitterState` avec statistiques
- [ ] Détection cross-fleet anomaly
- [ ] `examples/fleet_demo.rs`
- [ ] Tests (au moins 5)

## Critères de succès

```bash
cargo test fleet
cargo run --example fleet_demo
```

Le fleet manager doit :
- Gérer N émetteurs simultanément
- Détecter les anomalies cross-fleet
- Maintenir des stats par émetteur

## Prochaine étape

→ `07_securite.md` (v1.0.0 - Sécurité)
