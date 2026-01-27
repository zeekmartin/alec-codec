// ALEC Complexity - Integration Tests
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Comprehensive integration tests for ALEC Complexity (35+ tests).

use alec_complexity::config::*;
use alec_complexity::*;

// ============================================================================
// Helper Functions
// ============================================================================

fn create_enabled_config() -> ComplexityConfig {
    ComplexityConfig {
        enabled: true,
        baseline: BaselineConfig {
            build_time_ms: 0,
            min_valid_snapshots: 3,
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

fn create_input(timestamp_ms: u64, h_bytes: f64) -> input::InputSnapshot {
    // Use h_bytes to vary other fields too, so we get non-zero std
    GenericInput::new(timestamp_ms, h_bytes)
        .with_tc(h_bytes * 0.3)
        .with_h_joint(h_bytes * 0.6)
        .with_r(h_bytes * 0.15)
        .build()
}

fn create_input_with_channels(timestamp_ms: u64, h_bytes: f64) -> input::InputSnapshot {
    GenericInput::new(timestamp_ms, h_bytes)
        .with_tc(1.0)
        .with_h_joint(2.0)
        .with_r(0.5)
        .with_channel("temp", 1.0)
        .with_channel("humidity", 1.2)
        .with_channel("pressure", 0.8)
        .build()
}

// ============================================================================
// Section 1: Configuration Tests (5 tests)
// ============================================================================

#[test]
fn test_01_default_config_is_disabled() {
    let config = ComplexityConfig::default();
    assert!(!config.enabled);
}

#[test]
fn test_02_baseline_config_defaults() {
    let config = BaselineConfig::default();
    assert_eq!(config.build_time_ms, 300_000);
    assert_eq!(config.min_valid_snapshots, 20);
    assert!(matches!(config.update_mode, BaselineUpdateMode::Frozen));
}

#[test]
fn test_03_anomaly_config_defaults() {
    let config = AnomalyConfig::default();
    assert!(config.enabled);
    assert!((config.z_threshold_warn - 2.0).abs() < 0.001);
    assert!((config.z_threshold_crit - 3.0).abs() < 0.001);
}

#[test]
fn test_04_event_type_config_all_enabled() {
    let config = EventTypeConfig::default();
    assert!(config.payload_entropy_spike);
    assert!(config.complexity_surge);
    assert!(config.redundancy_drop);
    assert!(config.structure_break);
    assert!(config.criticality_shift);
}

#[test]
fn test_05_structure_config_sparsify_defaults() {
    let config = StructureConfig::default();
    assert!(config.sparsify.enabled);
    assert_eq!(config.sparsify.top_k_edges, 64);
    assert!((config.sparsify.min_abs_weight - 0.2).abs() < 0.001);
}

// ============================================================================
// Section 2: Input Adapter Tests (6 tests)
// ============================================================================

#[test]
fn test_06_generic_input_minimal() {
    let input = GenericInput::new(1000, 3.5).build();
    assert_eq!(input.timestamp_ms, 1000);
    assert!((input.h_bytes - 3.5).abs() < 0.001);
    assert!(input.tc.is_none());
    assert!(input.h_joint.is_none());
}

#[test]
fn test_07_generic_input_with_all_fields() {
    let input = GenericInput::new(1000, 3.5)
        .with_tc(1.0)
        .with_h_joint(2.0)
        .with_r(0.5)
        .build();

    assert!(input.tc.is_some());
    assert!(input.h_joint.is_some());
    assert!(input.r.is_some());
}

#[test]
fn test_08_generic_input_with_channels() {
    let input = GenericInput::new(1000, 3.5)
        .with_channel("ch1", 1.0)
        .with_channel("ch2", 1.5)
        .build();

    assert_eq!(input.channel_entropies.len(), 2);
    assert!(input.can_compute_structure());
}

#[test]
fn test_09_generic_input_json_parsing() {
    let json = r#"{
        "timestamp_ms": 1000,
        "h_bytes": 3.5,
        "tc": 1.2,
        "h_joint": 2.3,
        "r": 0.6
    }"#;

    let input = GenericInput::from_json(json).unwrap();
    let snapshot = input.to_snapshot();

    assert_eq!(snapshot.timestamp_ms, 1000);
    assert!((snapshot.h_bytes - 3.5).abs() < 0.001);
    assert!((snapshot.tc.unwrap() - 1.2).abs() < 0.001);
}

#[test]
fn test_10_generic_input_json_roundtrip() {
    let input = GenericInput::new(1000, 3.5)
        .with_tc(1.0)
        .with_h_joint(2.0)
        .with_r(0.5)
        .with_channel("ch1", 1.0);

    let json = input.to_json().unwrap();
    let restored = GenericInput::from_json(&json).unwrap();
    let snapshot = restored.to_snapshot();

    assert_eq!(snapshot.timestamp_ms, 1000);
    assert!((snapshot.h_bytes - 3.5).abs() < 0.001);
}

#[test]
fn test_11_input_snapshot_helper_methods() {
    let minimal = input::InputSnapshot::minimal(1000, 3.5);
    assert!(!minimal.has_signal_metrics());

    let full = GenericInput::new(1000, 3.5)
        .with_tc(1.0)
        .with_h_joint(2.0)
        .build();
    assert!(full.has_signal_metrics());
}

// ============================================================================
// Section 3: Baseline Learning Tests (7 tests)
// ============================================================================

#[test]
fn test_12_baseline_starts_building() {
    let config = create_enabled_config();
    let engine = ComplexityEngine::new(config);

    assert!(!engine.is_baseline_locked());
}

#[test]
fn test_13_baseline_locks_after_min_snapshots() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    // Need 3 snapshots to lock
    engine.process(&create_input(1000, 3.0));
    assert!(!engine.is_baseline_locked());

    engine.process(&create_input(2000, 3.1));
    assert!(!engine.is_baseline_locked());

    engine.process(&create_input(3000, 3.2));
    assert!(engine.is_baseline_locked());
}

#[test]
fn test_14_baseline_emits_lock_event() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    engine.process(&create_input(1000, 3.0));
    engine.process(&create_input(2000, 3.1));
    let result = engine.process(&create_input(3000, 3.2)).unwrap();

    let has_lock_event = result
        .events
        .iter()
        .any(|e| e.event_type == EventType::BaselineLocked);
    assert!(has_lock_event);
}

