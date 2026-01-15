// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Data classification module
//!
//! This module determines the priority level (P1-P5) of each data point
//! based on its deviation from predictions and configured thresholds.

use crate::context::Context;
use crate::protocol::{Priority, RawData};
use std::collections::HashMap;

/// Classification result for a data point
#[derive(Debug, Clone, PartialEq)]
pub struct Classification {
    /// Assigned priority level
    pub priority: Priority,
    /// Reason for this classification
    pub reason: ClassificationReason,
    /// Delta from prediction (relative, 0.0-1.0+)
    pub delta: f64,
    /// Confidence in the prediction (0.0-1.0)
    pub confidence: f32,
}

impl Classification {
    /// Create a new classification
    pub fn new(
        priority: Priority,
        reason: ClassificationReason,
        delta: f64,
        confidence: f32,
    ) -> Self {
        Self {
            priority,
            reason,
            delta,
            confidence,
        }
    }

    /// Create classification for when no prediction is available
    pub fn no_prediction() -> Self {
        Self {
            priority: Priority::P3Normal,
            reason: ClassificationReason::NoPrediction,
            delta: 0.0,
            confidence: 0.0,
        }
    }
}

/// Reason for a classification decision
#[derive(Debug, Clone, PartialEq)]
pub enum ClassificationReason {
    /// Value exceeded a critical threshold
    ThresholdExceeded { threshold: f64, actual: f64 },
    /// Statistical anomaly detected
    AnomalyDetected { anomaly_type: AnomalyType },
    /// Regular scheduled transmission
    ScheduledTransmission,
    /// Value is essentially the same as predicted
    BelowMinimumDelta,
    /// Normal value, small deviation
    NormalValue,
    /// No prediction available (cold start)
    NoPrediction,
    /// Explicitly requested by user
    UserRequested,
}

/// Types of detected anomalies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalyType {
    /// Extreme deviation (> critical threshold)
    ExtremeDeviation,
    /// Significant deviation (> anomaly threshold)
    SignificantDeviation,
    /// Sudden spike
    Spike,
    /// Gradual drift detected
    Drift,
    /// Value outside expected range
    OutOfRange,
}

/// Critical thresholds for a source
#[derive(Debug, Clone, PartialEq)]
pub struct CriticalThresholds {
    /// Minimum acceptable value
    pub min: f64,
    /// Maximum acceptable value
    pub max: f64,
}

impl CriticalThresholds {
    /// Create new thresholds
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }
}

/// Configuration for the classifier
#[derive(Debug, Clone)]
pub struct ClassifierConfig {
    /// Relative delta threshold for anomaly detection (default: 0.15 = 15%)
    pub anomaly_threshold: f64,
    /// Relative delta threshold for critical anomaly (default: 0.30 = 30%)
    pub critical_anomaly_threshold: f64,
    /// Minimum delta to bother sending (default: 0.01 = 1%)
    pub minimum_delta_threshold: f64,
    /// Critical thresholds by source_id
    pub critical_thresholds: HashMap<u32, CriticalThresholds>,
    /// Scheduled transmission interval in seconds (0 = disabled)
    pub scheduled_interval: u64,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        Self {
            anomaly_threshold: 0.15,
            critical_anomaly_threshold: 0.30,
            minimum_delta_threshold: 0.01,
            critical_thresholds: HashMap::new(),
            scheduled_interval: 0,
        }
    }
}

/// Data classifier that assigns priority levels
#[derive(Debug, Clone)]
pub struct Classifier {
    config: ClassifierConfig,
    _last_scheduled: HashMap<u32, u64>,
}

impl Classifier {
    /// Create a new classifier with default configuration
    pub fn new() -> Self {
        Self {
            config: ClassifierConfig::default(),
            _last_scheduled: HashMap::new(),
        }
    }

    /// Create a classifier with custom configuration
    pub fn with_config(config: ClassifierConfig) -> Self {
        Self {
            config,
            _last_scheduled: HashMap::new(),
        }
    }

    /// Set critical thresholds for a source
    pub fn set_critical_thresholds(&mut self, source_id: u32, min: f64, max: f64) {
        self.config
            .critical_thresholds
            .insert(source_id, CriticalThresholds::new(min, max));
    }

    /// Classify a data point
    pub fn classify(&self, data: &RawData, context: &Context) -> Classification {
        // Try to get prediction
        let prediction = match context.predict(data.source_id) {
            Some(p) => p,
            None => return Classification::no_prediction(),
        };

        // Calculate delta
        let delta_info = self.calculate_delta(data.value, prediction.value);

        // Check critical thresholds first (highest priority)
        if let Some(classification) =
            self.check_critical_thresholds(data.value, data.source_id, &delta_info)
        {
            return classification;
        }

        // Check for anomalies
        if let Some(classification) = self.check_anomaly(&delta_info, prediction.confidence) {
            return classification;
        }

        // Normal classification
        self.classify_normal(data.timestamp, &delta_info, prediction.confidence)
    }

    /// Calculate absolute and relative delta
    fn calculate_delta(&self, value: f64, predicted: f64) -> DeltaInfo {
        let absolute = (value - predicted).abs();
        let relative = if predicted.abs() > f64::EPSILON {
            absolute / predicted.abs()
        } else {
            absolute
        };
        DeltaInfo { absolute, relative }
    }

