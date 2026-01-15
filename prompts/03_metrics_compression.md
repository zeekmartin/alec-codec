# Prompt 03 — Métriques de Compression

## Contexte

Pour évaluer l'efficacité d'ALEC, nous avons besoin de métriques permettant de mesurer :
- Le ratio de compression obtenu
- L'efficacité des prédictions
- La distribution des types d'encodage utilisés

## Objectif

Créer un module `metrics` qui collecte et expose des statistiques sur :
1. Taille des données avant/après compression
2. Types d'encodage utilisés
3. Qualité des prédictions
4. Performance du contexte

## Étapes

### 1. Créer `src/metrics.rs`

```rust
//! Metrics collection for ALEC compression analysis
//!
//! This module provides statistics about compression efficiency,
//! encoding distribution, and prediction accuracy.

use std::collections::HashMap;
use crate::protocol::EncodingType;

/// Compression statistics collector
#[derive(Debug, Clone, Default)]
pub struct CompressionMetrics {
    /// Total raw bytes (before compression)
    pub raw_bytes: u64,
    /// Total encoded bytes (after compression)
    pub encoded_bytes: u64,
    /// Number of messages processed
    pub message_count: u64,
    /// Encoding type distribution
    pub encoding_distribution: HashMap<EncodingType, u64>,
    /// Prediction hits (value matched prediction)
    pub prediction_hits: u64,
    /// Prediction misses
    pub prediction_misses: u64,
}

impl CompressionMetrics {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Record an encoding operation
    pub fn record_encode(&mut self, raw_size: usize, encoded_size: usize, encoding: EncodingType) {
        self.raw_bytes += raw_size as u64;
        self.encoded_bytes += encoded_size as u64;
        self.message_count += 1;
        *self.encoding_distribution.entry(encoding).or_insert(0) += 1;
    }
    
    /// Record a prediction result
    pub fn record_prediction(&mut self, hit: bool) {
        if hit {
            self.prediction_hits += 1;
        } else {
            self.prediction_misses += 1;
        }
    }
    
    /// Calculate compression ratio (higher = better)
    /// Returns raw_size / encoded_size
    pub fn compression_ratio(&self) -> f64 {
        if self.encoded_bytes == 0 {
            return 1.0;
        }
        self.raw_bytes as f64 / self.encoded_bytes as f64
    }
    
    /// Calculate space savings percentage
    /// Returns (1 - encoded/raw) * 100
    pub fn space_savings_percent(&self) -> f64 {
        if self.raw_bytes == 0 {
            return 0.0;
        }
        (1.0 - (self.encoded_bytes as f64 / self.raw_bytes as f64)) * 100.0
    }
    
    /// Calculate prediction accuracy (0.0 - 1.0)
    pub fn prediction_accuracy(&self) -> f64 {
        let total = self.prediction_hits + self.prediction_misses;
        if total == 0 {
            return 0.0;
        }
        self.prediction_hits as f64 / total as f64
    }
    
    /// Get most used encoding type
    pub fn most_used_encoding(&self) -> Option<EncodingType> {
        self.encoding_distribution
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(encoding, _)| *encoding)
    }
    
    /// Reset all metrics
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    
    /// Generate a human-readable report
    pub fn report(&self) -> String {
        let mut report = String::new();
        
        report.push_str("=== ALEC Compression Metrics ===\n\n");
        
        report.push_str(&format!("Messages processed: {}\n", self.message_count));
        report.push_str(&format!("Raw bytes: {} bytes\n", self.raw_bytes));
        report.push_str(&format!("Encoded bytes: {} bytes\n", self.encoded_bytes));
        report.push_str(&format!("Compression ratio: {:.2}x\n", self.compression_ratio()));
        report.push_str(&format!("Space savings: {:.1}%\n\n", self.space_savings_percent()));
        
        report.push_str("Encoding distribution:\n");
        for (encoding, count) in &self.encoding_distribution {
            let percent = (*count as f64 / self.message_count as f64) * 100.0;
            report.push_str(&format!("  {:?}: {} ({:.1}%)\n", encoding, count, percent));
        }
        
        report.push_str(&format!("\nPrediction accuracy: {:.1}%\n", 
            self.prediction_accuracy() * 100.0));
        
        report
    }
}

/// Context statistics
#[derive(Debug, Clone, Default)]
pub struct ContextMetrics {
    /// Number of patterns in dictionary
    pub pattern_count: usize,
    /// Total memory used by context (estimated)
    pub memory_bytes: usize,
    /// Number of sources tracked
    pub source_count: usize,
    /// Context version
    pub version: u32,
}

impl ContextMetrics {
    pub fn from_context(context: &crate::context::Context) -> Self {
        Self {
            pattern_count: context.pattern_count(),
            memory_bytes: context.estimated_memory(),
            source_count: context.source_count(),
            version: context.version(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_ratio() {
        let mut metrics = CompressionMetrics::new();
        metrics.record_encode(100, 25, EncodingType::Delta8);
        
        assert!((metrics.compression_ratio() - 4.0).abs() < 0.01);
        assert!((metrics.space_savings_percent() - 75.0).abs() < 0.1);
    }
    
    #[test]
    fn test_prediction_accuracy() {
        let mut metrics = CompressionMetrics::new();
        metrics.record_prediction(true);
        metrics.record_prediction(true);
        metrics.record_prediction(false);
        
        assert!((metrics.prediction_accuracy() - 0.666).abs() < 0.01);
    }
    
    #[test]
    fn test_encoding_distribution() {
        let mut metrics = CompressionMetrics::new();
        metrics.record_encode(100, 10, EncodingType::Delta8);
        metrics.record_encode(100, 10, EncodingType::Delta8);
        metrics.record_encode(100, 1, EncodingType::Repeated);
        
        assert_eq!(metrics.most_used_encoding(), Some(EncodingType::Delta8));
    }
    
    #[test]
    fn test_report_generation() {
        let mut metrics = CompressionMetrics::new();
        metrics.record_encode(1000, 250, EncodingType::Delta8);
        
        let report = metrics.report();
        assert!(report.contains("Compression ratio"));
        assert!(report.contains("Delta8"));
    }
}
```