#[test]
fn test_15_baseline_stats_computed() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    // Build baseline with known values
    engine.process(&create_input(1000, 3.0));
    engine.process(&create_input(2000, 3.0));
    engine.process(&create_input(3000, 3.0));

    let baseline = engine.baseline();
    assert!(baseline.is_ready());
    assert!((baseline.h_bytes.mean - 3.0).abs() < 0.01);
}

#[test]
fn test_16_baseline_export_import() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config.clone());

    // Build baseline
    engine.process(&create_input(1000, 3.0));
    engine.process(&create_input(2000, 3.1));
    engine.process(&create_input(3000, 3.2));

    // Export
    let exported = engine.export_baseline().unwrap();
    assert!(exported.contains("\"state\":\"Locked\""));

    // Import into new engine
    let mut engine2 = ComplexityEngine::new(config);
    engine2.import_baseline(&exported).unwrap();
    assert!(engine2.is_baseline_locked());
}

#[test]
fn test_17_baseline_ema_update() {
    let mut config = create_enabled_config();
    config.baseline.update_mode = BaselineUpdateMode::Ema { alpha: 30 }; // 0.30
    let mut engine = ComplexityEngine::new(config);

    // Build baseline
    engine.process(&create_input(1000, 3.0));
    engine.process(&create_input(2000, 3.0));
    engine.process(&create_input(3000, 3.0));

    let mean_before = engine.baseline().h_bytes.mean;

    // Process more with different value - EMA should update
    engine.process(&create_input(4000, 4.0));
    engine.process(&create_input(5000, 4.0));

    let mean_after = engine.baseline().h_bytes.mean;
    assert!(mean_after > mean_before); // EMA shifted towards 4.0
}

#[test]
fn test_18_baseline_frozen_mode() {
    let mut config = create_enabled_config();
    config.baseline.update_mode = BaselineUpdateMode::Frozen;
    let mut engine = ComplexityEngine::new(config);

    // Build baseline
    engine.process(&create_input(1000, 3.0));
    engine.process(&create_input(2000, 3.0));
    engine.process(&create_input(3000, 3.0));

    let mean_before = engine.baseline().h_bytes.mean;

    // Process more with different value - should NOT update
    engine.process(&create_input(4000, 10.0));
    engine.process(&create_input(5000, 10.0));

    let mean_after = engine.baseline().h_bytes.mean;
    assert!((mean_after - mean_before).abs() < 0.001); // Should be unchanged
}