    /// Check if value exceeds critical thresholds
    fn check_critical_thresholds(
        &self,
        value: f64,
        source_id: u32,
        delta: &DeltaInfo,
    ) -> Option<Classification> {
        let thresholds = self.config.critical_thresholds.get(&source_id)?;

        let violated = if value < thresholds.min {
            Some(thresholds.min)
        } else if value > thresholds.max {
            Some(thresholds.max)
        } else {
            None
        }?;

        Some(Classification::new(
            Priority::P1Critical,
            ClassificationReason::ThresholdExceeded {
                threshold: violated,
                actual: value,
            },
            delta.relative,
            1.0,
        ))
    }

    /// Check for statistical anomalies
    fn check_anomaly(&self, delta: &DeltaInfo, confidence: f32) -> Option<Classification> {
        if delta.relative <= self.config.anomaly_threshold {
            return None;
        }

        let (priority, anomaly_type) = if delta.relative > self.config.critical_anomaly_threshold {
            (Priority::P1Critical, AnomalyType::ExtremeDeviation)
        } else {
            (Priority::P2Important, AnomalyType::SignificantDeviation)
        };

        Some(Classification::new(
            priority,
            ClassificationReason::AnomalyDetected { anomaly_type },
            delta.relative,
            confidence,
        ))
    }

    /// Classify normal (non-anomalous) values
    fn classify_normal(
        &self,
        _timestamp: u64,
        delta: &DeltaInfo,
        confidence: f32,
    ) -> Classification {
        // Check if delta is too small to bother sending
        if delta.relative < self.config.minimum_delta_threshold {
            return Classification::new(
                Priority::P5Disposable,
                ClassificationReason::BelowMinimumDelta,
                delta.relative,
                confidence,
            );
        }

        // Check if this is a scheduled transmission time
        // (simplified: would need timestamp checking in real impl)
        if self.config.scheduled_interval > 0 {
            return Classification::new(
                Priority::P3Normal,
                ClassificationReason::ScheduledTransmission,
                delta.relative,
                confidence,
            );
        }

        // Default: deferred
        Classification::new(
            Priority::P4Deferred,
            ClassificationReason::NormalValue,
            delta.relative,
            confidence,
        )
    }

    /// Get current configuration
    pub fn config(&self) -> &ClassifierConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: ClassifierConfig) {
        self.config = config;
    }
}

impl Default for Classifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal struct for delta calculations
struct DeltaInfo {
    #[allow(dead_code)]
    absolute: f64,
    relative: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context_with_prediction(value: f64) -> Context {
        let mut ctx = Context::new();
        // Observe some values to build prediction
        for i in 0..10 {
            let data = RawData::new(value + (i as f64 * 0.001), i as u64);
            ctx.observe(&data);
        }
        ctx
    }

    #[test]
    fn test_classify_no_prediction() {
        let classifier = Classifier::new();
        let context = Context::new(); // Empty context
        let data = RawData::new(42.0, 0);

        let result = classifier.classify(&data, &context);

        assert_eq!(result.priority, Priority::P3Normal);
        assert!(matches!(result.reason, ClassificationReason::NoPrediction));
    }

    #[test]
    fn test_classify_normal_value() {
        let classifier = Classifier::new();
        let context = make_context_with_prediction(20.0);
        let data = RawData::new(20.1, 100); // Small deviation

        let result = classifier.classify(&data, &context);

        // Should be P4 or P5 depending on delta
        assert!(matches!(
            result.priority,
            Priority::P4Deferred | Priority::P5Disposable
        ));
    }

    #[test]
    fn test_classify_critical_threshold() {
        let mut classifier = Classifier::new();
        classifier.set_critical_thresholds(0, 10.0, 30.0);

        let context = make_context_with_prediction(20.0);
        let data = RawData::new(5.0, 100); // Below minimum!

        let result = classifier.classify(&data, &context);

        assert_eq!(result.priority, Priority::P1Critical);
        assert!(matches!(
            result.reason,
            ClassificationReason::ThresholdExceeded { .. }
        ));
    }

    #[test]
    fn test_classify_anomaly() {
        let classifier = Classifier::new();
        let context = make_context_with_prediction(20.0);
        let data = RawData::new(30.0, 100); // 50% deviation!

        let result = classifier.classify(&data, &context);

        assert!(matches!(
            result.priority,
            Priority::P1Critical | Priority::P2Important
        ));
        assert!(matches!(
            result.reason,
            ClassificationReason::AnomalyDetected { .. }
        ));
    }

    #[test]
    fn test_classify_disposable() {
        let classifier = Classifier::new();
        let context = make_context_with_prediction(20.0);
        let data = RawData::new(20.001, 100); // Tiny deviation

        let result = classifier.classify(&data, &context);

        assert_eq!(result.priority, Priority::P5Disposable);
        assert!(matches!(
            result.reason,
            ClassificationReason::BelowMinimumDelta
        ));
    }

    #[test]
    fn test_priority_should_transmit() {
        assert!(Priority::P1Critical.should_transmit());
        assert!(Priority::P2Important.should_transmit());
        assert!(Priority::P3Normal.should_transmit());
        assert!(!Priority::P4Deferred.should_transmit());
        assert!(!Priority::P5Disposable.should_transmit());
    }
}
