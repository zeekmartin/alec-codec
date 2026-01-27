// ALEC Complexity - Standalone complexity monitoring
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! # ALEC Complexity
//!
//! Standalone complexity monitoring and anomaly detection for IoT systems.
//!
//! ALEC Complexity provides temporal interpretation of information-theoretic metrics:
//! - **Baseline learning**: Statistical summary of nominal operation
//! - **Delta computation**: Deviations from baseline with z-scores
//! - **S-lite structure**: Lightweight pairwise channel similarity summary
//! - **Anomaly detection**: Events with persistence and cooldown
//!
//! ## Standalone Usage
//!
//! ```rust
//! use alec_complexity::{ComplexityEngine, ComplexityConfig, GenericInput, InputAdapter};
//!
//! // Create engine with default config (disabled)
//! let mut config = ComplexityConfig::default();
//! config.enabled = true;
//! config.baseline.build_time_ms = 0; // No time requirement
//! config.baseline.min_valid_snapshots = 2;
//!
//! let mut engine = ComplexityEngine::new(config);
//!
//! // Build baseline
//! let input1 = GenericInput::new(1000, 3.0)
//!     .with_tc(1.0)
//!     .with_h_joint(2.0)
//!     .with_r(0.5)
//!     .build();
//! engine.process(&input1);
//!
//! let input2 = GenericInput::new(2000, 3.1)
//!     .with_tc(1.1)
//!     .with_h_joint(2.1)
//!     .with_r(0.6)
//!     .build();
//! engine.process(&input2);
//!
//! // Now baseline is locked, subsequent inputs produce full analysis
//! let input3 = GenericInput::new(3000, 3.5)
//!     .with_tc(1.2)
//!     .with_h_joint(2.2)
//!     .with_r(0.5)
//!     .build();
//!
//! if let Some(snapshot) = engine.process(&input3) {
//!     assert!(snapshot.is_baseline_locked());
//!     assert!(snapshot.deltas.is_some());
//!     assert!(snapshot.z_scores.is_some());
//! }
//! ```
//!
//! ## With ALEC Gateway (feature-gated)
//!
//! When compiled with the `gateway` feature, you can use metrics directly from
//! the ALEC Gateway:
//!
//! ```ignore
//! use alec_complexity::{ComplexityEngine, ComplexityConfig};
//! use alec_complexity::input::MetricsSnapshotExt;
//!
//! // Get MetricsSnapshot from Gateway...
//! let metrics_snapshot = gateway.flush(timestamp_ms);
//!
//! // Convert to complexity input
//! let input = metrics_snapshot.to_complexity_input();
//!
//! // Process
//! let result = engine.process(&input);
//! ```
//!
//! ## Key Concepts
//!
//! ### Baseline
//!
//! The baseline represents "normal" system behavior. During the building phase,
//! the engine collects statistical summaries (mean, std) of key metrics. Once
//! locked, deviations from baseline trigger anomaly detection.
//!
//! ### Deltas and Z-Scores
//!
//! Deltas measure the absolute difference from baseline means. Z-scores normalize
//! these deviations by the baseline standard deviation, making anomaly detection
//! robust across different metric scales.
//!
//! ### S-lite (Structure Summary)
//!
//! S-lite captures pairwise relationships between channels using normalized
//! entropy differences. This lightweight structure enables detection of
//! coordination changes without storing full dependency graphs.
//!
//! ### Anomaly Events
//!
//! Events are emitted when z-scores exceed thresholds. The system includes:
//! - **Persistence**: Conditions must persist for a minimum duration
//! - **Cooldown**: Same event type cannot repeat within a cooldown window
//! - **Severity levels**: Warning and Critical based on threshold levels
//!
//! ## Configuration
//!
//! The engine is highly configurable via `ComplexityConfig`:
//!
//! ```rust
//! use alec_complexity::config::*;
//!
//! let config = ComplexityConfig {
//!     enabled: true,
//!     baseline: BaselineConfig {
//!         build_time_ms: 60_000,
//!         min_valid_snapshots: 10,
//!         update_mode: BaselineUpdateMode::Ema { alpha: 10 }, // 10 = 0.10
//!         ..Default::default()
//!     },
//!     anomaly: AnomalyConfig {
//!         enabled: true,
//!         z_threshold_warn: 2.0,
//!         z_threshold_crit: 3.0,
//!         persistence_ms: 5000,
//!         cooldown_ms: 30000,
//!         ..Default::default()
//!     },
//!     ..Default::default()
//! };
//! ```

// Core modules
pub mod anomaly;
pub mod baseline;
pub mod config;
pub mod delta;
pub mod engine;
pub mod event;
pub mod input;
pub mod snapshot;
pub mod structure;

// Re-exports for convenience
pub use config::ComplexityConfig;
pub use engine::ComplexityEngine;
pub use event::{ComplexityEvent, EventSeverity, EventType};
pub use input::{ChannelEntropy, GenericInput, InputAdapter, InputSnapshot};
pub use snapshot::ComplexitySnapshot;

// Gateway-specific re-exports
#[cfg(feature = "gateway")]
pub use input::{GatewayInput, MetricsSnapshotExt};

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_basic_workflow() {
        let config = ComplexityConfig {
            enabled: true,
            baseline: config::BaselineConfig {
                min_valid_snapshots: 2,
                build_time_ms: 0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut engine = ComplexityEngine::new(config);

        // Build baseline
        let input1 = GenericInput::new(1000, 3.0)
            .with_tc(1.0)
            .with_h_joint(2.0)
            .with_r(0.5)
            .build();
        let result1 = engine.process(&input1).unwrap();
        assert!(!result1.is_baseline_locked());

        let input2 = GenericInput::new(2000, 3.1)
            .with_tc(1.1)
            .with_h_joint(2.1)
            .with_r(0.6)
            .build();
        let result2 = engine.process(&input2).unwrap();
        assert!(result2.is_baseline_locked());

        // Full analysis
        let input3 = GenericInput::new(3000, 3.2)
            .with_tc(1.2)
            .with_h_joint(2.2)
            .with_r(0.5)
            .build();
        let result3 = engine.process(&input3).unwrap();
        assert!(result3.deltas.is_some());
        assert!(result3.z_scores.is_some());
    }

    #[test]
    fn test_json_input() {
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
        assert!(snapshot.tc.is_some());
    }

    #[test]
    fn test_snapshot_serialization() {
        let config = ComplexityConfig {
            enabled: true,
            baseline: config::BaselineConfig {
                min_valid_snapshots: 2,
                build_time_ms: 0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut engine = ComplexityEngine::new(config);

        // Build baseline
        engine.process(&GenericInput::new(1000, 3.0).build());
        engine.process(&GenericInput::new(2000, 3.0).build());

        // Get full snapshot
        let result = engine
            .process(&GenericInput::new(3000, 3.5).build())
            .unwrap();

        // Serialize
        let json = result.to_json().unwrap();
        assert!(json.contains("\"version\""));
        assert!(json.contains("\"timestamp_ms\""));

        // Deserialize
        let restored = ComplexitySnapshot::from_json(&json).unwrap();
        assert_eq!(restored.timestamp_ms, result.timestamp_ms);
    }
}