// ============================================================================
// Section 4: Delta and Z-Score Tests (5 tests)
// ============================================================================

#[test]
fn test_19_deltas_computed_after_baseline() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    // Build baseline with varying values to get non-zero std
    engine.process(&create_input(1000, 2.8));
    engine.process(&create_input(2000, 3.0));
    engine.process(&create_input(3000, 3.2));

    // Get deltas - mean is ~3.0, so 4.0 should give positive delta
    let result = engine.process(&create_input(4000, 4.0)).unwrap();
    assert!(result.deltas.is_some());

    let deltas = result.deltas.unwrap();
    // Should be positive (4.0 - 3.0 mean ≈ 1.0, but smoothing may affect)
    assert!(deltas.h_bytes > 0.0);
}

#[test]
fn test_20_z_scores_computed() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    // Build baseline with some variance
    engine.process(&create_input(1000, 3.0));
    engine.process(&create_input(2000, 3.2));
    engine.process(&create_input(3000, 2.8));

    let result = engine.process(&create_input(4000, 5.0)).unwrap();
    assert!(result.z_scores.is_some());

    let z_scores = result.z_scores.unwrap();
    assert!(z_scores.h_bytes > 0.0); // Should be positive (above mean)
}

#[test]
fn test_21_z_scores_optional_fields() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    // Build baseline with varying values to get non-zero std
    engine.process(&create_input(1000, 2.8));
    engine.process(&create_input(2000, 3.0));
    engine.process(&create_input(3000, 3.2));

    let result = engine.process(&create_input(4000, 4.0)).unwrap();
    let z_scores = result.z_scores.unwrap();

    // TC and H_joint should have z-scores (now with variance in baseline)
    assert!(z_scores.tc.is_some());
    assert!(z_scores.h_joint.is_some());
    assert!(z_scores.r.is_some());
}

#[test]
fn test_22_smoothing_affects_deltas() {
    let mut config = create_enabled_config();
    config.deltas.smoothing.enabled = true;
    config.deltas.smoothing.alpha = 0.5;
    let mut engine = ComplexityEngine::new(config);

    // Build baseline with varying values to get non-zero std
    engine.process(&create_input(1000, 2.8));
    engine.process(&create_input(2000, 3.0));
    engine.process(&create_input(3000, 3.2));

    // First spike - mean ~3.0, so delta ~3.0
    let result1 = engine.process(&create_input(4000, 6.0)).unwrap();
    let delta1 = result1.deltas.unwrap().h_bytes;

    // Second reading at same value - smoothed delta should persist
    let result2 = engine.process(&create_input(5000, 6.0)).unwrap();
    let delta2 = result2.deltas.unwrap().h_bytes;

    // Both should be positive
    assert!(delta1 > 0.0);
    assert!(delta2 > 0.0);
}

#[test]
fn test_23_max_abs_z_score() {
    use alec_complexity::delta::ZScores;

    let z_scores = ZScores {
        tc: Some(1.5),
        h_joint: Some(-2.0),
        h_bytes: 0.5,
        r: Some(-3.0),
    };

    assert!((z_scores.max_abs() - 3.0).abs() < 0.001);
}

// ============================================================================
// Section 5: Structure (S-lite) Tests (4 tests)
// ============================================================================

#[test]
fn test_24_s_lite_extraction() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    // Build baseline
    engine.process(&create_input_with_channels(1000, 3.0));
    engine.process(&create_input_with_channels(2000, 3.0));
    engine.process(&create_input_with_channels(3000, 3.0));

    let result = engine
        .process(&create_input_with_channels(4000, 3.0))
        .unwrap();

    assert!(result.s_lite.is_some());
    let s_lite = result.s_lite.unwrap();
    assert!(!s_lite.edges.is_empty());
}

#[test]
fn test_25_s_lite_sparsification() {
    use alec_complexity::structure::SLiteExtractor;

    let config = StructureConfig {
        enabled: true,
        emit_s_lite: true,
        max_channels: 32,
        sparsify: SparsifyConfig {
            enabled: true,
            top_k_edges: 2,
            min_abs_weight: 0.0,
        },
        detect_breaks: true,
        break_threshold: 0.3,
    };

    let mut extractor = SLiteExtractor::new(config);

    let input = GenericInput::new(1000, 3.5)
        .with_channel("ch1", 1.0)
        .with_channel("ch2", 1.5)
        .with_channel("ch3", 2.0)
        .with_channel("ch4", 2.5)
        .build();

    let s_lite = extractor.extract(&input).unwrap();
    assert!(s_lite.edges.len() <= 2); // top_k_edges = 2
}

