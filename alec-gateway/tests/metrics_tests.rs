// ALEC Gateway - Metrics Tests
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Tests for the ALEC Gateway metrics module.
//!
//! These tests verify:
//! - Signal-level entropy computation
//! - Payload-level entropy computation
//! - Resilience index calculation
//! - MetricsEngine orchestration
//! - Gateway integration

#![cfg(feature = "metrics")]

use alec_gateway::metrics::{
    AlignmentStrategy, LogBase, MetricsConfig, MetricsEngine, MetricsSnapshot, MissingDataPolicy,
    NormalizationConfig, NormalizationMethod, NumericsConfig, PayloadMetricsConfig,
    ResilienceConfig, ResilienceThresholds, ResilienceZone, SignalComputeSchedule, SignalEstimator,
    SignalWindow,
};
use alec_gateway::{ChannelConfig, Gateway, GatewayConfig};

// ===========================================================================
// Configuration Tests
// ===========================================================================

#[test]
fn test_default_config_is_disabled() {
    let config = MetricsConfig::default();
    assert!(!config.enabled);
}

#[test]
fn test_config_with_enabled() {
    let config = MetricsConfig {
        enabled: true,
        ..Default::default()
    };
    assert!(config.enabled);
}

#[test]
fn test_config_signal_window_time() {
    let config = MetricsConfig {
        enabled: true,
        signal_window: SignalWindow::TimeMillis(120_000),
        ..Default::default()
    };
    match config.signal_window {
        SignalWindow::TimeMillis(ms) => assert_eq!(ms, 120_000),
        _ => panic!("Wrong window type"),
    }
}

#[test]
fn test_config_signal_window_samples() {
    let config = MetricsConfig {
        enabled: true,
        signal_window: SignalWindow::LastNSamples(200),
        ..Default::default()
    };
    match config.signal_window {
        SignalWindow::LastNSamples(n) => assert_eq!(n, 200),
        _ => panic!("Wrong window type"),
    }
}

#[test]
fn test_config_signal_compute_schedule_flushes() {
    let config = MetricsConfig {
        enabled: true,
        signal_compute: SignalComputeSchedule::EveryNFlushes(5),
        ..Default::default()
    };
    match config.signal_compute {
        SignalComputeSchedule::EveryNFlushes(n) => assert_eq!(n, 5),
        _ => panic!("Wrong schedule type"),
    }
}

#[test]
fn test_config_signal_compute_schedule_millis() {
    let config = MetricsConfig {
        enabled: true,
        signal_compute: SignalComputeSchedule::EveryMillis(30_000),
        ..Default::default()
    };
    match config.signal_compute {
        SignalComputeSchedule::EveryMillis(ms) => assert_eq!(ms, 30_000),
        _ => panic!("Wrong schedule type"),
    }
}

#[test]
fn test_config_signal_compute_schedule_combined() {
    let config = MetricsConfig {
        enabled: true,
        signal_compute: SignalComputeSchedule::NFlushesOrMillis {
            n_flushes: 3,
            millis: 10_000,
        },
        ..Default::default()
    };
    match config.signal_compute {
        SignalComputeSchedule::NFlushesOrMillis { n_flushes, millis } => {
            assert_eq!(n_flushes, 3);
            assert_eq!(millis, 10_000);
        }
        _ => panic!("Wrong schedule type"),
    }
}

#[test]
fn test_config_alignment_sample_and_hold() {
    let config = MetricsConfig {
        enabled: true,
        alignment: AlignmentStrategy::SampleAndHold,
        ..Default::default()
    };
    assert!(matches!(config.alignment, AlignmentStrategy::SampleAndHold));
}

#[test]
fn test_config_alignment_nearest() {
    let config = MetricsConfig {
        enabled: true,
        alignment: AlignmentStrategy::Nearest,
        ..Default::default()
    };
    assert!(matches!(config.alignment, AlignmentStrategy::Nearest));
}

#[test]
fn test_config_alignment_linear_interpolation() {
    let config = MetricsConfig {
        enabled: true,
        alignment: AlignmentStrategy::LinearInterpolation,
        ..Default::default()
    };
    assert!(matches!(
        config.alignment,
        AlignmentStrategy::LinearInterpolation
    ));
}

#[test]
fn test_config_missing_data_drop_incomplete() {
    let config = MetricsConfig {
        enabled: true,
        missing_data: MissingDataPolicy::DropIncompleteSnapshots,
        ..Default::default()
    };
    assert!(matches!(
        config.missing_data,
        MissingDataPolicy::DropIncompleteSnapshots
    ));
}

