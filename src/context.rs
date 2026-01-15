// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Shared context module
//!
//! This module manages the shared context between emitter and receiver:
//! - Dictionary of patterns for compression
//! - Predictive model for delta encoding
//! - Synchronization mechanisms

use crate::error::{ContextError, Result};
use crate::protocol::RawData;
use std::collections::HashMap;
use xxhash_rust::xxh64::xxh64;

/// Maximum number of patterns in dictionary
pub const MAX_PATTERNS: usize = 65535;

/// Maximum pattern size in bytes
pub const MAX_PATTERN_SIZE: usize = 255;

/// Default memory limit for context (64 KB)
pub const DEFAULT_MEMORY_LIMIT: usize = 64 * 1024;

/// A prediction for a source
#[derive(Debug, Clone, PartialEq)]
pub struct Prediction {
    /// Predicted value
    pub value: f64,
    /// Confidence in prediction (0.0-1.0)
    pub confidence: f32,
    /// Type of model used
    pub model_type: PredictionModel,
}

/// Type of prediction model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PredictionModel {
    /// No model, using last value
    #[default]
    LastValue,
    /// Simple moving average
    MovingAverage,
    /// Linear regression
    LinearRegression,
    /// Periodic pattern detected
    Periodic,
}

/// Statistics for a single source with EMA prediction
#[derive(Debug, Clone)]
struct SourceStats {
    /// Last observed value
    last_value: f64,
    /// Exponential moving average
    ema: f64,
    /// EMA alpha (smoothing factor, 0-1)
    ema_alpha: f64,
    /// Number of observations
    count: u64,
    /// Sum of squared differences (for variance)
    sum_sq_diff: f64,
    /// Running mean
    mean: f64,
    /// History buffer for advanced predictions
    history: Vec<f64>,
    /// Maximum history size
    max_history: usize,
}

impl SourceStats {
    fn new(max_history: usize, ema_alpha: f64) -> Self {
        Self {
            last_value: 0.0,
            ema: 0.0,
            ema_alpha,
            count: 0,
            sum_sq_diff: 0.0,
            mean: 0.0,
            history: Vec::with_capacity(max_history),
            max_history,
        }
    }

    fn observe(&mut self, value: f64) {
        self.count += 1;
        self.last_value = value;

        // Update EMA
        if self.count == 1 {
            self.ema = value;
        } else {
            self.ema = self.ema_alpha * value + (1.0 - self.ema_alpha) * self.ema;
        }

        // Update running statistics (Welford's algorithm)
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.sum_sq_diff += delta * delta2;

        // Update history
        if self.history.len() >= self.max_history {
            self.history.remove(0);
        }
        self.history.push(value);
    }

    fn predict(&self) -> Option<Prediction> {
        if self.count == 0 {
            return None;
        }

        // Calculate variance for confidence
        let variance = if self.count > 1 {
            self.sum_sq_diff / (self.count - 1) as f64
        } else {
            0.0
        };

        // Lower variance = higher confidence
        let confidence = if variance < 0.001 {
            0.95
        } else if variance < 0.01 {
            0.85
        } else if variance < 0.1 {
            0.70
        } else {
            0.50
        };

        // Use EMA for prediction after enough observations
        let (predicted_value, model_type) = if self.count < 3 {
            (self.last_value, PredictionModel::LastValue)
        } else {
            (self.ema, PredictionModel::MovingAverage)
        };

        Some(Prediction {
            value: predicted_value,
            confidence: confidence as f32,
            model_type,
        })
    }

    fn moving_average(&self, window: usize) -> Option<f64> {
        if self.history.is_empty() {
            return None;
        }
        let window = window.min(self.history.len());
        let start = self.history.len() - window;
        let sum: f64 = self.history[start..].iter().sum();
        Some(sum / window as f64)
    }
}

