// ALEC Complexity - Standalone complexity monitoring
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Complexity Monitoring configuration.

use serde::{Deserialize, Serialize};

/// Master configuration for Complexity Monitoring.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComplexityConfig {
    /// Master switch for complexity monitoring (default: false, opt-in).
    pub enabled: bool,

    /// Baseline learning configuration.
    pub baseline: BaselineConfig,

    /// Delta computation settings.
    pub deltas: DeltaConfig,

    /// Structure summary (S-lite) settings.
    pub structure: StructureConfig,

    /// Anomaly detection and event emission.
    pub anomaly: AnomalyConfig,

    /// Output settings.
    pub output: OutputConfig,
}

/// Baseline learning configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineConfig {
    /// Build baseline over this duration (ms) before locking.
    pub build_time_ms: u64,

    /// Minimum valid signal snapshots required to lock baseline.
    pub min_valid_snapshots: u32,

    /// Baseline update mode after lock.
    pub update_mode: BaselineUpdateMode,

    /// Rolling window size (if update_mode is Rolling).
    pub rolling_window_snapshots: u32,
}

impl Default for BaselineConfig {
    fn default() -> Self {
        Self {
            build_time_ms: 300_000, // 5 minutes
            min_valid_snapshots: 20,
            update_mode: BaselineUpdateMode::Frozen,
            rolling_window_snapshots: 100,
        }
    }
}

/// Baseline update mode after initial lock.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BaselineUpdateMode {
    /// Baseline is frozen after initial build (deterministic).
    Frozen,
    /// Baseline updates using exponential moving average.
    Ema {
        /// Alpha * 100 (e.g., 10 = 0.10). Stored as u32 to allow Eq derivation.
        alpha: u32,
    },
    /// Baseline updates using rolling window.
    Rolling,
}

/// Delta computation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaConfig {
    /// Compute delta for Total Correlation.
    pub compute_tc: bool,
    /// Compute delta for Resilience index R.
    pub compute_r: bool,
    /// Compute delta for joint entropy.
    pub compute_h_joint: bool,
    /// Compute delta for payload entropy.
    pub compute_payload_entropy: bool,

    /// Optional smoothing over deltas.
    pub smoothing: SmoothingConfig,
}

impl Default for DeltaConfig {
    fn default() -> Self {
        Self {
            compute_tc: true,
            compute_r: true,
            compute_h_joint: true,
            compute_payload_entropy: true,
            smoothing: SmoothingConfig::default(),
        }
    }
}

/// Smoothing configuration for deltas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmoothingConfig {
    /// Enable EMA smoothing on deltas.
    pub enabled: bool,
    /// EMA alpha factor (0.0 - 1.0).
    pub alpha: f64,
}

impl Default for SmoothingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            alpha: 0.2,
        }
    }
}

/// Structure summary (S-lite) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureConfig {
    /// Enable structure summaries.
    pub enabled: bool,

    /// Emit S-lite (lightweight pairwise edges).
    pub emit_s_lite: bool,

    /// Maximum channels to include in structure computation.
    pub max_channels: usize,

    /// Sparsification settings.
    pub sparsify: SparsifyConfig,

    /// Detect structure breaks (edge changes).
    pub detect_breaks: bool,

    /// Minimum edge weight change to consider a break.
    pub break_threshold: f64,
}

impl Default for StructureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            emit_s_lite: true,
            max_channels: 32,
            sparsify: SparsifyConfig::default(),
            detect_breaks: true,
            break_threshold: 0.3,
        }
    }
}

/// Sparsification settings for S-lite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparsifyConfig {
    /// Enable sparsification (recommended).
    pub enabled: bool,
    /// Keep only top K edges by absolute weight.
    pub top_k_edges: usize,
    /// Minimum absolute weight to include an edge.
    pub min_abs_weight: f64,
}

impl Default for SparsifyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            top_k_edges: 64,
            min_abs_weight: 0.2,
        }
    }
}

/// Anomaly detection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyConfig {
    /// Enable anomaly detection.
    pub enabled: bool,

    /// Z-score threshold for warnings.
    pub z_threshold_warn: f64,

    /// Z-score threshold for critical alerts.
    pub z_threshold_crit: f64,

    /// Minimum time (ms) for condition to persist before emitting event.
    pub persistence_ms: u64,

    /// Cooldown (ms) between events of the same type.
    pub cooldown_ms: u64,

    /// Enable specific event types.
    pub events: EventTypeConfig,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            z_threshold_warn: 2.0,
            z_threshold_crit: 3.0,
            persistence_ms: 30_000, // 30 seconds
            cooldown_ms: 120_000,   // 2 minutes
            events: EventTypeConfig::default(),
        }
    }
}

/// Per-event-type enable flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTypeConfig {
    pub baseline_events: bool,
    pub payload_entropy_spike: bool,
    pub structure_break: bool,
    pub redundancy_drop: bool,
    pub complexity_surge: bool,
    pub criticality_shift: bool,
}

impl Default for EventTypeConfig {
    fn default() -> Self {
        Self {
            baseline_events: true,
            payload_entropy_spike: true,
            structure_break: true,
            redundancy_drop: true,
            complexity_surge: true,
            criticality_shift: true,
        }
    }
}

/// Output configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Emit a ComplexitySnapshot every N process ticks.
    pub snapshot_every_n_ticks: u32,

    /// Emit events to the event stream.
    pub emit_events: bool,

    /// Include raw baseline stats in snapshot.
    pub include_baseline_stats: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            snapshot_every_n_ticks: 1,
            emit_events: true,
            include_baseline_stats: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ComplexityConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.baseline.build_time_ms, 300_000);
        assert_eq!(config.baseline.min_valid_snapshots, 20);
    }

    #[test]
    fn test_config_serialization() {
        let config = ComplexityConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ComplexityConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.enabled, parsed.enabled);
        assert_eq!(config.baseline.build_time_ms, parsed.baseline.build_time_ms);
    }

    #[test]
    fn test_baseline_update_modes() {
        let frozen = BaselineUpdateMode::Frozen;
        let ema = BaselineUpdateMode::Ema { alpha: 10 };
        let rolling = BaselineUpdateMode::Rolling;

        assert!(matches!(frozen, BaselineUpdateMode::Frozen));
        assert!(matches!(ema, BaselineUpdateMode::Ema { alpha: 10 }));
        assert!(matches!(rolling, BaselineUpdateMode::Rolling));
    }

    #[test]
    fn test_anomaly_thresholds() {
        let config = AnomalyConfig::default();
        assert!(config.z_threshold_crit > config.z_threshold_warn);
    }

    #[test]
    fn test_structure_config() {
        let config = StructureConfig::default();
        assert!(config.enabled);
        assert!(config.sparsify.enabled);
        assert!(config.sparsify.top_k_edges > 0);
    }
}