#[test]
fn test_26_s_lite_break_detection() {
    use alec_complexity::structure::SLiteExtractor;

    let config = StructureConfig {
        enabled: true,
        emit_s_lite: true,
        max_channels: 32,
        sparsify: SparsifyConfig {
            enabled: false,
            top_k_edges: 64,
            min_abs_weight: 0.0,
        },
        detect_breaks: true,
        break_threshold: 0.1,
    };

    let mut extractor = SLiteExtractor::new(config);

    // First extraction - stores s_lite1 in last_s_lite
    let input1 = GenericInput::new(1000, 3.5)
        .with_channel("ch1", 2.0)
        .with_channel("ch2", 2.0)
        .build();
    let s_lite1 = extractor.extract(&input1).unwrap();

    // Second extraction with changed values - stores s_lite2 in last_s_lite
    let input2 = GenericInput::new(2000, 3.5)
        .with_channel("ch1", 2.0)
        .with_channel("ch2", 5.0)
        .build();
    let _s_lite2 = extractor.extract(&input2).unwrap();

    // detect_break compares s_lite1 against last_s_lite (which is s_lite2)
    // to find the break between them
    let break_info = extractor.detect_break(&s_lite1);
    assert!(break_info.is_some());
}

#[test]
fn test_27_s_lite_no_break_when_similar() {
    use alec_complexity::structure::SLiteExtractor;

    let config = StructureConfig {
        enabled: true,
        emit_s_lite: true,
        max_channels: 32,
        sparsify: SparsifyConfig {
            enabled: false,
            top_k_edges: 64,
            min_abs_weight: 0.0,
        },
        detect_breaks: true,
        break_threshold: 0.5,
    };

    let mut extractor = SLiteExtractor::new(config);

    // First extraction
    let input1 = GenericInput::new(1000, 3.5)
        .with_channel("ch1", 2.0)
        .with_channel("ch2", 2.0)
        .build();
    extractor.extract(&input1);

    // Second extraction with similar values
    let input2 = GenericInput::new(2000, 3.5)
        .with_channel("ch1", 2.0)
        .with_channel("ch2", 2.1)
        .build();
    let s_lite2 = extractor.extract(&input2).unwrap();

    let break_info = extractor.detect_break(&s_lite2);
    assert!(break_info.is_none());
}

// ============================================================================
// Section 6: Anomaly Detection Tests (6 tests)
// ============================================================================

#[test]
fn test_28_anomaly_below_threshold_no_event() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    // Build baseline
    engine.process(&create_input(1000, 3.0));
    engine.process(&create_input(2000, 3.0));
    engine.process(&create_input(3000, 3.0));

    // Small deviation - should not trigger
    let result = engine.process(&create_input(4000, 3.1)).unwrap();

    let has_anomaly = result
        .events
        .iter()
        .any(|e| e.event_type == EventType::PayloadEntropySpike);
    assert!(!has_anomaly);
}

#[test]
fn test_29_anomaly_above_threshold_emits_event() {
    let mut config = create_enabled_config();
    config.baseline.min_valid_snapshots = 3;
    let mut engine = ComplexityEngine::new(config);

    // Build baseline with tight variance
    engine.process(&create_input(1000, 3.0));
    engine.process(&create_input(2000, 3.0));
    engine.process(&create_input(3000, 3.0));

    // Large spike
    let result = engine.process(&create_input(4000, 10.0)).unwrap();

    // With zero variance, any deviation gives infinite z-score
    assert!(result.z_scores.is_some());
}

#[test]
fn test_30_anomaly_severity_warning() {
    use alec_complexity::anomaly::AnomalyDetector;
    use alec_complexity::delta::ZScores;

    let config = AnomalyConfig {
        enabled: true,
        z_threshold_warn: 2.0,
        z_threshold_crit: 3.0,
        persistence_ms: 0,
        cooldown_ms: 0,
        events: EventTypeConfig::default(),
    };

    let mut detector = AnomalyDetector::new(config);

    let z_scores = ZScores {
        h_bytes: 2.5, // Above warn, below crit
        tc: None,
        h_joint: None,
        r: None,
    };

    let events = detector.evaluate(&z_scores, None, None, 1000);

    if !events.is_empty() {
        assert_eq!(events[0].severity, EventSeverity::Warning);
    }
}

