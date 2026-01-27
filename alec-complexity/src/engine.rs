// ALEC Complexity - Standalone complexity monitoring
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! ComplexityEngine - main orchestration for complexity monitoring.

use crate::anomaly::AnomalyDetector;
use crate::baseline::BaselineBuilder;
use crate::config::ComplexityConfig;
use crate::delta::DeltaCalculator;
use crate::event::ComplexityEvent;
use crate::input::InputSnapshot;
use crate::snapshot::ComplexitySnapshot;
use crate::structure::SLiteExtractor;

/// Main complexity engine orchestrating all components.
pub struct ComplexityEngine {
    config: ComplexityConfig,
    baseline_builder: BaselineBuilder,
    delta_calculator: DeltaCalculator,
    structure_extractor: SLiteExtractor,
    anomaly_detector: AnomalyDetector,

    /// Last top critical channels for shift detection.
    last_top_critical: Option<Vec<String>>,
    /// Total snapshots processed.
    snapshot_count: u64,
    /// Last snapshot output.
    last_output: Option<ComplexitySnapshot>,
    /// Whether baseline lock event was emitted.
    baseline_lock_emitted: bool,
}

impl ComplexityEngine {
    /// Create a new complexity engine with the given configuration.
    pub fn new(config: ComplexityConfig) -> Self {
        let track_r = config.deltas.compute_r;
        Self {
            baseline_builder: BaselineBuilder::new(config.baseline.clone(), track_r),
            delta_calculator: DeltaCalculator::new(config.deltas.clone()),
            structure_extractor: SLiteExtractor::new(config.structure.clone()),
            anomaly_detector: AnomalyDetector::new(config.anomaly.clone()),
            config,
            last_top_critical: None,
            snapshot_count: 0,
            last_output: None,
            baseline_lock_emitted: false,
        }
    }

    /// Process an input snapshot and return complexity analysis.
    /// Returns None if complexity monitoring is disabled.
    pub fn process(&mut self, input: &InputSnapshot) -> Option<ComplexitySnapshot> {
        if !self.config.enabled {
            return None;
        }

        self.snapshot_count += 1;

        let mut events = Vec::new();

        // Process baseline
        let just_locked = self.baseline_builder.process(
            input.tc,
            input.h_joint,
            input.h_bytes,
            input.r,
            input.timestamp_ms,
        );

        // Check baseline state and emit events (scope limits baseline borrow)
        let baseline_ready = {
            let baseline = self.baseline_builder.baseline();

            // Emit baseline events
            if self.config.anomaly.events.baseline_events {
                if !baseline.is_ready() {
                    events.push(ComplexityEvent::baseline_building(
                        input.timestamp_ms,
                        baseline.build_progress,
                    ));
                } else if just_locked && !self.baseline_lock_emitted {
                    events.push(ComplexityEvent::baseline_locked(input.timestamp_ms));
                    self.baseline_lock_emitted = true;
                }
            }

            // If baseline is still building, return early snapshot
            if !baseline.is_ready() {
                let output = ComplexitySnapshot::building(input.timestamp_ms, baseline, events);
                self.last_output = Some(output.clone());
                return Some(output);
            }

            true
        };

        if !baseline_ready {
            return None; // Should not reach here
        }

        // Extract S-lite if we have channel data
        let s_lite = self.structure_extractor.extract(input);

        // Detect structure break
        let structure_break = if let Some(ref current) = s_lite {
            self.structure_extractor.detect_break(current)
        } else {
            None
        };

        // Detect criticality shift (if we have channel data with signal metrics)
        let criticality_change = self.detect_criticality_change(input);

        // Now get baseline again for delta computation
        let baseline = self.baseline_builder.baseline();

        // Compute deltas and z-scores
        let (deltas, z_scores) = self.delta_calculator.compute(
            baseline,
            input.tc,
            input.h_joint,
            input.h_bytes,
            input.r,
        );

        // Evaluate anomalies
        let anomaly_events = self.anomaly_detector.evaluate(
            &z_scores,
            structure_break.as_ref(),
            criticality_change,
            input.timestamp_ms,
        );
        events.extend(anomaly_events);

        // Build flags
        let flags = self.build_flags(&structure_break.is_some());

        // Create output snapshot
        let output = ComplexitySnapshot::new(
            input.timestamp_ms,
            baseline,
            Some(deltas),
            Some(z_scores),
            s_lite,
            events,
            flags,
        );

        self.last_output = Some(output.clone());
        Some(output)
    }

