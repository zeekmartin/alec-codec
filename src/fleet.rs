//! Fleet management for multi-emitter scenarios
//!
//! Manages multiple contexts and provides cross-fleet analytics.
//! Supports:
//! - Individual contexts per emitter
//! - Shared fleet-wide context for common patterns
//! - Cross-fleet anomaly detection
//! - Fleet-wide statistics

use std::collections::HashMap;

use crate::classifier::Classifier;
use crate::context::{Context, Pattern};
use crate::decoder::Decoder;
use crate::error::Result;
use crate::protocol::{Priority, RawData};

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
    /// Maximum recent values to keep
    max_recent: usize,
    /// Is this emitter behaving anomalously?
    pub is_anomalous: bool,
}

impl EmitterState {
    /// Create a new emitter state
    pub fn new() -> Self {
        Self {
            context: Context::new(),
            last_seen: 0,
            message_count: 0,
            recent_values: Vec::with_capacity(100),
            max_recent: 100,
            is_anomalous: false,
        }
    }

    /// Create with custom recent values capacity
    pub fn with_capacity(max_recent: usize) -> Self {
        Self {
            context: Context::new(),
            last_seen: 0,
            message_count: 0,
            recent_values: Vec::with_capacity(max_recent),
            max_recent,
            is_anomalous: false,
        }
    }

