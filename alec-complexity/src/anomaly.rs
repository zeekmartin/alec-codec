// ALEC Complexity - Standalone complexity monitoring
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Anomaly detection and event emission.

use crate::config::AnomalyConfig;
use crate::delta::ZScores;
use crate::event::{ComplexityEvent, EventSeverity, EventType};
use crate::structure::StructureBreak;
use std::collections::HashMap;

/// Anomaly detector with persistence and cooldown.
pub struct AnomalyDetector {
    config: AnomalyConfig,
    /// Tracks when each condition started persisting.
    condition_start_ms: HashMap<EventType, u64>,
    /// Tracks last event emission time for cooldown.
    last_event_ms: HashMap<EventType, u64>,
}

impl AnomalyDetector {
    pub fn new(config: AnomalyConfig) -> Self {
        Self {
            config,
            condition_start_ms: HashMap::new(),
            last_event_ms: HashMap::new(),
        }
    }

    /// Evaluate z-scores and other inputs for anomalies.
    /// Returns list of events to emit.
    pub fn evaluate(
        &mut self,
        z_scores: &ZScores,
        structure_break: Option<&StructureBreak>,
        criticality_change: Option<(Vec<String>, Vec<String>)>,
        timestamp_ms: u64,
    ) -> Vec<ComplexityEvent> {
        if !self.config.enabled {
            return Vec::new();
        }

        let mut events = Vec::new();

        // Check payload entropy spike
        if self.config.events.payload_entropy_spike {
            if let Some(event) = self.check_z_score_event(
                EventType::PayloadEntropySpike,
                z_scores.h_bytes,
                true, // positive spike
                timestamp_ms,
            ) {
                events.push(event);
            }
        }

        // Check complexity surge (TC or H_joint)
        if self.config.events.complexity_surge {
            let max_z = z_scores
                .tc
                .unwrap_or(0.0)
                .max(z_scores.h_joint.unwrap_or(0.0));
            if let Some(event) = self.check_z_score_event(
                EventType::ComplexitySurge,
                max_z,
                true, // positive surge
                timestamp_ms,
            ) {
                events.push(event);
            }
        }

        // Check redundancy drop (R)
        if self.config.events.redundancy_drop {
            if let Some(z_r) = z_scores.r {
                if let Some(event) = self.check_z_score_event(
                    EventType::RedundancyDrop,
                    z_r,
                    false, // negative drop
                    timestamp_ms,
                ) {
                    events.push(event);
                }
            }
        }

        // Check structure break
        if self.config.events.structure_break {
            if let Some(break_info) = structure_break {
                if self.check_cooldown(EventType::StructureBreak, timestamp_ms) {
                    events.push(ComplexityEvent::structure_break(
                        timestamp_ms,
                        break_info.clone(),
                    ));
                    self.last_event_ms
                        .insert(EventType::StructureBreak, timestamp_ms);
                }
            }
        }

        // Check criticality shift
        if self.config.events.criticality_shift {
            if let Some((old_top, new_top)) = criticality_change {
                if self.check_cooldown(EventType::CriticalityShift, timestamp_ms) {
                    events.push(ComplexityEvent::criticality_shift(
                        timestamp_ms,
                        old_top,
                        new_top,
                    ));
                    self.last_event_ms
                        .insert(EventType::CriticalityShift, timestamp_ms);
                }
            }
        }

        events
    }

    /// Check z-score based anomaly with persistence and cooldown.
    fn check_z_score_event(
        &mut self,
        event_type: EventType,
        z_score: f64,
        check_positive: bool,
        timestamp_ms: u64,
    ) -> Option<ComplexityEvent> {
        let (threshold_warn, threshold_crit) = if check_positive {
            (self.config.z_threshold_warn, self.config.z_threshold_crit)
        } else {
            (-self.config.z_threshold_warn, -self.config.z_threshold_crit)
        };

        let exceeds_warn = if check_positive {
            z_score >= threshold_warn
        } else {
            z_score <= threshold_warn
        };

        let exceeds_crit = if check_positive {
            z_score >= threshold_crit
        } else {
            z_score <= threshold_crit
        };

        if !exceeds_warn {
            // Condition cleared - reset persistence
            self.condition_start_ms.remove(&event_type);
            return None;
        }

        // Check persistence
        let start_ms = *self
            .condition_start_ms
            .entry(event_type)
            .or_insert(timestamp_ms);

        let persisted = timestamp_ms.saturating_sub(start_ms) >= self.config.persistence_ms;

        if !persisted {
            return None;
        }

        // Check cooldown
        if !self.check_cooldown(event_type, timestamp_ms) {
            return None;
        }

        // Emit event
        let severity = if exceeds_crit {
            EventSeverity::Critical
        } else {
            EventSeverity::Warning
        };

        let threshold = if check_positive {
            threshold_warn
        } else {
            -threshold_warn
        };

        self.last_event_ms.insert(event_type, timestamp_ms);

        let event = match event_type {
            EventType::PayloadEntropySpike => {
                ComplexityEvent::payload_entropy_spike(timestamp_ms, severity, z_score, threshold)
            }
            EventType::ComplexitySurge => {
                ComplexityEvent::complexity_surge(timestamp_ms, severity, z_score, threshold)
            }
            EventType::RedundancyDrop => {
                ComplexityEvent::redundancy_drop(timestamp_ms, severity, z_score, threshold)
            }
            _ => return None,
        };

        Some(event)
    }