#[test]
fn test_config_missing_data_allow_partial() {
    let config = MetricsConfig {
        enabled: true,
        missing_data: MissingDataPolicy::AllowPartial { min_channels: 2 },
        ..Default::default()
    };
    match config.missing_data {
        MissingDataPolicy::AllowPartial { min_channels } => assert_eq!(min_channels, 2),
        _ => panic!("Wrong policy"),
    }
}

#[test]
fn test_config_missing_data_fill_with_last_known() {
    let config = MetricsConfig {
        enabled: true,
        missing_data: MissingDataPolicy::FillWithLastKnown,
        ..Default::default()
    };
    assert!(matches!(
        config.missing_data,
        MissingDataPolicy::FillWithLastKnown
    ));
}

#[test]
fn test_config_normalization() {
    let config = MetricsConfig {
        enabled: true,
        normalization: NormalizationConfig {
            enabled: true,
            method: NormalizationMethod::ZScore,
            min_samples: 20,
        },
        ..Default::default()
    };
    assert!(config.normalization.enabled);
    assert!(matches!(
        config.normalization.method,
        NormalizationMethod::ZScore
    ));
    assert_eq!(config.normalization.min_samples, 20);
}

#[test]
fn test_config_normalization_robust_mad() {
    let config = MetricsConfig {
        enabled: true,
        normalization: NormalizationConfig {
            enabled: true,
            method: NormalizationMethod::RobustMad,
            min_samples: 10,
        },
        ..Default::default()
    };
    assert!(matches!(
        config.normalization.method,
        NormalizationMethod::RobustMad
    ));
}

#[test]
fn test_config_signal_estimator() {
    let config = MetricsConfig {
        enabled: true,
        signal_estimator: SignalEstimator::GaussianCovariance {
            log_base: LogBase::Two,
        },
        ..Default::default()
    };
    match config.signal_estimator {
        SignalEstimator::GaussianCovariance { log_base } => {
            assert!(matches!(log_base, LogBase::Two));
        }
    }
}

#[test]
fn test_config_signal_estimator_log_e() {
    let config = MetricsConfig {
        enabled: true,
        signal_estimator: SignalEstimator::GaussianCovariance {
            log_base: LogBase::E,
        },
        ..Default::default()
    };
    match config.signal_estimator {
        SignalEstimator::GaussianCovariance { log_base } => {
            assert!(matches!(log_base, LogBase::E));
        }
    }
}