    /// Get the current baseline.
    pub fn baseline(&self) -> &crate::baseline::Baseline {
        self.baseline_builder.baseline()
    }

    /// Check if baseline is locked.
    pub fn is_baseline_locked(&self) -> bool {
        self.baseline_builder.baseline().is_ready()
    }

    /// Get the last output snapshot.
    pub fn last_output(&self) -> Option<&ComplexitySnapshot> {
        self.last_output.as_ref()
    }

    /// Get total snapshots processed.
    pub fn snapshot_count(&self) -> u64 {
        self.snapshot_count
    }

    /// Get current configuration.
    pub fn config(&self) -> &ComplexityConfig {
        &self.config
    }

    /// Check if engine is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        let track_r = self.config.deltas.compute_r;
        self.baseline_builder = BaselineBuilder::new(self.config.baseline.clone(), track_r);
        self.delta_calculator = DeltaCalculator::new(self.config.deltas.clone());
        self.structure_extractor = SLiteExtractor::new(self.config.structure.clone());
        self.anomaly_detector.reset();
        self.last_top_critical = None;
        self.snapshot_count = 0;
        self.last_output = None;
        self.baseline_lock_emitted = false;
    }

    /// Export baseline state for persistence.
    pub fn export_baseline(&self) -> Option<String> {
        let baseline = self.baseline_builder.export();
        serde_json::to_string(&baseline).ok()
    }

    /// Import baseline state from persistence.
    pub fn import_baseline(&mut self, json: &str) -> Result<(), String> {
        let baseline: crate::baseline::Baseline =
            serde_json::from_str(json).map_err(|e| e.to_string())?;
        self.baseline_builder.import(baseline);
        if self.baseline_builder.baseline().is_ready() {
            self.baseline_lock_emitted = true;
        }
        Ok(())
    }

    fn detect_criticality_change(
        &mut self,
        input: &InputSnapshot,
    ) -> Option<(Vec<String>, Vec<String>)> {
        if !self.config.anomaly.events.criticality_shift {
            return None;
        }

        // Need channel data
        if input.channel_entropies.is_empty() {
            return None;
        }

        // Sort channels by entropy to find most critical
        let mut channel_entropies: Vec<_> = input
            .channel_entropies
            .iter()
            .map(|c| (c.channel_id.clone(), c.h))
            .collect();

        channel_entropies
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top 3 (or fewer if not enough channels)
        let top_n = 3.min(channel_entropies.len());
        let new_top: Vec<String> = channel_entropies[..top_n]
            .iter()
            .map(|(id, _)| id.clone())
            .collect();

        // Compare with previous
        let result = if let Some(old_top) = &self.last_top_critical {
            if *old_top != new_top {
                Some((old_top.clone(), new_top.clone()))
            } else {
                None
            }
        } else {
            None
        };

        self.last_top_critical = Some(new_top);
        result
    }

    fn build_flags(&self, has_structure_break: &bool) -> Vec<String> {
        let mut flags = Vec::new();

        if self.baseline_builder.baseline().is_ready() {
            flags.push("BASELINE_LOCKED".to_string());
        }

        if self.config.deltas.smoothing.enabled {
            flags.push(format!(
                "SMOOTHING_EMA_{:.2}",
                self.config.deltas.smoothing.alpha
            ));
        }

        if *has_structure_break {
            flags.push("STRUCTURE_BREAK_DETECTED".to_string());
        }

        if self.config.anomaly.enabled {
            flags.push("ANOMALY_DETECTION_ENABLED".to_string());
        }

        flags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AnomalyConfig, BaselineConfig, EventTypeConfig};
    use crate::event::EventType;
    use crate::input::ChannelEntropy;

    fn create_test_config() -> ComplexityConfig {
        ComplexityConfig {
            enabled: true,
            baseline: BaselineConfig {
                build_time_ms: 0,
                min_valid_snapshots: 2,
                ..Default::default()
            },
            anomaly: AnomalyConfig {
                enabled: true,
                z_threshold_warn: 2.0,
                z_threshold_crit: 3.0,
                persistence_ms: 0,
                cooldown_ms: 0,
                events: EventTypeConfig::default(),
            },
            ..Default::default()
        }
    }

    fn create_input(timestamp_ms: u64, h_bytes: f64) -> InputSnapshot {
        InputSnapshot {
            timestamp_ms,
            tc: Some(1.0),
            h_joint: Some(2.0),
            h_bytes,
            r: Some(0.5),
            channel_entropies: vec![],
            source: "test".to_string(),
        }
    }

    #[test]
    fn test_engine_creation() {
        let config = create_test_config();
        let engine = ComplexityEngine::new(config);

        assert!(engine.is_enabled());
        assert!(!engine.is_baseline_locked());
        assert_eq!(engine.snapshot_count(), 0);
    }

    #[test]
    fn test_engine_disabled() {
        let config = ComplexityConfig::default(); // disabled by default
        let mut engine = ComplexityEngine::new(config);

        let input = create_input(1000, 3.0);
        let result = engine.process(&input);

        assert!(result.is_none());
    }

    #[test]
    fn test_baseline_building_phase() {
        let config = create_test_config();
        let mut engine = ComplexityEngine::new(config);

        // First snapshot - baseline building
        let input = create_input(1000, 3.0);
        let result = engine.process(&input).unwrap();

        assert!(!result.is_baseline_locked());
        assert!(result.flags.contains(&"BASELINE_BUILDING".to_string()));
    }

    #[test]
    fn test_baseline_locks() {
        let config = create_test_config();
        let mut engine = ComplexityEngine::new(config);

        // Process enough snapshots to lock baseline
        engine.process(&create_input(1000, 3.0));
        let result = engine.process(&create_input(2000, 3.1)).unwrap();

        assert!(result.is_baseline_locked());
        assert!(engine.is_baseline_locked());
    }

    #[test]
    fn test_deltas_after_baseline() {
        let config = create_test_config();
        let mut engine = ComplexityEngine::new(config);

        // Build baseline
        engine.process(&create_input(1000, 3.0));
        engine.process(&create_input(2000, 3.0));

        // Now baseline is locked, next input should have deltas
        let result = engine.process(&create_input(3000, 3.5)).unwrap();

        assert!(result.deltas.is_some());
        assert!(result.z_scores.is_some());
    }

    #[test]
    fn test_anomaly_detection() {
        let config = create_test_config();
        let mut engine = ComplexityEngine::new(config);

        // Build baseline with h_bytes around 3.0
        engine.process(&create_input(1000, 3.0));
        engine.process(&create_input(2000, 3.0));

        // Now send a spike
        let result = engine.process(&create_input(3000, 10.0)).unwrap();

        // Check z-scores computed
        assert!(result.z_scores.is_some());
    }

    #[test]
    fn test_structure_extraction() {
        let config = create_test_config();
        let mut engine = ComplexityEngine::new(config);

        // Build baseline
        engine.process(&create_input(1000, 3.0));
        engine.process(&create_input(2000, 3.0));

        // Input with channel data
        let mut input = create_input(3000, 3.0);
        input.channel_entropies = vec![
            ChannelEntropy {
                channel_id: "ch1".to_string(),
                h: 1.0,
            },
            ChannelEntropy {
                channel_id: "ch2".to_string(),
                h: 1.1,
            },
            ChannelEntropy {
                channel_id: "ch3".to_string(),
                h: 1.2,
            },
        ];

        let result = engine.process(&input).unwrap();
        assert!(result.s_lite.is_some());
    }

    #[test]
    fn test_reset() {
        let config = create_test_config();
        let mut engine = ComplexityEngine::new(config);

        // Process some data
        engine.process(&create_input(1000, 3.0));
        engine.process(&create_input(2000, 3.0));

        assert!(engine.is_baseline_locked());
        assert_eq!(engine.snapshot_count(), 2);

        // Reset
        engine.reset();

        assert!(!engine.is_baseline_locked());
        assert_eq!(engine.snapshot_count(), 0);
        assert!(engine.last_output().is_none());
    }

    #[test]
    fn test_baseline_export_import() {
        let config = create_test_config();
        let mut engine = ComplexityEngine::new(config.clone());

        // Build baseline
        engine.process(&create_input(1000, 3.0));
        engine.process(&create_input(2000, 3.1));

        assert!(engine.is_baseline_locked());

        // Export
        let exported = engine.export_baseline().unwrap();

        // Create new engine and import
        let mut engine2 = ComplexityEngine::new(config);
        engine2.import_baseline(&exported).unwrap();

        assert!(engine2.is_baseline_locked());
    }

    #[test]
    fn test_last_output() {
        let config = create_test_config();
        let mut engine = ComplexityEngine::new(config);

        assert!(engine.last_output().is_none());

        engine.process(&create_input(1000, 3.0));

        assert!(engine.last_output().is_some());
        assert_eq!(engine.last_output().unwrap().timestamp_ms, 1000);
    }

    #[test]
    fn test_snapshot_count() {
        let config = create_test_config();
        let mut engine = ComplexityEngine::new(config);

        assert_eq!(engine.snapshot_count(), 0);

        for i in 0..5 {
            engine.process(&create_input(i * 1000, 3.0));
        }

        assert_eq!(engine.snapshot_count(), 5);
    }

    #[test]
    fn test_criticality_shift_detection() {
        let mut config = create_test_config();
        config.anomaly.events.criticality_shift = true;
        let mut engine = ComplexityEngine::new(config);

        // Build baseline
        engine.process(&create_input(1000, 3.0));
        engine.process(&create_input(2000, 3.0));

        // First input with channels
        let mut input1 = create_input(3000, 3.0);
        input1.channel_entropies = vec![
            ChannelEntropy {
                channel_id: "ch1".to_string(),
                h: 3.0,
            },
            ChannelEntropy {
                channel_id: "ch2".to_string(),
                h: 2.0,
            },
            ChannelEntropy {
                channel_id: "ch3".to_string(),
                h: 1.0,
            },
        ];
        engine.process(&input1);

        // Second input with different criticality order
        let mut input2 = create_input(4000, 3.0);
        input2.channel_entropies = vec![
            ChannelEntropy {
                channel_id: "ch1".to_string(),
                h: 1.0,
            },
            ChannelEntropy {
                channel_id: "ch2".to_string(),
                h: 3.0,
            },
            ChannelEntropy {
                channel_id: "ch3".to_string(),
                h: 2.0,
            },
        ];
        let result = engine.process(&input2).unwrap();

        // Should have criticality shift event
        let has_shift = result
            .events
            .iter()
            .any(|e| e.event_type == EventType::CriticalityShift);
        assert!(has_shift);
    }

    #[test]
    fn test_flags_include_anomaly_enabled() {
        let config = create_test_config();
        let mut engine = ComplexityEngine::new(config);

        // Build baseline
        engine.process(&create_input(1000, 3.0));
        engine.process(&create_input(2000, 3.0));

        let result = engine.process(&create_input(3000, 3.0)).unwrap();

        assert!(result
            .flags
            .contains(&"ANOMALY_DETECTION_ENABLED".to_string()));
        assert!(result.flags.contains(&"BASELINE_LOCKED".to_string()));
    }
}