/// A pattern in the dictionary with usage statistics
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    /// Raw bytes of the pattern
    pub data: Vec<u8>,
    /// Associated value (if numeric pattern)
    pub value: Option<f64>,
    /// Usage frequency counter
    pub frequency: u64,
    /// Last time this pattern was used (observation count)
    pub last_used: u64,
    /// When the pattern was created (observation count)
    pub created_at: u64,
}

impl Pattern {
    /// Create a new pattern
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            value: None,
            frequency: 1,
            last_used: 0,
            created_at: 0,
        }
    }

    /// Create a new pattern with timestamp
    pub fn with_timestamp(data: Vec<u8>, timestamp: u64) -> Self {
        Self {
            data,
            value: None,
            frequency: 1,
            last_used: timestamp,
            created_at: timestamp,
        }
    }

    /// Create a numeric pattern
    pub fn numeric(value: f64) -> Self {
        Self {
            data: value.to_be_bytes().to_vec(),
            value: Some(value),
            frequency: 1,
            last_used: 0,
            created_at: 0,
        }
    }

    /// Create a numeric pattern with timestamp
    pub fn numeric_with_timestamp(value: f64, timestamp: u64) -> Self {
        Self {
            data: value.to_be_bytes().to_vec(),
            value: Some(value),
            frequency: 1,
            last_used: timestamp,
            created_at: timestamp,
        }
    }

    /// Update usage statistics
    pub fn touch(&mut self, timestamp: u64) {
        self.frequency = self.frequency.saturating_add(1);
        self.last_used = timestamp;
    }

    /// Calculate a score for this pattern (higher = more valuable)
    /// Score combines frequency and recency
    pub fn score(&self, current_time: u64) -> f64 {
        let age = current_time.saturating_sub(self.last_used) as f64;
        let recency = 1.0 / (1.0 + age / 1000.0);
        let freq_score = (self.frequency as f64 + 1.0).ln();
        freq_score * recency
    }
}

/// Configuration for context evolution
#[derive(Debug, Clone)]
pub struct EvolutionConfig {
    /// Minimum frequency to keep a pattern during pruning
    pub min_frequency: u64,
    /// Maximum age (in observations) before pruning
    pub max_age: u64,
    /// How often to run evolution (every N observations)
    pub evolution_interval: u64,
    /// Threshold for promotion (frequency)
    pub promotion_threshold: u64,
    /// Whether evolution is enabled
    pub enabled: bool,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            min_frequency: 2,
            max_age: 10000,
            evolution_interval: 100,
            promotion_threshold: 10,
            enabled: true,
        }
    }
}

/// Configuration for the context
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Maximum patterns in dictionary
    pub max_patterns: usize,
    /// Maximum memory usage in bytes
    pub max_memory: usize,
    /// History size per source for predictions
    pub history_size: usize,
    /// EMA alpha (smoothing factor for predictions)
    pub ema_alpha: f64,
    /// Evolution configuration
    pub evolution: EvolutionConfig,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_patterns: MAX_PATTERNS,
            max_memory: DEFAULT_MEMORY_LIMIT,
            history_size: 100,
            ema_alpha: 0.3,
            evolution: EvolutionConfig::default(),
        }
    }
}

/// The shared context between emitter and receiver
#[derive(Debug, Clone)]
pub struct Context {
    /// Current version number
    version: u32,
    /// Total observation count (used for timestamps)
    observation_count: u64,
    /// Dictionary: code -> pattern
    dictionary: HashMap<u32, Pattern>,
    /// Reverse lookup: pattern hash -> code
    pattern_index: HashMap<u64, u32>,
    /// Next available code
    next_code: u32,
    /// Per-source statistics for prediction
    source_stats: HashMap<u32, SourceStats>,
    /// Configuration
    config: ContextConfig,
    /// Scale factor for delta encoding
    scale_factor: u32,
}

impl Context {
    /// Create a new empty context
    pub fn new() -> Self {
        Self {
            version: 0,
            observation_count: 0,
            dictionary: HashMap::new(),
            pattern_index: HashMap::new(),
            next_code: 0,
            source_stats: HashMap::new(),
            config: ContextConfig::default(),
            scale_factor: crate::DEFAULT_SCALE_FACTOR,
        }
    }