### 2. Ajouter au module principal

Dans `src/lib.rs` :

```rust
pub mod metrics;
pub use metrics::{CompressionMetrics, ContextMetrics};
```

### 3. Intégrer dans l'encodeur (optionnel)

```rust
impl Encoder {
    /// Encode with metrics collection
    pub fn encode_with_metrics(
        &mut self,
        data: &RawData,
        classification: &Classification,
        context: &Context,
        metrics: &mut CompressionMetrics,
    ) -> EncodedMessage {
        let message = self.encode(data, classification, context);
        
        if let Some(encoding) = message.encoding_type() {
            metrics.record_encode(
                data.raw_size(),
                message.len(),
                encoding,
            );
        }
        
        message
    }
}
```

### 4. Ajouter méthodes au Context

Dans `src/context.rs`, ajouter si manquant :

```rust
impl Context {
    /// Number of patterns in dictionary
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
    
    /// Number of tracked sources
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }
    
    /// Estimated memory usage in bytes
    pub fn estimated_memory(&self) -> usize {
        // Rough estimate
        self.patterns.len() * 64 + self.sources.len() * 32
    }
}
```

### 5. Créer un exemple avec métriques

`examples/metrics_demo.rs` :

```rust
use alec::{Classifier, Context, Encoder, CompressionMetrics, RawData};

fn main() {
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();
    let mut metrics = CompressionMetrics::new();
    
    // Simuler 1000 mesures
    println!("Simulating 1000 sensor readings...\n");
    
    for i in 0..1000 {
        // Température avec légère variation
        let temp = 20.0 + (i as f64 * 0.01).sin() * 2.0;
        let data = RawData::new(temp, i as u64);
        
        let classification = classifier.classify(&data, &context);
        let message = encoder.encode_with_metrics(
            &data, &classification, &context, &mut metrics
        );
        
        context.observe(&data);
    }
    
    println!("{}", metrics.report());
}
```

## Livrables

- [ ] `src/metrics.rs` — Module complet
- [ ] `src/lib.rs` — Export du module
- [ ] `src/context.rs` — Méthodes helper
- [ ] `examples/metrics_demo.rs` — Exemple
- [ ] Tests (au moins 4)

## Critères de succès

```bash
cargo test metrics  # Tests du module
cargo run --example metrics_demo  # Affiche un rapport
```

Output attendu :
```
=== ALEC Compression Metrics ===

Messages processed: 1000
Raw bytes: 24000 bytes
Encoded bytes: ~5000 bytes
Compression ratio: ~4.8x
Space savings: ~79%
...
```

## Prochaine étape

→ `04_contexte_evolutif.md` (v0.2.0)