    /// Check if cooldown has passed for event type.
    fn check_cooldown(&self, event_type: EventType, timestamp_ms: u64) -> bool {
        if let Some(&last_ms) = self.last_event_ms.get(&event_type) {
            timestamp_ms.saturating_sub(last_ms) >= self.config.cooldown_ms
        } else {
            true
        }
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.condition_start_ms.clear();
        self.last_event_ms.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EventTypeConfig;

    fn create_test_config() -> AnomalyConfig {
        AnomalyConfig {
            enabled: true,
            z_threshold_warn: 2.0,
            z_threshold_crit: 3.0,
            persistence_ms: 1000,
            cooldown_ms: 5000,
            events: EventTypeConfig::default(),
        }
    }

    #[test]
    fn test_no_event_below_threshold() {
        let config = create_test_config();
        let mut detector = AnomalyDetector::new(config);

        let z_scores = ZScores {
            h_bytes: 1.5, // Below threshold
            ..Default::default()
        };

        let events = detector.evaluate(&z_scores, None, None, 1000);
        assert!(events.is_empty());
    }

    #[test]
    fn test_persistence_required() {
        let config = create_test_config();
        let mut detector = AnomalyDetector::new(config);

        let z_scores = ZScores {
            h_bytes: 2.5, // Above threshold
            ..Default::default()
        };

        // First call - should not emit (persistence not met)
        let events = detector.evaluate(&z_scores, None, None, 1000);
        assert!(events.is_empty());

        // Second call - still not enough time
        let events = detector.evaluate(&z_scores, None, None, 1500);
        assert!(events.is_empty());

        // Third call - persistence met
        let events = detector.evaluate(&z_scores, None, None, 2100);
        assert!(!events.is_empty());
    }

    #[test]
    fn test_cooldown_enforced() {
        let config = create_test_config();
        let mut detector = AnomalyDetector::new(config);

        let z_scores = ZScores {
            h_bytes: 2.5,
            ..Default::default()
        };

        // Emit first event
        detector.evaluate(&z_scores, None, None, 0);
        let events = detector.evaluate(&z_scores, None, None, 2000);
        assert!(!events.is_empty());

        // Try to emit again - should be blocked by cooldown
        let events = detector.evaluate(&z_scores, None, None, 3000);
        assert!(events.is_empty());

        // After cooldown - should emit
        let events = detector.evaluate(&z_scores, None, None, 10000);
        assert!(!events.is_empty());
    }

    #[test]
    fn test_severity_levels() {
        let mut config = create_test_config();
        config.persistence_ms = 0; // No persistence for test
        let mut detector = AnomalyDetector::new(config);

        // Warning level
        let z_scores = ZScores {
            h_bytes: 2.5,
            ..Default::default()
        };
        let events = detector.evaluate(&z_scores, None, None, 1000);
        assert_eq!(events[0].severity, EventSeverity::Warning);

        detector.reset();

        // Critical level
        let z_scores = ZScores {
            h_bytes: 3.5,
            ..Default::default()
        };
        let events = detector.evaluate(&z_scores, None, None, 2000);
        assert_eq!(events[0].severity, EventSeverity::Critical);
    }

    #[test]
    fn test_multiple_events() {
        let mut config = create_test_config();
        config.persistence_ms = 0;
        config.cooldown_ms = 0;
        let mut detector = AnomalyDetector::new(config);

        let z_scores = ZScores {
            h_bytes: 2.5,
            tc: Some(2.5),
            h_joint: Some(2.5),
            r: Some(-2.5), // Negative for redundancy drop
        };

        let events = detector.evaluate(&z_scores, None, None, 1000);
        assert!(events.len() >= 3); // At least payload, complexity, redundancy
    }

    #[test]
    fn test_structure_break_event() {
        let mut config = create_test_config();
        config.persistence_ms = 0;
        let mut detector = AnomalyDetector::new(config);

        let break_info = StructureBreak {
            changed_edges: vec![],
            total_change: 0.5,
        };

        let events = detector.evaluate(&ZScores::default(), Some(&break_info), None, 1000);
        assert!(!events.is_empty());
        assert_eq!(events[0].event_type, EventType::StructureBreak);
    }

    #[test]
    fn test_disabled_detection() {
        let mut config = create_test_config();
        config.enabled = false;
        let mut detector = AnomalyDetector::new(config);

        let z_scores = ZScores {
            h_bytes: 5.0,
            ..Default::default()
        };

        let events = detector.evaluate(&z_scores, None, None, 1000);
        assert!(events.is_empty());
    }

    #[test]
    fn test_condition_cleared() {
        let config = create_test_config();
        let mut detector = AnomalyDetector::new(config);

        // Start persisting
        let z_scores = ZScores {
            h_bytes: 2.5,
            ..Default::default()
        };
        detector.evaluate(&z_scores, None, None, 0);

        // Clear condition
        let z_scores = ZScores {
            h_bytes: 1.0, // Below threshold
            ..Default::default()
        };
        detector.evaluate(&z_scores, None, None, 500);

        // Back above threshold - should start fresh
        let z_scores = ZScores {
            h_bytes: 2.5,
            ..Default::default()
        };
        let events = detector.evaluate(&z_scores, None, None, 1000);
        assert!(events.is_empty()); // Persistence reset
    }
}