    /// Create context with custom configuration
    pub fn with_config(config: ContextConfig) -> Self {
        Self {
            version: 0,
            observation_count: 0,
            dictionary: HashMap::new(),
            pattern_index: HashMap::new(),
            next_code: 0,
            source_stats: HashMap::new(),
            config,
            scale_factor: crate::DEFAULT_SCALE_FACTOR,
        }
    }

    /// Create context with evolution configuration
    pub fn with_evolution(evolution_config: EvolutionConfig) -> Self {
        let config = ContextConfig {
            evolution: evolution_config,
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Get observation count
    pub fn observation_count(&self) -> u64 {
        self.observation_count
    }

    /// Get current version
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Get scale factor
    pub fn scale_factor(&self) -> u32 {
        self.scale_factor
    }

    /// Calculate hash of the entire context for sync verification
    pub fn hash(&self) -> u64 {
        let mut data = Vec::new();

        // Sort codes for deterministic hash
        let mut codes: Vec<_> = self.dictionary.keys().collect();
        codes.sort();

        for code in codes {
            if let Some(pattern) = self.dictionary.get(code) {
                data.extend_from_slice(&code.to_be_bytes());
                data.extend_from_slice(&(pattern.data.len() as u16).to_be_bytes());
                data.extend_from_slice(&pattern.data);
            }
        }

        xxh64(&data, 0)
    }

    /// Get number of patterns in dictionary
    pub fn pattern_count(&self) -> usize {
        self.dictionary.len()
    }

    /// Get number of tracked sources
    pub fn source_count(&self) -> usize {
        self.source_stats.len()
    }

    /// Estimate memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        let dict_size: usize = self
            .dictionary
            .values()
            .map(|p| p.data.len() + 32) // pattern data + overhead
            .sum();
        let stats_size = self.source_stats.len() * 200; // approximate
        dict_size + stats_size + 256 // base overhead
    }

    /// Alias for memory_usage (for metrics compatibility)
    pub fn estimated_memory(&self) -> usize {
        self.memory_usage()
    }

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

    /// Prune patterns that are old or rarely used
    fn prune_patterns(&mut self, current_time: u64) {
        let config = &self.config.evolution;
        let min_freq = config.min_frequency;
        let max_age = config.max_age;

        // Collect patterns to remove
        let to_remove: Vec<u32> = self
            .dictionary
            .iter()
            .filter(|(_, pattern)| {
                let age = current_time.saturating_sub(pattern.last_used);
                pattern.frequency < min_freq || age > max_age
            })
            .map(|(code, _)| *code)
            .collect();

        // Remove patterns and their index entries
        for code in to_remove {
            if let Some(pattern) = self.dictionary.remove(&code) {
                let hash = xxh64(&pattern.data, 0);
                self.pattern_index.remove(&hash);
            }
        }
    }

    /// Reorder patterns by score (best patterns get lowest IDs)
    fn reorder_patterns(&mut self, current_time: u64) {
        if self.dictionary.is_empty() {
            return;
        }

        // Collect and sort by score (descending)
        let mut entries: Vec<_> = self.dictionary.drain().collect();
        entries.sort_by(|a, b| {
            b.1.score(current_time)
                .partial_cmp(&a.1.score(current_time))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Clear pattern index
        self.pattern_index.clear();

        // Reassign IDs (best patterns get lowest IDs)
        self.next_code = 0;
        for (_, pattern) in entries {
            let new_id = self.next_code;
            let hash = xxh64(&pattern.data, 0);
            self.pattern_index.insert(hash, new_id);
            self.dictionary.insert(new_id, pattern);
            self.next_code += 1;
        }
    }

    /// Observe a new data point (update statistics and trigger evolution)
    pub fn observe(&mut self, data: &RawData) {
        self.observation_count += 1;

        // Update source statistics
        let ema_alpha = self.config.ema_alpha;
        let history_size = self.config.history_size;
        let stats = self
            .source_stats
            .entry(data.source_id)
            .or_insert_with(|| SourceStats::new(history_size, ema_alpha));

        stats.observe(data.value);
        self.version += 1;

        // Check if evolution is needed
        let evolution = &self.config.evolution;
        if evolution.enabled
            && evolution.evolution_interval > 0
            && self.observation_count % evolution.evolution_interval == 0
        {
            self.evolve();
        }
    }

    /// Get prediction for a source
    pub fn predict(&self, source_id: u32) -> Option<Prediction> {
        self.source_stats.get(&source_id)?.predict()
    }

    /// Get last observed value for a source
    pub fn last_value(&self, source_id: u32) -> Option<f64> {
        self.source_stats.get(&source_id).map(|s| s.last_value)
    }

    /// Get moving average for a source
    pub fn moving_average(&self, source_id: u32, window: usize) -> Option<f64> {
        self.source_stats.get(&source_id)?.moving_average(window)
    }

    /// Register a new pattern in the dictionary
    pub fn register_pattern(&mut self, pattern: Pattern) -> Result<u32> {
        // Check limits
        if self.dictionary.len() >= self.config.max_patterns {
            return Err(ContextError::DictionaryFull {
                max: self.config.max_patterns,
            }
            .into());
        }

        if pattern.data.len() > MAX_PATTERN_SIZE {
            return Err(ContextError::PatternTooLarge {
                size: pattern.data.len(),
                max: MAX_PATTERN_SIZE,
            }
            .into());
        }

        // Check if pattern already exists
        let pattern_hash = xxh64(&pattern.data, 0);
        if let Some(&existing_code) = self.pattern_index.get(&pattern_hash) {
            // Increment frequency
            if let Some(p) = self.dictionary.get_mut(&existing_code) {
                p.frequency += 1;
            }
            return Ok(existing_code);
        }

        // Add new pattern
        let code = self.next_code;
        self.next_code += 1;
        self.pattern_index.insert(pattern_hash, code);
        self.dictionary.insert(code, pattern);
        self.version += 1;

        Ok(code)
    }

    /// Get pattern by code
    pub fn get_pattern(&self, code: u32) -> Option<&Pattern> {
        self.dictionary.get(&code)
    }

    /// Find pattern code by data
    pub fn find_pattern(&self, data: &[u8]) -> Option<u32> {
        let hash = xxh64(data, 0);
        self.pattern_index.get(&hash).copied()
    }

    // === Synchronization helper methods ===

    /// Remove a pattern by ID
    pub fn remove_pattern(&mut self, id: u32) {
        if let Some(pattern) = self.dictionary.remove(&id) {
            let hash = xxh64(&pattern.data, 0);
            self.pattern_index.remove(&hash);
        }
    }

    /// Set a pattern at a specific ID (for sync)
    pub fn set_pattern(&mut self, id: u32, pattern: Pattern) {
        let hash = xxh64(&pattern.data, 0);
        self.pattern_index.insert(hash, id);
        self.dictionary.insert(id, pattern);
        if id >= self.next_code {
            self.next_code = id + 1;
        }
    }

    /// Check if a pattern exists by ID
    pub fn has_pattern(&self, id: u32) -> bool {
        self.dictionary.contains_key(&id)
    }

    /// Iterate over patterns (id, pattern)
    pub fn patterns_iter(&self) -> impl Iterator<Item = (&u32, &Pattern)> {
        self.dictionary.iter()
    }

    /// Get all pattern IDs
    pub fn pattern_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.dictionary.keys().copied()
    }

    /// Set version directly (for sync)
    pub fn set_version(&mut self, version: u32) {
        self.version = version;
    }

    /// Get iterator over pattern hashes (for fleet sync)
    pub fn pattern_hashes(&self) -> impl Iterator<Item = u64> + '_ {
        self.pattern_index.keys().copied()
    }

    /// Export full context for synchronization
    pub fn export_full(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Version
        data.extend_from_slice(&self.version.to_be_bytes());

        // Hash
        data.extend_from_slice(&self.hash().to_be_bytes());

        // Pattern count
        data.extend_from_slice(&(self.dictionary.len() as u16).to_be_bytes());

        // Patterns
        for (&code, pattern) in &self.dictionary {
            data.extend_from_slice(&code.to_be_bytes());
            data.push(pattern.data.len() as u8);
            data.extend_from_slice(&pattern.data);
        }

        data
    }

    /// Export diff since a given version
    pub fn export_diff(&self, _from_version: u32) -> Vec<u8> {
        // Simplified: export full for now
        // In real implementation, track changes per version
        self.export_full()
    }

    /// Import full context
    pub fn import_full(&mut self, data: &[u8]) -> Result<()> {
        if data.len() < 14 {
            return Err(ContextError::SyncFailed {
                reason: "Data too short".to_string(),
            }
            .into());
        }

        let version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let hash = u64::from_be_bytes([
            data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
        ]);
        let count = u16::from_be_bytes([data[12], data[13]]) as usize;

        // Clear current dictionary
        self.dictionary.clear();
        self.pattern_index.clear();
        self.next_code = 0;

        // Read patterns
        let mut offset = 14;
        for _ in 0..count {
            if offset + 5 > data.len() {
                return Err(ContextError::SyncFailed {
                    reason: "Truncated data".to_string(),
                }
                .into());
            }

            let code = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let len = data[offset + 4] as usize;
            offset += 5;

            if offset + len > data.len() {
                return Err(ContextError::SyncFailed {
                    reason: "Truncated pattern data".to_string(),
                }
                .into());
            }

            let pattern_data = data[offset..offset + len].to_vec();
            offset += len;

            let pattern_hash = xxh64(&pattern_data, 0);
            self.dictionary.insert(code, Pattern::new(pattern_data));
            self.pattern_index.insert(pattern_hash, code);

            if code >= self.next_code {
                self.next_code = code + 1;
            }
        }

        self.version = version;

        // Verify hash
        let computed_hash = self.hash();
        if computed_hash != hash {
            return Err(ContextError::HashMismatch {
                expected: hash,
                actual: computed_hash,
            }
            .into());
        }

        Ok(())
    }

    /// Reset context to initial state
    pub fn reset(&mut self) {
        self.dictionary.clear();
        self.pattern_index.clear();
        self.source_stats.clear();
        self.next_code = 0;
        self.version = 0;
        self.observation_count = 0;
    }

    /// Verify hash matches
    pub fn verify(&self, expected_hash: u64) -> bool {
        self.hash() == expected_hash
    }

    /// Get the type of prediction model being used
    pub fn model_type(&self) -> PredictionModel {
        // Return most common model type across sources
        if self.source_stats.is_empty() {
            PredictionModel::LastValue
        } else {
            // Simplified: return LastValue
            // In real impl, could track which model performs best
            PredictionModel::LastValue
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

// HealthCheckable implementation for Context
impl crate::health::HealthCheckable for Context {
    fn health_check(&self) -> crate::health::HealthCheck {
        use crate::health::{HealthCheck, HealthStatus};
        use std::time::Instant;

        let start = Instant::now();

        // Check memory usage
        let memory = self.estimated_memory();
        let pattern_count = self.pattern_count();

        // Determine health status based on memory and pattern count
        let status = if memory > 100_000_000 {
            // Over 100MB is unhealthy
            HealthStatus::Unhealthy
        } else if memory > 10_000_000 || pattern_count > 50_000 {
            // Over 10MB or 50K patterns is degraded
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        HealthCheck {
            component: "Context".to_string(),
            status,
            last_check: Instant::now(),
            message: format!(
                "Memory: {} bytes, Patterns: {}, Sources: {}",
                memory,
                pattern_count,
                self.source_count()
            ),
            latency: start.elapsed(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_new() {
        let ctx = Context::new();
        assert_eq!(ctx.version(), 0);
        assert_eq!(ctx.pattern_count(), 0);
    }

    #[test]
    fn test_observe_and_predict() {
        let mut ctx = Context::new();

        // No prediction initially
        assert!(ctx.predict(0).is_none());

        // Observe some values (20.0, 20.1, 20.2, ... 20.9)
        for i in 0..10 {
            ctx.observe(&RawData::new(20.0 + i as f64 * 0.1, i as u64));
        }

        // Should have prediction now (using EMA)
        let pred = ctx.predict(0).unwrap();
        // EMA with alpha=0.3 will be between first and last value
        // It should be somewhere in the range [20.0, 21.0]
        assert!(pred.value > 20.0 && pred.value < 21.0);
        assert!(pred.confidence > 0.0);
    }

    #[test]
    fn test_register_pattern() {
        let mut ctx = Context::new();

        let pattern = Pattern::new(vec![1, 2, 3, 4]);
        let code = ctx.register_pattern(pattern.clone()).unwrap();

        assert_eq!(ctx.pattern_count(), 1);
        assert!(ctx.get_pattern(code).is_some());

        // Registering same pattern returns same code
        let code2 = ctx.register_pattern(pattern).unwrap();
        assert_eq!(code, code2);
        assert_eq!(ctx.pattern_count(), 1); // Still just 1 pattern
    }

    #[test]
    fn test_context_hash() {
        let mut ctx1 = Context::new();
        let mut ctx2 = Context::new();

        // Same patterns should have same hash
        ctx1.register_pattern(Pattern::new(vec![1, 2, 3])).unwrap();
        ctx2.register_pattern(Pattern::new(vec![1, 2, 3])).unwrap();

        assert_eq!(ctx1.hash(), ctx2.hash());

        // Different patterns should have different hash
        ctx1.register_pattern(Pattern::new(vec![4, 5, 6])).unwrap();
        assert_ne!(ctx1.hash(), ctx2.hash());
    }

    #[test]
    fn test_export_import() {
        let mut ctx1 = Context::new();
        ctx1.register_pattern(Pattern::new(vec![1, 2, 3])).unwrap();
        ctx1.register_pattern(Pattern::new(vec![4, 5, 6])).unwrap();

        let exported = ctx1.export_full();

        let mut ctx2 = Context::new();
        ctx2.import_full(&exported).unwrap();

        assert_eq!(ctx1.hash(), ctx2.hash());
        assert_eq!(ctx1.pattern_count(), ctx2.pattern_count());
    }

    #[test]
    fn test_context_reset() {
        let mut ctx = Context::new();
        ctx.observe(&RawData::new(42.0, 0));
        ctx.register_pattern(Pattern::new(vec![1, 2, 3])).unwrap();

        assert!(ctx.version() > 0);
        assert!(ctx.pattern_count() > 0);

        ctx.reset();

        assert_eq!(ctx.version(), 0);
        assert_eq!(ctx.pattern_count(), 0);
        assert!(ctx.predict(0).is_none());
    }

    #[test]
    fn test_last_value() {
        let mut ctx = Context::new();

        assert!(ctx.last_value(0).is_none());

        ctx.observe(&RawData::new(42.5, 0));
        assert_eq!(ctx.last_value(0), Some(42.5));

        ctx.observe(&RawData::new(43.0, 1));
        assert_eq!(ctx.last_value(0), Some(43.0));
    }

    // === Evolution Tests ===

    #[test]
    fn test_pattern_pruning() {
        let mut config = ContextConfig::default();
        config.evolution = EvolutionConfig {
            min_frequency: 3,
            max_age: 50,
            evolution_interval: 10,
            promotion_threshold: 5,
            enabled: false, // Manual control
        };

        let mut ctx = Context::with_config(config);

        // Add pattern used only once
        let mut pattern = Pattern::new(vec![42]);
        pattern.frequency = 1;
        pattern.last_used = 0;
        ctx.register_pattern(pattern).unwrap();

        assert_eq!(ctx.pattern_count(), 1);

        // Simulate time passing
        ctx.observation_count = 100;
        ctx.evolve();

        // Pattern should be pruned (frequency < 3 or age > 50)
        assert_eq!(ctx.pattern_count(), 0);
    }

    #[test]
    fn test_pattern_kept_if_frequent() {
        let mut config = ContextConfig::default();
        config.evolution = EvolutionConfig {
            min_frequency: 2,
            max_age: 1000,
            evolution_interval: 10,
            promotion_threshold: 5,
            enabled: false,
        };

        let mut ctx = Context::with_config(config);

        // Add frequently used pattern
        let mut pattern = Pattern::new(vec![42]);
        pattern.frequency = 10;
        pattern.last_used = 50;
        ctx.register_pattern(pattern).unwrap();

        // Update the pattern's stats
        if let Some(p) = ctx.dictionary.get_mut(&0) {
            p.frequency = 10;
            p.last_used = 50;
        }

        ctx.observation_count = 100;
        ctx.evolve();

        // Pattern should be kept (frequency >= 2 and age <= 1000)
        assert_eq!(ctx.pattern_count(), 1);
    }

    #[test]
    fn test_pattern_reordering() {
        let mut config = ContextConfig::default();
        config.evolution.enabled = false;

        let mut ctx = Context::with_config(config);

        // Add two patterns with different frequencies
        ctx.register_pattern(Pattern::with_timestamp(vec![1], 0))
            .unwrap();
        ctx.register_pattern(Pattern::with_timestamp(vec![2], 0))
            .unwrap();

        // Make second pattern more frequent
        if let Some(p) = ctx.dictionary.get_mut(&1) {
            p.frequency = 100;
            p.last_used = 10;
        }
        if let Some(p) = ctx.dictionary.get_mut(&0) {
            p.frequency = 1;
            p.last_used = 0;
        }

        ctx.observation_count = 10;
        ctx.evolve();

        // More frequent pattern should now have ID 0
        let pattern_0 = ctx.get_pattern(0).unwrap();
        assert_eq!(pattern_0.frequency, 100);
    }

    #[test]
    fn test_ema_prediction() {
        let mut ctx = Context::new();

        // Observe trending values (increasing)
        for i in 0..20 {
            ctx.observe(&RawData::new(20.0 + i as f64, i as u64));
        }

        // EMA should predict around recent values
        let prediction = ctx.predict(0).unwrap();

        // With alpha=0.3, EMA should be between last value (39) and mean
        // EMA gives more weight to recent values
        assert!(prediction.value > 30.0 && prediction.value < 40.0);
        assert_eq!(prediction.model_type, PredictionModel::MovingAverage);
    }

    #[test]
    fn test_evolution_triggered_automatically() {
        let mut config = ContextConfig::default();
        config.evolution = EvolutionConfig {
            min_frequency: 1,
            max_age: 10000,
            evolution_interval: 5, // Evolve every 5 observations
            promotion_threshold: 5,
            enabled: true,
        };

        let mut ctx = Context::with_config(config);

        // Add a pattern
        ctx.register_pattern(Pattern::new(vec![1, 2, 3])).unwrap();

        let initial_version = ctx.version();

        // Make 5 observations to trigger evolution
        for i in 0..5 {
            ctx.observe(&RawData::new(20.0, i as u64));
        }

        // Version should have increased from observations + evolution
        assert!(ctx.version() > initial_version);
        assert_eq!(ctx.observation_count(), 5);
    }

    #[test]
    fn test_pattern_score() {
        let mut pattern = Pattern::new(vec![1, 2, 3]);
        pattern.frequency = 100;
        pattern.last_used = 900;

        // Score at time 1000 (age = 100)
        let score_recent = pattern.score(1000);

        // Score at time 2000 (age = 1100)
        let score_old = pattern.score(2000);

        // More recent should have higher score
        assert!(score_recent > score_old);
    }
}
