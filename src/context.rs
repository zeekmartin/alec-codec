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

/// Statistics for a single source
#[derive(Debug, Clone)]
struct SourceStats {
    /// Last observed value
    last_value: f64,
    /// Sum of recent values (for moving average)
    sum: f64,
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
    fn new(max_history: usize) -> Self {
        Self {
            last_value: 0.0,
            sum: 0.0,
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

        // Update running statistics (Welford's algorithm)
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.sum_sq_diff += delta * delta2;

        self.sum += value;

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

        // Simple prediction: use last value with confidence based on variance
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

        let model_type = if self.count < 5 {
            PredictionModel::LastValue
        } else if variance < 0.01 {
            PredictionModel::MovingAverage
        } else {
            PredictionModel::LastValue
        };

        Some(Prediction {
            value: self.last_value,
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

/// A pattern in the dictionary
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    /// Raw bytes of the pattern
    pub data: Vec<u8>,
    /// Associated value (if numeric pattern)
    pub value: Option<f64>,
    /// Usage count
    pub frequency: u64,
}

impl Pattern {
    /// Create a new pattern
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            value: None,
            frequency: 0,
        }
    }

    /// Create a numeric pattern
    pub fn numeric(value: f64) -> Self {
        Self {
            data: value.to_be_bytes().to_vec(),
            value: Some(value),
            frequency: 0,
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
    /// Minimum frequency before pattern promotion
    pub promotion_threshold: u64,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_patterns: MAX_PATTERNS,
            max_memory: DEFAULT_MEMORY_LIMIT,
            history_size: 100,
            promotion_threshold: 10,
        }
    }
}

/// The shared context between emitter and receiver
#[derive(Debug, Clone)]
pub struct Context {
    /// Current version number
    version: u32,
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
            dictionary: HashMap::new(),
            pattern_index: HashMap::new(),
            next_code: 0,
            source_stats: HashMap::new(),
            config,
            scale_factor: crate::DEFAULT_SCALE_FACTOR,
        }
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

    /// Observe a new data point (update statistics)
    pub fn observe(&mut self, data: &RawData) {
        let stats = self
            .source_stats
            .entry(data.source_id)
            .or_insert_with(|| SourceStats::new(self.config.history_size));

        stats.observe(data.value);
        self.version += 1;
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

        // Observe some values
        for i in 0..10 {
            ctx.observe(&RawData::new(20.0 + i as f64 * 0.1, i as u64));
        }

        // Should have prediction now
        let pred = ctx.predict(0).unwrap();
        assert!((pred.value - 20.9).abs() < 0.1);
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
}