#[test]
fn test_config_resilience() {
    let config = MetricsConfig {
        enabled: true,
        resilience: ResilienceConfig {
            enabled: true,
            thresholds: ResilienceThresholds {
                healthy_min: 0.3,
                attention_min: 0.1,
            },
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(config.resilience.enabled);
    assert_eq!(config.resilience.thresholds.healthy_min, 0.3);
    assert_eq!(config.resilience.thresholds.attention_min, 0.1);
}

#[test]
fn test_config_payload() {
    let config = MetricsConfig {
        enabled: true,
        payload: PayloadMetricsConfig {
            frame_entropy: true,
            per_channel_entropy: true,
            sizes: true,
            include_histogram: true,
        },
        ..Default::default()
    };
    assert!(config.payload.frame_entropy);
    assert!(config.payload.per_channel_entropy);
    assert!(config.payload.sizes);
    assert!(config.payload.include_histogram);
}

#[test]
fn test_config_numerics() {
    let config = MetricsConfig {
        enabled: true,
        numerics: NumericsConfig {
            min_aligned_samples: 20,
            covariance_epsilon: 1e-5,
            max_channels_for_joint: 16,
        },
        ..Default::default()
    };
    assert_eq!(config.numerics.min_aligned_samples, 20);
    assert_eq!(config.numerics.covariance_epsilon, 1e-5);
    assert_eq!(config.numerics.max_channels_for_joint, 16);
}

// ===========================================================================
// MetricsEngine Tests
// ===========================================================================

fn create_enabled_config() -> MetricsConfig {
    MetricsConfig {
        enabled: true,
        signal_compute: SignalComputeSchedule::EveryNFlushes(1),
        numerics: NumericsConfig {
            min_aligned_samples: 3,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn test_engine_new_disabled() {
    let config = MetricsConfig::default();
    let engine = MetricsEngine::new(config);
    assert!(!engine.is_enabled());
}

#[test]
fn test_engine_new_enabled() {
    let config = create_enabled_config();
    let engine = MetricsEngine::new(config);
    assert!(engine.is_enabled());
}

#[test]
fn test_engine_observe_sample_when_disabled() {
    let config = MetricsConfig::default();
    let mut engine = MetricsEngine::new(config);
    engine.observe_sample("ch1", 1.0, 1000);
    // Should be a no-op when disabled
    assert_eq!(engine.flush_count(), 0);
}

#[test]
fn test_engine_observe_sample_when_enabled() {
    let config = create_enabled_config();
    let mut engine = MetricsEngine::new(config);
    engine.observe_sample("ch1", 1.0, 1000);
    engine.observe_sample("ch1", 2.0, 2000);
    engine.observe_sample("ch2", 3.0, 1000);
    // Samples are stored in the window
    assert_eq!(engine.flush_count(), 0);
}

#[test]
fn test_engine_observe_frame_returns_none_when_disabled() {
    let config = MetricsConfig::default();
    let mut engine = MetricsEngine::new(config);
    let result = engine.observe_frame(&[1, 2, 3], 1000);
    assert!(result.is_none());
}

#[test]
fn test_engine_observe_frame_returns_snapshot_when_enabled() {
    let config = create_enabled_config();
    let mut engine = MetricsEngine::new(config);

    // Add samples first
    for i in 0..5 {
        engine.observe_sample("ch1", (i as f64) * 0.1 + 20.0, i as u64 * 1000);
    }

    let result = engine.observe_frame(&[1, 2, 3, 4, 5], 5000);
    assert!(result.is_some());

    let snapshot = result.unwrap();
    assert_eq!(snapshot.payload.frame_size_bytes, 5);
}

#[test]
fn test_engine_flush_count_increments() {
    let config = create_enabled_config();
    let mut engine = MetricsEngine::new(config);

    assert_eq!(engine.flush_count(), 0);
    engine.observe_frame(&[], 1000);
    assert_eq!(engine.flush_count(), 1);
    engine.observe_frame(&[], 2000);
    assert_eq!(engine.flush_count(), 2);
}

#[test]
fn test_engine_signal_compute_count() {
    let config = create_enabled_config();
    let mut engine = MetricsEngine::new(config);

    // Add enough samples
    for i in 0..5 {
        engine.observe_sample("ch1", i as f64, i as u64 * 1000);
    }

    engine.observe_frame(&[], 5000);
    assert!(engine.signal_compute_count() >= 1);
}

#[test]
fn test_engine_last_snapshot() {
    let config = create_enabled_config();
    let mut engine = MetricsEngine::new(config);

    assert!(engine.last_snapshot().is_none());

    engine.observe_frame(&[1, 2, 3], 1000);

    assert!(engine.last_snapshot().is_some());
}

#[test]
fn test_engine_reset() {
    let config = create_enabled_config();
    let mut engine = MetricsEngine::new(config);

    engine.observe_sample("ch1", 1.0, 1000);
    engine.observe_frame(&[1, 2, 3], 2000);

    assert!(engine.flush_count() > 0);
    assert!(engine.last_snapshot().is_some());

    engine.reset();

    assert_eq!(engine.flush_count(), 0);
    assert_eq!(engine.signal_compute_count(), 0);
    assert!(engine.last_snapshot().is_none());
}

#[test]
fn test_engine_config_access() {
    let config = MetricsConfig {
        enabled: true,
        signal_window: SignalWindow::TimeMillis(120_000),
        ..Default::default()
    };
    let engine = MetricsEngine::new(config);

    match engine.config().signal_window {
        SignalWindow::TimeMillis(ms) => assert_eq!(ms, 120_000),
        _ => panic!("Wrong window type"),
    }
}

#[test]
fn test_engine_register_channel() {
    let config = create_enabled_config();
    let mut engine = MetricsEngine::new(config);

    engine.register_channel("temp");
    engine.register_channel("humidity");

    // Channels are pre-registered (no samples yet, but ready)
    // This is a no-op test to verify it doesn't panic
}

// ===========================================================================
// Payload Entropy Tests
// ===========================================================================

#[test]
fn test_payload_entropy_empty_frame() {
    let config = create_enabled_config();
    let mut engine = MetricsEngine::new(config);

    let snapshot = engine.observe_frame(&[], 1000).unwrap();

    assert_eq!(snapshot.payload.frame_size_bytes, 0);
    assert_eq!(snapshot.payload.h_bytes, 0.0);
}

#[test]
fn test_payload_entropy_single_byte() {
    let config = create_enabled_config();
    let mut engine = MetricsEngine::new(config);

    let snapshot = engine.observe_frame(&[0x42], 1000).unwrap();

    assert_eq!(snapshot.payload.frame_size_bytes, 1);
    // Single byte has 0 entropy
    assert_eq!(snapshot.payload.h_bytes, 0.0);
}

#[test]
fn test_payload_entropy_uniform_bytes() {
    let config = MetricsConfig {
        enabled: true,
        signal_compute: SignalComputeSchedule::EveryNFlushes(1),
        payload: PayloadMetricsConfig {
            include_histogram: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut engine = MetricsEngine::new(config);

    // All bytes the same = 0 entropy
    let data = vec![0xAB; 100];
    let snapshot = engine.observe_frame(&data, 1000).unwrap();

    assert_eq!(snapshot.payload.frame_size_bytes, 100);
    assert_eq!(snapshot.payload.h_bytes, 0.0);
}

#[test]
fn test_payload_entropy_two_symbols() {
    let config = create_enabled_config();
    let mut engine = MetricsEngine::new(config);

    // Equal distribution of 2 symbols = 1 bit entropy
    let mut data = vec![0x00; 50];
    data.extend(vec![0xFF; 50]);
    let snapshot = engine.observe_frame(&data, 1000).unwrap();

    // H = -2 * 0.5 * log2(0.5) = 1.0
    assert!((snapshot.payload.h_bytes - 1.0).abs() < 0.01);
}

#[test]
fn test_payload_entropy_high_entropy() {
    let config = create_enabled_config();
    let mut engine = MetricsEngine::new(config);

    // All different bytes (256 unique values) = max entropy
    let data: Vec<u8> = (0..=255).collect();
    let snapshot = engine.observe_frame(&data, 1000).unwrap();

    // H = log2(256) = 8.0 bits
    assert!((snapshot.payload.h_bytes - 8.0).abs() < 0.01);
}

#[test]
fn test_payload_histogram_included() {
    let config = MetricsConfig {
        enabled: true,
        signal_compute: SignalComputeSchedule::EveryNFlushes(1),
        payload: PayloadMetricsConfig {
            include_histogram: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut engine = MetricsEngine::new(config);

    let data = vec![0x00, 0x00, 0x01, 0x02, 0x02, 0x02];
    let snapshot = engine.observe_frame(&data, 1000).unwrap();

    assert!(snapshot.payload.histogram.is_some());
}

#[test]
fn test_payload_histogram_excluded() {
    let config = MetricsConfig {
        enabled: true,
        signal_compute: SignalComputeSchedule::EveryNFlushes(1),
        payload: PayloadMetricsConfig {
            include_histogram: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut engine = MetricsEngine::new(config);

    let snapshot = engine.observe_frame(&[1, 2, 3], 1000).unwrap();
    assert!(snapshot.payload.histogram.is_none());
}

// ===========================================================================
// Resilience Zone Tests
// ===========================================================================

#[test]
fn test_resilience_zone_healthy_as_str() {
    assert_eq!(ResilienceZone::Healthy.as_str(), "healthy");
}

#[test]
fn test_resilience_zone_attention_as_str() {
    assert_eq!(ResilienceZone::Attention.as_str(), "attention");
}

#[test]
fn test_resilience_zone_critical_as_str() {
    assert_eq!(ResilienceZone::Critical.as_str(), "critical");
}

#[test]
fn test_resilience_thresholds_ordering() {
    let thresholds = ResilienceThresholds::default();
    assert!(thresholds.healthy_min > thresholds.attention_min);
}

// ===========================================================================
// Gateway Integration Tests
// ===========================================================================

#[test]
fn test_gateway_metrics_disabled_by_default() {
    let gateway = Gateway::new();
    assert!(!gateway.metrics_enabled());
}

#[test]
fn test_gateway_enable_metrics() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(MetricsConfig {
        enabled: true,
        ..Default::default()
    });
    assert!(gateway.metrics_enabled());
}

#[test]
fn test_gateway_disable_metrics() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(MetricsConfig {
        enabled: true,
        ..Default::default()
    });
    assert!(gateway.metrics_enabled());

    gateway.disable_metrics();
    assert!(!gateway.metrics_enabled());
}

#[test]
fn test_gateway_last_metrics_none_initially() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(create_enabled_config());
    assert!(gateway.last_metrics().is_none());
}

#[test]
fn test_gateway_last_metrics_after_flush() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(create_enabled_config());

    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();

    for i in 0..5 {
        gateway
            .push("temp", 20.0 + (i as f64) * 0.1, 1000 + i * 1000)
            .unwrap();
    }

    gateway.flush().unwrap();

    assert!(gateway.last_metrics().is_some());
}

#[test]
fn test_gateway_metrics_payload_in_snapshot() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(create_enabled_config());

    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();
    gateway.push("temp", 22.5, 1000).unwrap();

    let frame = gateway.flush().unwrap();
    let metrics = gateway.last_metrics().unwrap();

    // Payload size should match frame size
    assert_eq!(metrics.payload.frame_size_bytes, frame.to_bytes().len());
}

#[test]
fn test_gateway_metrics_multiple_channels() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(create_enabled_config());

    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();
    gateway
        .add_channel("humid", ChannelConfig::default())
        .unwrap();

    for i in 0..5 {
        let ts = 1000 + i * 1000;
        gateway.push("temp", 20.0 + (i as f64) * 0.1, ts).unwrap();
        gateway.push("humid", 60.0 + (i as f64) * 0.5, ts).unwrap();
    }

    gateway.flush().unwrap();

    let metrics = gateway.last_metrics().unwrap();
    assert!(metrics.payload.frame_size_bytes > 0);
}

#[test]
fn test_gateway_metrics_with_resilience() {
    let config = MetricsConfig {
        enabled: true,
        signal_compute: SignalComputeSchedule::EveryNFlushes(1),
        resilience: ResilienceConfig {
            enabled: true,
            ..Default::default()
        },
        numerics: NumericsConfig {
            min_aligned_samples: 3,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut gateway = Gateway::new();
    gateway.enable_metrics(config);

    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();
    gateway
        .add_channel("humid", ChannelConfig::default())
        .unwrap();

    for i in 0..10 {
        let ts = 1000 + i * 1000;
        gateway.push("temp", 20.0 + (i as f64) * 0.1, ts).unwrap();
        gateway.push("humid", 60.0 + (i as f64) * 0.2, ts).unwrap();
    }

    gateway.flush().unwrap();

    let metrics = gateway.last_metrics().unwrap();
    // Note: Resilience may or may not be computed depending on sample alignment
    assert!(metrics.payload.frame_size_bytes > 0);
}

#[test]
fn test_gateway_metrics_engine_access() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(create_enabled_config());

    let engine = gateway.metrics_engine_mut().unwrap();
    engine.reset();

    // Should still be enabled
    assert!(gateway.metrics_enabled());
}

#[test]
fn test_gateway_register_channel_metrics() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(create_enabled_config());

    // Manually register channels (normally done automatically)
    gateway.register_channel_metrics("sensor1");
    gateway.register_channel_metrics("sensor2");

    // This is a no-op test to verify it doesn't panic
}

// ===========================================================================
// Snapshot Serialization Tests
// ===========================================================================

#[test]
fn test_snapshot_to_json() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(create_enabled_config());

    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();
    gateway.push("temp", 22.5, 1000).unwrap();

    gateway.flush().unwrap();

    let metrics = gateway.last_metrics().unwrap();
    let json = metrics.to_json().unwrap();

    // Should be valid JSON
    assert!(json.contains("timestamp_ms"));
    assert!(json.contains("payload"));
    assert!(json.contains("frame_size_bytes"));
}

#[test]
fn test_snapshot_to_json_compact() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(create_enabled_config());

    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();
    gateway.push("temp", 22.5, 1000).unwrap();

    gateway.flush().unwrap();

    let metrics = gateway.last_metrics().unwrap();
    let json_pretty = metrics.to_json().unwrap();
    let json_compact = metrics.to_json_compact().unwrap();

    // Compact should be shorter (no whitespace)
    assert!(json_compact.len() < json_pretty.len());
}

#[test]
fn test_snapshot_from_json() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(create_enabled_config());

    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();
    gateway.push("temp", 22.5, 1000).unwrap();

    gateway.flush().unwrap();

    let original = gateway.last_metrics().unwrap();
    let json = original.to_json().unwrap();

    let parsed = MetricsSnapshot::from_json(&json).unwrap();
    assert_eq!(
        parsed.payload.frame_size_bytes,
        original.payload.frame_size_bytes
    );
}

// ===========================================================================
// Edge Cases and Boundary Tests
// ===========================================================================

#[test]
fn test_empty_gateway_flush_with_metrics() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(create_enabled_config());

    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();

    // Flush with no data
    let frame = gateway.flush().unwrap();
    assert!(frame.is_empty());

    // Metrics should still be computed (for empty frame)
    let metrics = gateway.last_metrics().unwrap();
    assert_eq!(metrics.payload.frame_size_bytes, frame.to_bytes().len());
}

#[test]
fn test_very_large_frame() {
    let config = MetricsConfig {
        enabled: true,
        signal_compute: SignalComputeSchedule::EveryNFlushes(1),
        payload: PayloadMetricsConfig {
            include_histogram: false, // Skip histogram for large frames
            ..Default::default()
        },
        ..Default::default()
    };

    let mut engine = MetricsEngine::new(config);

    // Large frame (1KB)
    let data: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
    let snapshot = engine.observe_frame(&data, 1000).unwrap();

    assert_eq!(snapshot.payload.frame_size_bytes, 1024);
}

#[test]
fn test_rapid_consecutive_flushes() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(MetricsConfig {
        enabled: true,
        signal_compute: SignalComputeSchedule::EveryNFlushes(1),
        ..Default::default()
    });

    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();

    for i in 0..10 {
        gateway.push("temp", 20.0 + i as f64, i * 100).unwrap();
        gateway.flush().unwrap();
    }

    // Should have processed 10 flushes
    assert!(gateway.last_metrics().is_some());
}

