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
//! - Preload file support for instant optimal compression

mod preload;

pub use preload::*;

#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use crate::error::{ContextError, Result};
use crate::protocol::RawData;
use xxhash_rust::xxh64::xxh64;

#[cfg(feature = "std")]
type Map<K, V> = std::collections::HashMap<K, V>;
#[cfg(not(feature = "std"))]
type Map<K, V> = alloc::collections::BTreeMap<K, V>;

/// Walk `map` in ascending-`u32`-key order, invoking `f(key, value)`
/// for each entry. Zero heap allocations on both the `std` and
/// `no_std` code paths.
///
/// * `no_std`: `Map` is a `BTreeMap`; `.iter()` is already sorted, so
///   we forward directly.
/// * `std`: `Map` is a `HashMap` with random iteration order. We run
///   an O(n²) "find next-smallest key strictly greater than the last"
///   sweep — no heap, tiny constants. For the sizes we care about
///   (typical `≤ 32` source_stats, `≤ 256` dictionary patterns) this
///   is orders of magnitude faster than a call into the stdlib sort
///   machinery and, unlike `Vec<u32> + sort()`, leaves a strict
///   tracking allocator at zero bytes.
#[inline]
fn for_each_sorted_u32<V, F: FnMut(u32, &V)>(map: &Map<u32, V>, mut f: F) {
    #[cfg(not(feature = "std"))]
    {
        for (k, v) in map {
            f(*k, v);
        }
    }
    #[cfg(feature = "std")]
    {
        let n = map.len();
        if n == 0 {
            return;
        }
        let mut last: Option<u32> = None;
        let mut emitted = 0usize;
        while emitted < n {
            let mut next: Option<u32> = None;
            for &k in map.keys() {
                if let Some(l) = last {
                    if k <= l {
                        continue;
                    }
                }
                match next {
                    None => next = Some(k),
                    Some(cur) if k < cur => next = Some(k),
                    _ => {}
                }
            }
            match next {
                Some(k) => {
                    f(k, map.get(&k).unwrap());
                    last = Some(k);
                    emitted += 1;
                }
                None => break, // Defensive: unreachable if map.len() matches real entries.
            }
        }
    }
}

/// Byte-streaming helper: append one SourceStats entry to `out` at
/// cursor `w`. Zero heap, no intermediate buffers.
///
/// Used by `Context::write_preload_bytes` — the v1.3.9 zero-heap
/// serialiser. Factored out to keep the parent function's stack frame
/// small and avoid nine copy-paste expansions of the field layout.
fn write_source_stats_into(out: &mut [u8], w: &mut usize, sid: u32, s: &SourceStats) {
    out[*w..*w + 4].copy_from_slice(&sid.to_le_bytes());
    *w += 4;
    out[*w..*w + 8].copy_from_slice(&s.count.to_le_bytes());
    *w += 8;
    out[*w..*w + 8].copy_from_slice(&s.last_value.to_le_bytes());
    *w += 8;
    out[*w..*w + 8].copy_from_slice(&s.ema.to_le_bytes());
    *w += 8;
    out[*w..*w + 8].copy_from_slice(&s.ema_alpha.to_le_bytes());
    *w += 8;
    out[*w..*w + 8].copy_from_slice(&s.sum_sq_diff.to_le_bytes());
    *w += 8;
    out[*w..*w + 8].copy_from_slice(&s.mean.to_le_bytes());
    *w += 8;
    out[*w..*w + 4].copy_from_slice(&(s.max_history as u32).to_le_bytes());
    *w += 4;
    out[*w..*w + 4].copy_from_slice(&(s.history.len() as u32).to_le_bytes());
    *w += 4;
    for v in &s.history {
        out[*w..*w + 8].copy_from_slice(&v.to_le_bytes());
        *w += 8;
    }
}