    /// Record a new value
    pub fn record_value(&mut self, value: f64, timestamp: u64) {
        self.last_seen = timestamp;
        self.message_count += 1;

        // Keep last N values
        if self.recent_values.len() >= self.max_recent {
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

    /// Calculate standard deviation of recent values
    pub fn std_dev(&self) -> Option<f64> {
        let mean = self.mean()?;
        if self.recent_values.len() < 2 {
            return None;
        }

        let variance = self
            .recent_values
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>()
            / (self.recent_values.len() - 1) as f64;

        Some(variance.sqrt())
    }

    /// Get the last recorded value
    pub fn last_value(&self) -> Option<f64> {
        self.recent_values.last().copied()
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
    /// Threshold for cross-fleet anomaly (standard deviations)
    pub cross_fleet_threshold: f64,
    /// Minimum emitters for cross-fleet analysis
    pub min_emitters_for_comparison: usize,
    /// How often to promote patterns to fleet context
    pub fleet_sync_interval: u64,
    /// Maximum recent values to track per emitter
    pub max_recent_values: usize,
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            max_emitters: 1000,
            emitter_timeout: 300,
            cross_fleet_threshold: 3.0,
            min_emitters_for_comparison: 3,
            fleet_sync_interval: 1000,
            max_recent_values: 100,
        }
    }
}

/// Result of processing a message
#[derive(Debug, Clone)]
pub struct ProcessedMessage {
    /// ID of the emitter
    pub emitter_id: EmitterId,
    /// Decoded value
    pub value: f64,
    /// Assigned priority
    pub priority: Priority,
    /// Whether this triggered a cross-fleet anomaly
    pub is_cross_fleet_anomaly: bool,
}

/// Manages a fleet of emitters
#[derive(Debug)]
pub struct FleetManager {
    /// Individual contexts per emitter
    emitter_contexts: HashMap<EmitterId, EmitterState>,
    /// Shared fleet-wide context (common patterns)
    fleet_context: Context,
    /// Classifier for fleet-wide analysis
    #[allow(dead_code)]
    classifier: Classifier,
    /// Decoder
    decoder: Decoder,
    /// Configuration
    config: FleetConfig,
    /// Statistics
    stats: FleetStats,
    /// Message counter for sync interval
    message_counter: u64,
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
            message_counter: 0,
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: FleetConfig) -> Self {
        Self {
            emitter_contexts: HashMap::new(),
            fleet_context: Context::new(),
            classifier: Classifier::default(),
            decoder: Decoder::new(),
            config,
            stats: FleetStats::default(),
            message_counter: 0,
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
        let max_recent = self.config.max_recent_values;
        let emitter = self
            .emitter_contexts
            .entry(emitter_id)
            .or_insert_with(|| EmitterState::with_capacity(max_recent));

        // Decode message
        let decoded = self.decoder.decode(message, &emitter.context)?;

        // Update emitter state
        emitter.record_value(decoded.value, timestamp);
        emitter
            .context
            .observe(&RawData::new(decoded.value, timestamp));

        // Update stats
        self.stats.total_messages += 1;
        self.stats.emitter_count = self.emitter_contexts.len();
        *self
            .stats
            .priority_distribution
            .entry(decoded.priority)
            .or_insert(0) += 1;

        // Check for cross-fleet anomaly
        let cross_fleet_anomaly = self.check_cross_fleet_anomaly(emitter_id, decoded.value);
        if cross_fleet_anomaly {
            self.stats.cross_fleet_anomalies += 1;
            if let Some(e) = self.emitter_contexts.get_mut(&emitter_id) {
                e.is_anomalous = true;
            }
        }

        // Check for regular anomaly
        if decoded.priority == Priority::P1Critical || decoded.priority == Priority::P2Important {
            self.stats.anomaly_count += 1;
        }

        // Periodic fleet sync
        self.message_counter += 1;
        if self.message_counter >= self.config.fleet_sync_interval {
            self.sync_fleet_patterns();
            self.message_counter = 0;
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

        // Calculate fleet-wide mean from other emitters
        let other_means: Vec<f64> = self
            .emitter_contexts
            .iter()
            .filter(|(id, _)| **id != emitter_id)
            .filter_map(|(_, state)| state.mean())
            .collect();

        if other_means.len() < self.config.min_emitters_for_comparison - 1 {
            return false;
        }

        let fleet_mean = other_means.iter().sum::<f64>() / other_means.len() as f64;
        let fleet_variance = other_means
            .iter()
            .map(|m| (m - fleet_mean).powi(2))
            .sum::<f64>()
            / other_means.len() as f64;
        let fleet_std = fleet_variance.sqrt();

        if fleet_std < 0.001 {
            // Avoid division by near-zero
            return (value - fleet_mean).abs() > 1.0;
        }

        // Check if this value is outside threshold
        let z_score = (value - fleet_mean).abs() / fleet_std;
        z_score > self.config.cross_fleet_threshold
    }

    /// Get list of active emitters
    pub fn active_emitters(&self, current_time: u64) -> Vec<EmitterId> {
        self.emitter_contexts
            .iter()
            .filter(|(_, state)| current_time - state.last_seen < self.config.emitter_timeout)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get list of anomalous emitters
    pub fn anomalous_emitters(&self) -> Vec<EmitterId> {
        self.emitter_contexts
            .iter()
            .filter(|(_, state)| state.is_anomalous)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get emitter state
    pub fn get_emitter(&self, id: EmitterId) -> Option<&EmitterState> {
        self.emitter_contexts.get(&id)
    }

    /// Get mutable emitter state
    pub fn get_emitter_mut(&mut self, id: EmitterId) -> Option<&mut EmitterState> {
        self.emitter_contexts.get_mut(&id)
    }

    /// Get fleet statistics
    pub fn stats(&self) -> &FleetStats {
        &self.stats
    }

    /// Get fleet-wide context
    pub fn fleet_context(&self) -> &Context {
        &self.fleet_context
    }

    /// Get mutable fleet context
    pub fn fleet_context_mut(&mut self) -> &mut Context {
        &mut self.fleet_context
    }

    /// Get number of tracked emitters
    pub fn emitter_count(&self) -> usize {
        self.emitter_contexts.len()
    }

    /// Iterate over all emitters
    pub fn emitters(&self) -> impl Iterator<Item = (&EmitterId, &EmitterState)> {
        self.emitter_contexts.iter()
    }

    /// Promote common patterns to fleet context
    pub fn sync_fleet_patterns(&mut self) {
        if self.emitter_contexts.len() < 2 {
            return;
        }

        // Find patterns that appear in multiple emitters
        let mut pattern_counts: HashMap<u64, (u32, Option<Pattern>)> = HashMap::new();

        for state in self.emitter_contexts.values() {
            for (_id, pattern) in state.context.patterns_iter() {
                let hash = xxhash_rust::xxh64::xxh64(&pattern.data, 0);
                let entry = pattern_counts.entry(hash).or_insert((0, None));
                entry.0 += 1;
                if entry.1.is_none() {
                    entry.1 = Some(pattern.clone());
                }
            }
        }

        // Promote patterns found in >50% of emitters
        let threshold = self.emitter_contexts.len() / 2;
        for (_, (count, pattern_opt)) in pattern_counts {
            if count as usize > threshold {
                if let Some(pattern) = pattern_opt {
                    // Add to fleet context if not already present
                    if self.fleet_context.find_pattern(&pattern.data).is_none() {
                        let _ = self.fleet_context.register_pattern(pattern);
                    }
                }
            }
        }
    }

    /// Remove stale emitters
    pub fn cleanup_stale_emitters(&mut self, current_time: u64) {
        let timeout = self.config.emitter_timeout * 2;
        self.emitter_contexts
            .retain(|_, state| current_time - state.last_seen < timeout);
        self.stats.emitter_count = self.emitter_contexts.len();
    }

    /// Reset an emitter's anomaly flag
    pub fn clear_anomaly(&mut self, emitter_id: EmitterId) {
        if let Some(emitter) = self.emitter_contexts.get_mut(&emitter_id) {
            emitter.is_anomalous = false;
        }
    }

    /// Get fleet-wide mean across all emitters
    pub fn fleet_mean(&self) -> Option<f64> {
        let means: Vec<f64> = self
            .emitter_contexts
            .values()
            .filter_map(|s| s.mean())
            .collect();

        if means.is_empty() {
            return None;
        }

        Some(means.iter().sum::<f64>() / means.len() as f64)
    }

    /// Get fleet-wide standard deviation
    pub fn fleet_std_dev(&self) -> Option<f64> {
        let fleet_mean = self.fleet_mean()?;
        let means: Vec<f64> = self
            .emitter_contexts
            .values()
            .filter_map(|s| s.mean())
            .collect();

        if means.len() < 2 {
            return None;
        }

        let variance =
            means.iter().map(|m| (m - fleet_mean).powi(2)).sum::<f64>() / (means.len() - 1) as f64;

        Some(variance.sqrt())
    }
}

impl Default for FleetManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fleet_manager_creation() {
        let fleet = FleetManager::new();
        assert_eq!(fleet.stats().emitter_count, 0);
        assert_eq!(fleet.stats().total_messages, 0);
    }

    #[test]
    fn test_emitter_state_mean() {
        let mut state = EmitterState::new();
        state.record_value(10.0, 0);
        state.record_value(20.0, 1);
        state.record_value(30.0, 2);

        assert_eq!(state.mean(), Some(20.0));
        assert_eq!(state.message_count, 3);
        assert_eq!(state.last_seen, 2);
    }

    #[test]
    fn test_emitter_state_std_dev() {
        let mut state = EmitterState::new();
        state.record_value(10.0, 0);
        state.record_value(20.0, 1);
        state.record_value(30.0, 2);

        let std_dev = state.std_dev().unwrap();
        // std dev of [10, 20, 30] = 10
        assert!((std_dev - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_emitter_state_empty() {
        let state = EmitterState::new();
        assert!(state.mean().is_none());
        assert!(state.std_dev().is_none());
        assert!(state.last_value().is_none());
    }

    #[test]
    fn test_cross_fleet_anomaly_detection() {
        let mut fleet = FleetManager::with_config(FleetConfig {
            min_emitters_for_comparison: 2,
            cross_fleet_threshold: 2.0,
            ..Default::default()
        });

        // Add normal emitters with similar values
        for i in 0..5 {
            let mut state = EmitterState::new();
            for j in 0..10 {
                state.record_value(20.0 + (j as f64 * 0.1), j);
            }
            fleet.emitter_contexts.insert(i, state);
        }

        // Check that a very different value is detected as anomaly
        let is_anomaly = fleet.check_cross_fleet_anomaly(99, 100.0);
        assert!(is_anomaly);

        // Check that a similar value is not anomaly
        let is_normal = fleet.check_cross_fleet_anomaly(99, 20.5);
        assert!(!is_normal);
    }

    #[test]
    fn test_active_emitters() {
        let mut fleet = FleetManager::with_config(FleetConfig {
            emitter_timeout: 100,
            ..Default::default()
        });

        // Add emitters with different timestamps
        let mut state1 = EmitterState::new();
        state1.last_seen = 100;
        fleet.emitter_contexts.insert(1, state1);

        let mut state2 = EmitterState::new();
        state2.last_seen = 150;
        fleet.emitter_contexts.insert(2, state2);

        let mut state3 = EmitterState::new();
        state3.last_seen = 10; // Old
        fleet.emitter_contexts.insert(3, state3);

        // At time 160, with timeout 100:
        // state1: 160-100=60 < 100, active
        // state2: 160-150=10 < 100, active
        // state3: 160-10=150 > 100, not active
        let active = fleet.active_emitters(160);
        assert_eq!(active.len(), 2);

        // At time 250, with timeout 100:
        // state1: 250-100=150 > 100, not active
        // state2: 250-150=100 NOT < 100, not active
        // state3: 250-10=240 > 100, not active
        // Actually state2: 250-150=100 is NOT < 100, so not active
        // Let's check at time 249:
        // state2: 249-150=99 < 100, active
        let active = fleet.active_emitters(249);
        assert_eq!(active.len(), 1);
        assert!(active.contains(&2));
    }

    #[test]
    fn test_cleanup_stale_emitters() {
        let mut fleet = FleetManager::with_config(FleetConfig {
            emitter_timeout: 100,
            ..Default::default()
        });

        let mut state1 = EmitterState::new();
        state1.last_seen = 150; // Recent
        fleet.emitter_contexts.insert(1, state1);

        let mut state2 = EmitterState::new();
        state2.last_seen = 10; // Old
        fleet.emitter_contexts.insert(2, state2);

        // Cleanup at time 250 (timeout*2 = 200)
        // state1: 250-150=100 < 200, kept
        // state2: 250-10=240 > 200, removed
        fleet.cleanup_stale_emitters(250);

        assert_eq!(fleet.emitter_count(), 1);
        assert!(fleet.get_emitter(1).is_some());
        assert!(fleet.get_emitter(2).is_none());
    }

    #[test]
    fn test_fleet_mean() {
        let mut fleet = FleetManager::new();

        // Add emitters with known means
        let mut state1 = EmitterState::new();
        state1.record_value(10.0, 0);
        state1.record_value(10.0, 1);
        fleet.emitter_contexts.insert(1, state1);

        let mut state2 = EmitterState::new();
        state2.record_value(20.0, 0);
        state2.record_value(20.0, 1);
        fleet.emitter_contexts.insert(2, state2);

        let mut state3 = EmitterState::new();
        state3.record_value(30.0, 0);
        state3.record_value(30.0, 1);
        fleet.emitter_contexts.insert(3, state3);

        // Fleet mean should be (10+20+30)/3 = 20
        assert_eq!(fleet.fleet_mean(), Some(20.0));
    }

    #[test]
    fn test_anomalous_emitters() {
        let mut fleet = FleetManager::new();

        let mut state1 = EmitterState::new();
        state1.is_anomalous = true;
        fleet.emitter_contexts.insert(1, state1);

        let mut state2 = EmitterState::new();
        state2.is_anomalous = false;
        fleet.emitter_contexts.insert(2, state2);

        let anomalous = fleet.anomalous_emitters();
        assert_eq!(anomalous.len(), 1);
        assert!(anomalous.contains(&1));
    }

    #[test]
    fn test_recent_values_capacity() {
        let mut state = EmitterState::with_capacity(5);

        // Record more than capacity
        for i in 0..10 {
            state.record_value(i as f64, i);
        }

        // Should only keep last 5
        assert_eq!(state.recent_values.len(), 5);
        assert_eq!(state.recent_values, vec![5.0, 6.0, 7.0, 8.0, 9.0]);
    }
}