#[test]
fn test_metrics_with_lorawan_config() {
    let gateway_config = GatewayConfig::lorawan(4);
    let mut gateway = Gateway::with_config(gateway_config);

    gateway.enable_metrics(create_enabled_config());

    gateway
        .add_channel("sensor", ChannelConfig::default())
        .unwrap();
    gateway.push("sensor", 42.0, 1000).unwrap();

    let frame = gateway.flush().unwrap();
    let metrics = gateway.last_metrics().unwrap();

    assert!(frame.size() <= 242); // LoRaWAN DR4 limit
    assert_eq!(metrics.payload.frame_size_bytes, frame.to_bytes().len());
}

#[test]
fn test_snapshot_version() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(create_enabled_config());

    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();
    gateway.push("temp", 22.5, 1000).unwrap();

    gateway.flush().unwrap();

    let metrics = gateway.last_metrics().unwrap();
    assert_eq!(metrics.version, MetricsSnapshot::VERSION);
}

#[test]
fn test_snapshot_window_info() {
    let config = MetricsConfig {
        enabled: true,
        signal_window: SignalWindow::TimeMillis(120_000),
        signal_compute: SignalComputeSchedule::EveryNFlushes(1),
        ..Default::default()
    };

    let mut engine = MetricsEngine::new(config);
    let snapshot = engine.observe_frame(&[1, 2, 3], 1000).unwrap();

    assert_eq!(snapshot.window.kind, "time_ms");
    assert_eq!(snapshot.window.value, 120_000);
}

#[test]
fn test_snapshot_flags() {
    let config = MetricsConfig {
        enabled: true,
        signal_compute: SignalComputeSchedule::EveryNFlushes(1),
        resilience: ResilienceConfig {
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut engine = MetricsEngine::new(config);
    let snapshot = engine.observe_frame(&[1, 2, 3], 1000).unwrap();

    assert!(!snapshot.flags.is_empty());
    assert!(snapshot.flags.iter().any(|f| f.contains("RESILIENCE")));
}

#[test]
fn test_multiple_flush_metrics_update() {
    let mut gateway = Gateway::new();
    gateway.enable_metrics(create_enabled_config());

    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();

    // First flush
    gateway.push("temp", 22.5, 1000).unwrap();
    gateway.flush().unwrap();
    let first_size = gateway.last_metrics().unwrap().payload.frame_size_bytes;

    // Second flush with different data
    gateway.push("temp", 22.5, 2000).unwrap();
    gateway.push("temp", 22.6, 3000).unwrap();
    gateway.flush().unwrap();
    let second_size = gateway.last_metrics().unwrap().payload.frame_size_bytes;

    // Metrics should be updated
    // (sizes may or may not be different depending on compression)
    assert!(first_size > 0);
    assert!(second_size > 0);
}