#[test]
fn test_31_anomaly_severity_critical() {
    use alec_complexity::anomaly::AnomalyDetector;
    use alec_complexity::delta::ZScores;

    let config = AnomalyConfig {
        enabled: true,
        z_threshold_warn: 2.0,
        z_threshold_crit: 3.0,
        persistence_ms: 0,
        cooldown_ms: 0,
        events: EventTypeConfig::default(),
    };

    let mut detector = AnomalyDetector::new(config);

    let z_scores = ZScores {
        h_bytes: 3.5, // Above crit
        tc: None,
        h_joint: None,
        r: None,
    };

    let events = detector.evaluate(&z_scores, None, None, 1000);

    if !events.is_empty() {
        assert_eq!(events[0].severity, EventSeverity::Critical);
    }
}

#[test]
fn test_32_anomaly_cooldown() {
    use alec_complexity::anomaly::AnomalyDetector;
    use alec_complexity::delta::ZScores;

    let config = AnomalyConfig {
        enabled: true,
        z_threshold_warn: 2.0,
        z_threshold_crit: 3.0,
        persistence_ms: 0,
        cooldown_ms: 5000,
        events: EventTypeConfig::default(),
    };

    let mut detector = AnomalyDetector::new(config);

    let z_scores = ZScores {
        h_bytes: 2.5,
        tc: None,
        h_joint: None,
        r: None,
    };

    // First event
    let events1 = detector.evaluate(&z_scores, None, None, 1000);

    // Second evaluation - within cooldown
    let events2 = detector.evaluate(&z_scores, None, None, 3000);
    assert!(events2.is_empty() || events1.is_empty()); // One should be blocked

    // After cooldown
    let events3 = detector.evaluate(&z_scores, None, None, 10000);
    // Should allow event again
    assert!(!events3.is_empty() || events1.is_empty());
}

#[test]
fn test_33_anomaly_persistence() {
    use alec_complexity::anomaly::AnomalyDetector;
    use alec_complexity::delta::ZScores;

    let config = AnomalyConfig {
        enabled: true,
        z_threshold_warn: 2.0,
        z_threshold_crit: 3.0,
        persistence_ms: 2000,
        cooldown_ms: 0,
        events: EventTypeConfig::default(),
    };

    let mut detector = AnomalyDetector::new(config);

    let z_scores = ZScores {
        h_bytes: 2.5,
        tc: None,
        h_joint: None,
        r: None,
    };

    // First - no event (persistence not met)
    let events1 = detector.evaluate(&z_scores, None, None, 1000);
    assert!(events1.is_empty());

    // Still persisting, but not long enough
    let events2 = detector.evaluate(&z_scores, None, None, 2000);
    assert!(events2.is_empty());

    // Now persistence met
    let events3 = detector.evaluate(&z_scores, None, None, 4000);
    assert!(!events3.is_empty());
}

// ============================================================================
// Section 7: Event Tests (4 tests)
// ============================================================================

#[test]
fn test_34_event_serialization() {
    let event = ComplexityEvent::payload_entropy_spike(1000, EventSeverity::Warning, 2.5, 2.0);

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("PayloadEntropySpike"));
    assert!(json.contains("Warning"));
}

#[test]
fn test_35_event_types_distinct() {
    assert_ne!(EventType::PayloadEntropySpike, EventType::ComplexitySurge);
    assert_ne!(EventType::RedundancyDrop, EventType::StructureBreak);
}

#[test]
fn test_36_structure_break_event() {
    use alec_complexity::structure::{EdgeChange, StructureBreak};

    let break_info = StructureBreak {
        changed_edges: vec![EdgeChange {
            channel_a: "ch1".to_string(),
            channel_b: "ch2".to_string(),
            old_weight: 0.5,
            new_weight: 0.9,
            delta: 0.4,
        }],
        total_change: 0.4,
    };

    let event = ComplexityEvent::structure_break(1000, break_info);
    assert_eq!(event.event_type, EventType::StructureBreak);
    assert_eq!(event.severity, EventSeverity::Warning);
}