/// Byte-streaming helper: append one dictionary `Pattern` entry to
/// `out` at cursor `w`. Companion to `write_source_stats_into`.
fn write_pattern_into(out: &mut [u8], w: &mut usize, code: u32, p: &Pattern) {
    out[*w..*w + 4].copy_from_slice(&code.to_le_bytes());
    *w += 4;
    // Pattern data length capped at u16 (MAX_PATTERN_SIZE = 255 anyway).
    let data_len = p.data.len().min(u16::MAX as usize);
    out[*w..*w + 2].copy_from_slice(&(data_len as u16).to_le_bytes());
    *w += 2;
    out[*w..*w + data_len].copy_from_slice(&p.data[..data_len]);
    *w += data_len;
    out[*w..*w + 8].copy_from_slice(&p.frequency.to_le_bytes());
    *w += 8;
    out[*w..*w + 8].copy_from_slice(&p.last_used.to_le_bytes());
    *w += 8;
    out[*w..*w + 8].copy_from_slice(&p.created_at.to_le_bytes());
    *w += 8;
}

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
        // Use a simple log approximation for no_std compatibility
        // ln(x) ≈ (x - 1) / (x + 1) * 2 for x > 0 (rough but sufficient for scoring)
        let x = self.frequency as f64 + 1.0;
        #[cfg(feature = "std")]
        let freq_score = x.ln();
        #[cfg(not(feature = "std"))]
        let freq_score = {
            // Approximate ln using integer bit counting: ln(x) ≈ log2(x) * ln(2)
            let bits = (63 - (x as u64).leading_zeros()) as f64;
            bits * core::f64::consts::LN_2
        };
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
    dictionary: Map<u32, Pattern>,
    /// Reverse lookup: pattern hash -> code
    pattern_index: Map<u64, u32>,
    /// Next available code
    next_code: u32,
    /// Per-source statistics for prediction
    source_stats: Map<u32, SourceStats>,
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
            dictionary: Map::new(),
            pattern_index: Map::new(),
            next_code: 0,
            source_stats: Map::new(),
            config: ContextConfig::default(),
            scale_factor: crate::DEFAULT_SCALE_FACTOR,
        }
    }

    /// Create context with custom configuration
    pub fn with_config(config: ContextConfig) -> Self {
        Self {
            version: 0,
            observation_count: 0,
            dictionary: Map::new(),
            pattern_index: Map::new(),
            next_code: 0,
            source_stats: Map::new(),
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
        let keys: Vec<_> = self.dictionary.keys().copied().collect();
        let mut entries: Vec<_> = keys
            .into_iter()
            .filter_map(|k| self.dictionary.remove(&k).map(|v| (k, v)))
            .collect();
        entries.sort_by(|a, b| {
            b.1.score(current_time)
                .partial_cmp(&a.1.score(current_time))
                .unwrap_or(core::cmp::Ordering::Equal)
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

    /// Reset only the runtime-learned prediction state, preserving any
    /// patterns loaded from a preload file.
    ///
    /// Used by the packet-loss recovery path of the Milesight fixed-
    /// channel codec (Bloc C): when the decoder detects a sequence
    /// gap or a context-version mismatch on a non-keyframe frame, it
    /// calls this so that the stale per-channel EMA / last-value
    /// state is cleared. The next frame the decoder (and encoder, on
    /// `alec_force_keyframe`) sees must be a keyframe — Raw32 for
    /// every channel — which will re-seed the prediction state for
    /// all channels.
    ///
    /// What this clears:
    /// - `source_stats`: the per-channel EMA, last_value, history
    ///   and variance state. This is the core of the recovery — the
    ///   decoder must not apply stale predictions to new Delta8 /
    ///   Delta16 bytes after a gap.
    ///
    /// What this preserves:
    /// - `dictionary` and `pattern_index`: any preloaded patterns
    ///   survive the reset. We do NOT distinguish preloaded from
    ///   learned patterns (no tagging infrastructure exists), so in
    ///   practice we keep all patterns. For the Milesight fixed-
    ///   channel wire format this is moot because that path never
    ///   uses Pattern encoding (only Repeated / Delta8 / Delta16 /
    ///   Raw32), so the dictionary stays untouched in normal use.
    /// - `version`: keeping the counter lets the decoder continue to
    ///   detect future context-version mismatches against peers.
    /// - `observation_count`: preserved for metrics continuity.
    /// - `scale_factor`, `config`, `next_code`: preserved.
    ///
    /// # Invariant
    ///
    /// `dictionary` and `pattern_index` stay in sync after the call
    /// (neither is touched). This is important so that a subsequent
    /// `register_pattern()` does not create a duplicate entry.
    pub fn reset_to_baseline(&mut self) {
        self.source_stats.clear();
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

    // === Preload File Support ===

    /// Save current context state to a preload file
    ///
    /// This allows the context to be loaded later for instant optimal compression.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to save the preload file
    /// * `sensor_type` - Identifier for the sensor type (e.g., "temperature", "humidity")
    ///
    /// # Example
    ///
    /// ```no_run
    /// use alec::context::Context;
    /// use std::path::Path;
    ///
    /// let mut ctx = Context::new();
    /// // ... train the context with data ...
    /// ctx.save_to_file(Path::new("temperature.alec-context"), "temperature").unwrap();
    /// ```
    #[cfg(feature = "std")]
    pub fn save_to_file(&self, path: &std::path::Path, sensor_type: &str) -> Result<()> {
        let preload = PreloadFile::from_context(self, sensor_type);
        preload.save_to_file(path)
    }

    /// Load a preload file and initialize context
    ///
    /// This allows achieving optimal compression from the first byte
    /// by loading a pre-trained context.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the preload file
    ///
    /// # Returns
    ///
    /// A new Context initialized with the preload data
    ///
    /// # Example
    ///
    /// ```no_run
    /// use alec::context::Context;
    /// use std::path::Path;
    ///
    /// let ctx = Context::load_from_file(Path::new("temperature.alec-context")).unwrap();
    /// assert!(ctx.pattern_count() > 0);
    /// ```
    #[cfg(feature = "std")]
    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        let preload = PreloadFile::load_from_file(path)?;
        Self::from_preload(&preload)
    }

    /// Create a context from a preload file
    #[cfg(feature = "std")]
    fn from_preload(preload: &PreloadFile) -> Result<Self> {
        let mut ctx = Self::new();

        // Restore version
        ctx.version = preload.context_version;

        // Restore dictionary
        for entry in &preload.dictionary {
            let pattern = Pattern {
                data: entry.pattern.clone(),
                value: None,
                frequency: entry.frequency as u64,
                last_used: 0,
                created_at: 0,
            };
            let code = entry.code as u32;
            let hash = xxh64(&pattern.data, 0);
            ctx.pattern_index.insert(hash, code);
            ctx.dictionary.insert(code, pattern);
            if code >= ctx.next_code {
                ctx.next_code = code + 1;
            }
        }

        Ok(ctx)
    }

    /// Get context version for sync checking
    ///
    /// This version should be included in message headers to allow
    /// the decoder to verify it has the correct context.
    pub fn context_version(&self) -> u32 {
        self.version
    }

    /// Check if context version matches expected version
    ///
    /// Returns a `VersionCheckResult` indicating whether versions match
    /// or providing details about the mismatch.
    pub fn check_version(&self, message_version: u32) -> VersionCheckResult {
        if self.version == message_version {
            VersionCheckResult::Match
        } else {
            VersionCheckResult::Mismatch {
                expected: self.version,
                actual: message_version,
            }
        }
    }

    // ========================================================================
    // Bloc D — In-memory context-state persistence
    //
    // `to_preload_bytes` / `from_preload_bytes` serialize a Context to and
    // from a self-contained byte buffer, WITHOUT any filesystem I/O — so
    // they work in `no_std + alloc` builds and are the primary mechanism
    // the Milesight ChirpStack sidecar uses to persist per-DevEUI decoder
    // state across restarts (e.g. via Redis).
    //
    // Wire format (ALCS = "ALec Context State"):
    //
    //     magic      [4]  b"ALCS"
    //     version    [4]  u32 LE format version (currently 1)
    //     ctx_ver    [4]  u32 LE Context::version() (full u32, not u16-truncated)
    //     scale      [4]  u32 LE Context::scale_factor()
    //     obs_count  [8]  u64 LE Context::observation_count()
    //     next_code  [4]  u32 LE dictionary's next code assignment
    //     sens_len   [1]  u8   sensor_type length (0..=255)
    //     sensor     [N]  UTF-8 sensor-type identifier (e.g. "em500-co2")
    //     src_count  [4]  u32 LE number of per-source SourceStats entries
    //     for each source (sorted by source_id):
    //         source_id   [4] u32 LE
    //         count       [8] u64 LE observations count for this source
    //         last_value  [8] f64 LE
    //         ema         [8] f64 LE
    //         ema_alpha   [8] f64 LE
    //         sum_sq_diff [8] f64 LE
    //         mean        [8] f64 LE
    //         max_history [4] u32 LE
    //         hist_len    [4] u32 LE
    //         history     [hist_len × 8] f64 LE
    //     dict_count [4]  u32 LE number of patterns in the dictionary
    //     for each pattern (sorted by code):
    //         code       [4] u32 LE
    //         data_len   [2] u16 LE (max 255, per MAX_PATTERN_SIZE)
    //         data       [data_len] bytes
    //         frequency  [8] u64 LE
    //         last_used  [8] u64 LE
    //         created_at [8] u64 LE
    //     checksum   [4]  CRC32 (CRC_32_ISO_HDLC) over the whole buffer
    //                     up to this point, written last
    //
    // Distinction from the older `PreloadFile` format (magic b"ALEC"):
    // the ALEC format was designed for **training preloads** (one
    // PreloadStatistics aggregate + dictionary + prediction-model
    // metadata); it cannot represent per-source SourceStats state and
    // therefore cannot round-trip a running decoder. ALCS is
    // specifically for runtime state persistence and preserves every
    // field the encoder / decoder rely on.
    // ========================================================================

    /// Serialize this context to a self-contained byte buffer.
    ///
    /// Intended for per-DevEUI sidecar state persistence (Redis etc.).
    /// Pure in-memory; works in `no_std + alloc`.
    ///
    /// # Arguments
    ///
    /// * `sensor_type` - Human-readable device-model identifier (≤ 255 bytes).
    ///
    /// # Returns
    ///
    /// Serialized bytes, typically ~1-2 KB per context.
    ///
    /// # Errors
    ///
    /// * `ContextError::PatternTooLarge` if `sensor_type` is longer than 255 bytes.
    pub fn to_preload_bytes(&self, sensor_type: &str) -> Result<Vec<u8>> {
        // Backward-compat wrapper: allocate a right-sized Vec and
        // stream directly into it. Zero-heap callers should prefer
        // `write_preload_bytes` which writes to a caller-provided
        // slice.
        let needed = self.preload_bytes_len(sensor_type)?;
        // `vec![0; needed]` compiles to a single `__rust_alloc_zeroed`
        // call — faster than `with_capacity + resize` which triggers
        // a loop-based zero-fill (clippy::slow_vector_initialization).
        #[cfg(feature = "std")]
        let mut out: Vec<u8> = vec![0u8; needed];
        #[cfg(not(feature = "std"))]
        let mut out: Vec<u8> = alloc::vec![0u8; needed];
        let written = self.write_preload_bytes(sensor_type, &mut out)?;
        debug_assert_eq!(written, needed);
        out.truncate(written);
        Ok(out)
    }

    /// Number of bytes `write_preload_bytes(sensor_type, …)` / `to_preload_bytes`
    /// would produce for the current context, without allocating anything.
    ///
    /// Intended for callers that need to size a pre-allocated buffer
    /// before calling `write_preload_bytes` (e.g. MCU firmware with a
    /// static scratch arena).
    ///
    /// # Errors
    ///
    /// * `ContextError::PatternTooLarge` if `sensor_type` is longer than 255 bytes.
    pub fn preload_bytes_len(&self, sensor_type: &str) -> Result<usize> {
        let sens_len = sensor_type.len();
        if sens_len > 255 {
            return Err(crate::error::ContextError::PatternTooLarge {
                size: sens_len,
                max: 255,
            }
            .into());
        }
        // Fixed header: 4 (magic) + 4 (format ver) + 4 (ctx ver)
        //             + 4 (scale) + 8 (obs count) + 4 (next_code)
        //             + 1 (sens_len) + sens_len + 4 (src_count).
        let mut total = 4 + 4 + 4 + 4 + 8 + 4 + 1 + sens_len + 4;
        for s in self.source_stats.values() {
            // Fixed: sid(4) + count(8) + last_value(8) + ema(8)
            //      + ema_alpha(8) + sum_sq_diff(8) + mean(8)
            //      + max_history(4) + hist_len(4) = 60 B
            // Plus: hist_len * 8.
            total += 60 + s.history.len() * 8;
        }
        total += 4; // dict_count
        for p in self.dictionary.values() {
            let data_len = p.data.len().min(u16::MAX as usize);
            // Fixed: code(4) + data_len(2) + data + frequency(8)
            //      + last_used(8) + created_at(8) = 30 B + data.
            total += 30 + data_len;
        }
        total += 4; // trailing CRC32
        Ok(total)
    }

    /// Serialize the context into a caller-provided byte buffer.
    ///
    /// **Zero heap allocations.** This is the MCU-friendly serialiser —
    /// used by `alec_encoder_context_save` so that firmware can save
    /// its state into a static 2 KB buffer without asking the allocator
    /// for a ~1.5 KB scratch block (which frequently fails on 4 KB-heap
    /// Cortex-M targets once patterns have accumulated).
    ///
    /// The produced bytes are **byte-identical** to `to_preload_bytes` —
    /// same ALCS format, same CRC32, same ordering. The two functions
    /// are interchangeable from the reader's perspective.
    ///
    /// # Arguments
    ///
    /// * `sensor_type` — label written into the ALCS header (≤ 255 bytes).
    /// * `out` — destination buffer. Must be at least `preload_bytes_len(sensor_type)`
    ///   bytes long.
    ///
    /// # Returns
    ///
    /// Number of bytes written on success.
    ///
    /// # Errors
    ///
    /// * `ContextError::PatternTooLarge` if `sensor_type` is > 255 bytes.
    /// * `EncodeError::BufferTooSmall { needed, available }` if `out`
    ///   is too small. The buffer is NOT modified in that case
    ///   (no partial write) — callers can safely retry after resizing.
    pub fn write_preload_bytes(&self, sensor_type: &str, out: &mut [u8]) -> Result<usize> {
        let sens_bytes = sensor_type.as_bytes();
        if sens_bytes.len() > 255 {
            return Err(crate::error::ContextError::PatternTooLarge {
                size: sens_bytes.len(),
                max: 255,
            }
            .into());
        }

        // Pre-flight size check so the common BufferTooSmall path does
        // not leave a partial write in `out`.
        let needed = self.preload_bytes_len(sensor_type)?;
        if out.len() < needed {
            return Err(crate::error::EncodeError::BufferTooSmall {
                needed,
                available: out.len(),
            }
            .into());
        }

        let mut w = 0usize;
        // Magic + format version.
        out[w..w + 4].copy_from_slice(ALCS_MAGIC);
        w += 4;
        out[w..w + 4].copy_from_slice(&ALCS_FORMAT_VERSION.to_le_bytes());
        w += 4;
        // Core context scalars.
        out[w..w + 4].copy_from_slice(&self.version.to_le_bytes());
        w += 4;
        out[w..w + 4].copy_from_slice(&self.scale_factor.to_le_bytes());
        w += 4;
        out[w..w + 8].copy_from_slice(&self.observation_count.to_le_bytes());
        w += 8;
        out[w..w + 4].copy_from_slice(&self.next_code.to_le_bytes());
        w += 4;
        // sensor_type.
        out[w] = sens_bytes.len() as u8;
        w += 1;
        out[w..w + sens_bytes.len()].copy_from_slice(sens_bytes);
        w += sens_bytes.len();

        // === Per-source SourceStats ===
        out[w..w + 4].copy_from_slice(&(self.source_stats.len() as u32).to_le_bytes());
        w += 4;
        // Iterate in ascending source_id order with *zero* heap.
        //
        // * `no_std` path: `Map` is a `BTreeMap`, `.iter()` is already
        //   sorted — a direct `for` loop suffices.
        // * `std` path: `Map` is a `HashMap` (random iteration order).
        //   We use an O(n²) "find the next-smallest key > last" sweep
        //   via `for_each_sorted_u32`, which walks the map multiple
        //   times without allocating anywhere. `n` is bounded by
        //   `max_patterns` (≤ 65 535) and is typically < 32 for
        //   fixed-channel encoders, so the extra compute is trivial.
        //
        // This replaces the v1.3.8 `sorted_u32_keys(…) -> Vec<u32>`
        // helper, which allocated a small `Vec<u32>` and was the last
        // remaining heap allocation visible to a strict tracking
        // allocator (verified by `zero_heap_allocator_proof.rs`).
        for_each_sorted_u32(&self.source_stats, |sid, s| {
            write_source_stats_into(out, &mut w, sid, s);
        });

        // === Dictionary ===
        out[w..w + 4].copy_from_slice(&(self.dictionary.len() as u32).to_le_bytes());
        w += 4;
        for_each_sorted_u32(&self.dictionary, |code, p| {
            write_pattern_into(out, &mut w, code, p);
        });

        // === Trailing CRC32 ===
        // The `crc` crate's Crc<u32> holds a 1 KB lookup table; the
        // `const` hoists it into rodata so no stack copy is made on
        // ARM release builds (verified in v1.3.8 stack audit).
        use crc::{Crc, CRC_32_ISO_HDLC};
        const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);
        let crc = CRC32.checksum(&out[..w]);
        out[w..w + 4].copy_from_slice(&crc.to_le_bytes());
        w += 4;

        debug_assert_eq!(w, needed);
        Ok(w)
    }

    /// Reconstruct a context from bytes produced by `to_preload_bytes`.
    ///
    /// # Errors
    ///
    /// * `DecodeError::BufferTooShort` / `InvalidHeader` / `MalformedMessage`
    ///   for structural problems.
    /// * `DecodeError::InvalidChecksum` if the CRC32 does not match.
    pub fn from_preload_bytes(data: &[u8]) -> Result<Self> {
        use crc::{Crc, CRC_32_ISO_HDLC};
        const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

        // Minimum header (magic 4 + fmt 4 + ver 4 + scale 4 + obs 8 +
        // next 4 + sens_len 1 + sens 0 + src_count 4 + dict_count 4 +
        // crc 4) = 41 bytes.
        if data.len() < 41 {
            return Err(crate::error::DecodeError::BufferTooShort {
                needed: 41,
                available: data.len(),
            }
            .into());
        }
        if &data[..4] != ALCS_MAGIC {
            return Err(crate::error::DecodeError::InvalidHeader.into());
        }

        // CRC32 is the LAST 4 bytes of the buffer.
        let crc_offset = data.len() - 4;
        let stored_crc = u32::from_le_bytes(data[crc_offset..].try_into().unwrap());
        let computed_crc = CRC32.checksum(&data[..crc_offset]);
        if stored_crc != computed_crc {
            return Err(crate::error::DecodeError::InvalidChecksum {
                expected: stored_crc,
                actual: computed_crc,
            }
            .into());
        }

        let format_version = u32::from_le_bytes(data[4..8].try_into().unwrap());
        if format_version != ALCS_FORMAT_VERSION {
            return Err(crate::error::DecodeError::MalformedMessage {
                offset: 4,
                reason: {
                    #[cfg(feature = "std")]
                    {
                        format!(
                            "unsupported ALCS format version {} (expected {})",
                            format_version, ALCS_FORMAT_VERSION
                        )
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        "unsupported ALCS format version".to_string()
                    }
                },
            }
            .into());
        }

        let version = u32::from_le_bytes(data[8..12].try_into().unwrap());
        let scale_factor = u32::from_le_bytes(data[12..16].try_into().unwrap());
        let observation_count = u64::from_le_bytes(data[16..24].try_into().unwrap());
        let next_code = u32::from_le_bytes(data[24..28].try_into().unwrap());
        let sens_len = data[28] as usize;

        let mut offset: usize = 29;
        if offset + sens_len > crc_offset {
            return Err(crate::error::DecodeError::BufferTooShort {
                needed: offset + sens_len + 4,
                available: data.len(),
            }
            .into());
        }
        // We read but discard sensor_type — it's metadata for operators,
        // not part of the runtime state. The FFI layer may surface it
        // separately in a future revision.
        offset += sens_len;

        // === SourceStats ===
        if offset + 4 > crc_offset {
            return Err(crate::error::DecodeError::BufferTooShort {
                needed: offset + 4,
                available: data.len(),
            }
            .into());
        }
        let src_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        let mut source_stats: Map<u32, SourceStats> = Map::new();
        for _ in 0..src_count {
            // Fixed part: 4 + 8 + 5*8 + 4 + 4 = 56 bytes.
            if offset + 56 > crc_offset {
                return Err(crate::error::DecodeError::BufferTooShort {
                    needed: offset + 56,
                    available: data.len(),
                }
                .into());
            }
            let source_id = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
            let count = u64::from_le_bytes(data[offset + 4..offset + 12].try_into().unwrap());
            let last_value = f64::from_le_bytes(data[offset + 12..offset + 20].try_into().unwrap());
            let ema = f64::from_le_bytes(data[offset + 20..offset + 28].try_into().unwrap());
            let ema_alpha = f64::from_le_bytes(data[offset + 28..offset + 36].try_into().unwrap());
            let sum_sq_diff =
                f64::from_le_bytes(data[offset + 36..offset + 44].try_into().unwrap());
            let mean = f64::from_le_bytes(data[offset + 44..offset + 52].try_into().unwrap());
            let max_history =
                u32::from_le_bytes(data[offset + 52..offset + 56].try_into().unwrap()) as usize;
            offset += 56;

            if offset + 4 > crc_offset {
                return Err(crate::error::DecodeError::BufferTooShort {
                    needed: offset + 4,
                    available: data.len(),
                }
                .into());
            }
            let hist_len =
                u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
            offset += 4;

            let hist_bytes = hist_len.saturating_mul(8);
            if offset + hist_bytes > crc_offset {
                return Err(crate::error::DecodeError::BufferTooShort {
                    needed: offset + hist_bytes,
                    available: data.len(),
                }
                .into());
            }
            let mut history: Vec<f64> = Vec::with_capacity(hist_len);
            for i in 0..hist_len {
                let hv = f64::from_le_bytes(
                    data[offset + i * 8..offset + i * 8 + 8].try_into().unwrap(),
                );
                history.push(hv);
            }
            offset += hist_bytes;

            source_stats.insert(
                source_id,
                SourceStats {
                    last_value,
                    ema,
                    ema_alpha,
                    count,
                    sum_sq_diff,
                    mean,
                    history,
                    max_history,
                },
            );
        }

        // === Dictionary ===
        if offset + 4 > crc_offset {
            return Err(crate::error::DecodeError::BufferTooShort {
                needed: offset + 4,
                available: data.len(),
            }
            .into());
        }
        let dict_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        let mut dictionary: Map<u32, Pattern> = Map::new();
        let mut pattern_index: Map<u64, u32> = Map::new();
        for _ in 0..dict_count {
            // Fixed pre-data part: 4 + 2 = 6 bytes.
            if offset + 6 > crc_offset {
                return Err(crate::error::DecodeError::BufferTooShort {
                    needed: offset + 6,
                    available: data.len(),
                }
                .into());
            }
            let code = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
            let data_len =
                u16::from_le_bytes(data[offset + 4..offset + 6].try_into().unwrap()) as usize;
            offset += 6;

            // Fixed post-data part: 24 bytes (frequency 8 + last_used 8 + created_at 8).
            if offset + data_len + 24 > crc_offset {
                return Err(crate::error::DecodeError::BufferTooShort {
                    needed: offset + data_len + 24,
                    available: data.len(),
                }
                .into());
            }
            let pattern_bytes = data[offset..offset + data_len].to_vec();
            offset += data_len;

            let frequency = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
            let last_used = u64::from_le_bytes(data[offset + 8..offset + 16].try_into().unwrap());
            let created_at = u64::from_le_bytes(data[offset + 16..offset + 24].try_into().unwrap());
            offset += 24;

            let hash = xxh64(&pattern_bytes, 0);
            pattern_index.insert(hash, code);
            dictionary.insert(
                code,
                Pattern {
                    data: pattern_bytes,
                    value: None,
                    frequency,
                    last_used,
                    created_at,
                },
            );
        }

        // If `offset` != crc_offset here, the buffer has trailing bytes
        // between the end of the declared content and the CRC. That
        // shouldn't happen in a file we produced, so flag it.
        if offset != crc_offset {
            return Err(crate::error::DecodeError::MalformedMessage {
                offset,
                reason: {
                    #[cfg(feature = "std")]
                    {
                        format!(
                            "trailing {} byte(s) between content and CRC",
                            crc_offset - offset
                        )
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        "trailing bytes between content and CRC".to_string()
                    }
                },
            }
            .into());
        }

        Ok(Self {
            version,
            observation_count,
            dictionary,
            pattern_index,
            next_code,
            source_stats,
            config: ContextConfig::default(),
            scale_factor,
        })
    }
}

/// Magic bytes for the ALCS (ALec Context State) serialization format
/// produced by `Context::to_preload_bytes`. Distinct from the older
/// `PreloadFile` magic (`b"ALEC"`) so callers can safely auto-detect.
pub const ALCS_MAGIC: &[u8; 4] = b"ALCS";

/// Current ALCS format version. Increment on any wire-level change.
pub const ALCS_FORMAT_VERSION: u32 = 1;

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

// HealthCheckable implementation for Context
#[cfg(feature = "std")]
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

    /// Bloc C-1: `reset_to_baseline` must wipe prediction state but
    /// leave preloaded patterns (and the dictionary in general)
    /// intact, so the decoder can still decode existing Pattern
    /// references while starting fresh for Delta-encoded frames.
    #[test]
    fn test_reset_to_baseline_wipes_predictions_preserves_patterns() {
        let mut ctx = Context::new();

        // Build up prediction state for two "channels".
        for _ in 0..5 {
            ctx.observe(&RawData::with_source(1, 22.5, 0));
            ctx.observe(&RawData::with_source(2, 1013.25, 0));
        }
        assert!(ctx.predict(1).is_some());
        assert!(ctx.predict(2).is_some());
        assert_eq!(ctx.last_value(1), Some(22.5));
        assert_eq!(ctx.last_value(2), Some(1013.25));

        // Simulate a preloaded pattern.
        let code = ctx
            .register_pattern(Pattern::new(vec![0xDE, 0xAD, 0xBE, 0xEF]))
            .unwrap();
        let pre_pattern_count = ctx.pattern_count();
        let pre_version = ctx.version();
        assert!(pre_pattern_count >= 1);

        // Reset.
        ctx.reset_to_baseline();

        // Prediction state is wiped for all channels.
        assert!(ctx.predict(1).is_none());
        assert!(ctx.predict(2).is_none());
        assert_eq!(ctx.last_value(1), None);
        assert_eq!(ctx.last_value(2), None);

        // Dictionary and pattern_index are preserved — patterns
        // (preloaded or learned) survive the reset.
        assert_eq!(ctx.pattern_count(), pre_pattern_count);
        assert!(ctx.get_pattern(code).is_some());
        // Version is preserved so the decoder keeps detecting future
        // mismatches against its peer.
        assert_eq!(ctx.version(), pre_version);

        // After reset, a fresh observation must re-build prediction
        // state cleanly (no stale history).
        ctx.observe(&RawData::with_source(1, 99.0, 0));
        assert_eq!(ctx.last_value(1), Some(99.0));
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
        let config = ContextConfig {
            evolution: EvolutionConfig {
                min_frequency: 3,
                max_age: 50,
                evolution_interval: 10,
                promotion_threshold: 5,
                enabled: false, // Manual control
            },
            ..ContextConfig::default()
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
        let config = ContextConfig {
            evolution: EvolutionConfig {
                min_frequency: 2,
                max_age: 1000,
                evolution_interval: 10,
                promotion_threshold: 5,
                enabled: false,
            },
            ..ContextConfig::default()
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
        let config = ContextConfig {
            evolution: EvolutionConfig {
                min_frequency: 1,
                max_age: 10000,
                evolution_interval: 5, // Evolve every 5 observations
                promotion_threshold: 5,
                enabled: true,
            },
            ..ContextConfig::default()
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

    // =======================================================================
    // Bloc D-1 — Context::to_preload_bytes / from_preload_bytes round-trip
    // =======================================================================

    /// Build a context with non-trivial state for roundtrip testing.
    fn trained_context() -> Context {
        let mut ctx = Context::new();
        // Channel 1: 30 observations of a slow-drift signal.
        for i in 0..30 {
            ctx.observe(&RawData::with_source(1, 22.5 + (i as f64) * 0.01, 0));
        }
        // Channel 2: a few observations of a stable signal.
        for _ in 0..15 {
            ctx.observe(&RawData::with_source(2, 1013.25, 0));
        }
        // One pattern so the dictionary isn't empty.
        let _ = ctx
            .register_pattern(Pattern::new(vec![0xBE, 0xEF]))
            .unwrap();
        ctx
    }

    #[test]
    fn test_to_preload_bytes_from_preload_bytes_roundtrip() {
        let ctx = trained_context();
        let bytes = ctx.to_preload_bytes("em500-co2").expect("serialize");
        // Sanity: sidecar target size.
        assert!(
            bytes.len() < 3072,
            "serialized context should be well under 3 KB, got {}",
            bytes.len()
        );

        let restored = Context::from_preload_bytes(&bytes).expect("deserialize");
        // Scalars match exactly.
        assert_eq!(restored.version(), ctx.version());
        assert_eq!(restored.scale_factor(), ctx.scale_factor());
        assert_eq!(restored.observation_count(), ctx.observation_count());
        assert_eq!(restored.source_count(), ctx.source_count());
        assert_eq!(restored.pattern_count(), ctx.pattern_count());

        // Per-source state is bit-exact (f64 equality).
        for sid in [1u32, 2] {
            let a = ctx.source_stats.get(&sid).unwrap();
            let b = restored.source_stats.get(&sid).unwrap();
            assert_eq!(a.count, b.count, "sid {} count", sid);
            assert!(a.last_value.to_bits() == b.last_value.to_bits());
            assert!(a.ema.to_bits() == b.ema.to_bits());
            assert!(a.ema_alpha.to_bits() == b.ema_alpha.to_bits());
            assert!(a.sum_sq_diff.to_bits() == b.sum_sq_diff.to_bits());
            assert!(a.mean.to_bits() == b.mean.to_bits());
            assert_eq!(a.max_history, b.max_history);
            assert_eq!(a.history.len(), b.history.len());
            for (x, y) in a.history.iter().zip(b.history.iter()) {
                assert_eq!(x.to_bits(), y.to_bits(), "sid {} history", sid);
            }
        }
    }

    #[test]
    fn test_from_preload_bytes_rejects_bad_magic() {
        let mut bytes = trained_context().to_preload_bytes("x").unwrap();
        bytes[0] = b'X';
        let r = Context::from_preload_bytes(&bytes);
        assert!(r.is_err());
    }

    #[test]
    fn test_from_preload_bytes_rejects_bad_crc() {
        let mut bytes = trained_context().to_preload_bytes("x").unwrap();
        // Flip a byte in the MIDDLE of the buffer so the CRC changes.
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xFF;
        let r = Context::from_preload_bytes(&bytes);
        match r {
            Err(crate::error::AlecError::Decode(crate::error::DecodeError::InvalidChecksum {
                ..
            })) => {}
            other => panic!("expected InvalidChecksum, got {:?}", other),
        }
    }

    #[test]
    fn test_to_preload_bytes_rejects_oversize_sensor_type() {
        let ctx = Context::new();
        let long: String = "a".repeat(300);
        let r = ctx.to_preload_bytes(&long);
        assert!(r.is_err());
    }
}