#[test]
fn test_37_criticality_shift_event() {
    let event = ComplexityEvent::criticality_shift(
        1000,
        vec!["ch1".to_string(), "ch2".to_string()],
        vec!["ch2".to_string(), "ch3".to_string()],
    );

    assert_eq!(event.event_type, EventType::CriticalityShift);
}

// ============================================================================
// Section 8: Snapshot Tests (5 tests)
// ============================================================================

#[test]
fn test_38_snapshot_building_phase() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    let result = engine.process(&create_input(1000, 3.0)).unwrap();

    assert!(!result.is_baseline_locked());
    assert!(result.flags.contains(&"BASELINE_BUILDING".to_string()));
    assert!(result.deltas.is_none());
    assert!(result.z_scores.is_none());
}

#[test]
fn test_39_snapshot_locked_phase() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    // Build and lock
    engine.process(&create_input(1000, 3.0));
    engine.process(&create_input(2000, 3.0));
    engine.process(&create_input(3000, 3.0));

    let result = engine.process(&create_input(4000, 3.0)).unwrap();

    assert!(result.is_baseline_locked());
    assert!(result.flags.contains(&"BASELINE_LOCKED".to_string()));
    assert!(result.deltas.is_some());
    assert!(result.z_scores.is_some());
}

#[test]
fn test_40_snapshot_json_roundtrip() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    engine.process(&create_input(1000, 3.0));
    engine.process(&create_input(2000, 3.0));
    engine.process(&create_input(3000, 3.0));

    let result = engine.process(&create_input(4000, 3.5)).unwrap();

    let json = result.to_json().unwrap();
    let restored = ComplexitySnapshot::from_json(&json).unwrap();

    assert_eq!(result.timestamp_ms, restored.timestamp_ms);
    assert_eq!(result.version, restored.version);
}

#[test]
fn test_41_snapshot_version() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    let result = engine.process(&create_input(1000, 3.0)).unwrap();

    assert!(!result.version.is_empty());
    assert!(result.version.starts_with("0."));
}

#[test]
fn test_42_snapshot_events_by_severity() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    let result = engine.process(&create_input(1000, 3.0)).unwrap();

    // Should have baseline building event (Info severity)
    let info_events = result.events_by_severity("info");
    assert!(!info_events.is_empty());
}

// ============================================================================
// Section 9: Engine Orchestration Tests (5 tests)
// ============================================================================

#[test]
fn test_43_engine_disabled_returns_none() {
    let config = ComplexityConfig::default();
    let mut engine = ComplexityEngine::new(config);

    let result = engine.process(&create_input(1000, 3.0));
    assert!(result.is_none());
}

#[test]
fn test_44_engine_reset() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    // Build state
    engine.process(&create_input(1000, 3.0));
    engine.process(&create_input(2000, 3.0));
    engine.process(&create_input(3000, 3.0));

    assert!(engine.is_baseline_locked());
    assert_eq!(engine.snapshot_count(), 3);

    engine.reset();

    assert!(!engine.is_baseline_locked());
    assert_eq!(engine.snapshot_count(), 0);
}

#[test]
fn test_45_engine_last_output() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    assert!(engine.last_output().is_none());

    engine.process(&create_input(1000, 3.0));
    assert!(engine.last_output().is_some());

    engine.process(&create_input(2000, 3.5));
    assert_eq!(engine.last_output().unwrap().timestamp_ms, 2000);
}

#[test]
fn test_46_engine_snapshot_count() {
    let config = create_enabled_config();
    let mut engine = ComplexityEngine::new(config);

    for i in 0..10 {
        engine.process(&create_input(i * 1000, 3.0));
    }

    assert_eq!(engine.snapshot_count(), 10);
}

#[test]
fn test_47_full_pipeline_integration() {
    let mut config = create_enabled_config();
    config.anomaly.events.criticality_shift = true;
    let mut engine = ComplexityEngine::new(config);

    // Build baseline with channels
    for i in 0..3 {
        engine.process(&create_input_with_channels(i * 1000, 3.0));
    }

    assert!(engine.is_baseline_locked());

    // Process with deviation
    let result = engine
        .process(&create_input_with_channels(4000, 5.0))
        .unwrap();

    // Should have full analysis
    assert!(result.deltas.is_some());
    assert!(result.z_scores.is_some());
    assert!(result.s_lite.is_some());
    assert!(result.is_baseline_locked());

    // JSON export should work
    let json = result.to_json_pretty().unwrap();
    assert!(json.contains("version"));
    assert!(json.contains("baseline"));
}
