// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Metrics configuration for ALEC Gateway.
//!
//! All metrics are opt-in and designed for minimal overhead when disabled.

use serde::{Deserialize, Serialize};

/// Master configuration for the Metrics module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Master switch. When false, metrics are fully disabled (near-zero overhead).
    pub enabled: bool,

    /// When to compute signal-level metrics.
    pub signal_compute: SignalComputeSchedule,

    /// Sliding window definition for signal samples.
    pub signal_window: SignalWindow,

    /// Alignment strategy for asynchronous channels.
    pub alignment: AlignmentStrategy,

    /// How to treat missing values after alignment.
    pub missing_data: MissingDataPolicy,

    /// Optional normalization prior to entropy estimation.
    pub normalization: NormalizationConfig,

    /// Signal entropy estimator backend.
    pub signal_estimator: SignalEstimator,

    /// Payload entropy configuration.
    pub payload: PayloadMetricsConfig,

    /// Resilience metrics (R + criticality) - separate opt-in.
    pub resilience: ResilienceConfig,

    /// Numerical safety settings.
    pub numerics: NumericsConfig,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Opt-in
            signal_compute: SignalComputeSchedule::NFlushesOrMillis {
                n_flushes: 10,
                millis: 10_000,
            },
            signal_window: SignalWindow::TimeMillis(60_000),
            alignment: AlignmentStrategy::SampleAndHold,
            missing_data: MissingDataPolicy::DropIncompleteSnapshots,
            normalization: NormalizationConfig::default(),
            signal_estimator: SignalEstimator::GaussianCovariance {
                log_base: LogBase::Two,
            },
            payload: PayloadMetricsConfig::default(),
            resilience: ResilienceConfig::default(),
            numerics: NumericsConfig::default(),
        }
    }
}

/// When to compute signal-level metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalComputeSchedule {
    /// Compute once every N flushes.
    EveryNFlushes(u32),
    /// Compute at most once every T milliseconds.
    EveryMillis(u64),
    /// Compute on whichever trigger fires first.
    NFlushesOrMillis { n_flushes: u32, millis: u64 },
}

/// Sliding window definition for signal samples.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalWindow {
    /// Keep samples within the last `millis` milliseconds.
    TimeMillis(u64),
    /// Keep only the last `n` samples per channel.
    LastNSamples(usize),
}

/// Alignment strategy to build multi-channel snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlignmentStrategy {
    /// Sample & Hold (ZOH): pick latest value <= t_ref for each channel.
    /// Most robust for gateways; default in v1.
    SampleAndHold,
    /// Nearest sample to t_ref.
    Nearest,
    /// Linear interpolation (requires bracketing samples).
    LinearInterpolation,
}

/// Policy for missing values after alignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MissingDataPolicy {
    /// Skip snapshot points where any required channel is missing.
    DropIncompleteSnapshots,
    /// Allow partial data with minimum channel requirement.
    AllowPartial { min_channels: usize },
    /// Fill missing channels with last-known values.
    FillWithLastKnown,
}

/// Normalization configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationConfig {
    pub enabled: bool,
    pub method: NormalizationMethod,
    pub min_samples: usize,
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            method: NormalizationMethod::ZScore,
            min_samples: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NormalizationMethod {
    /// Standardize using running mean/std.
    ZScore,
    /// Robust scaling using median/MAD.
    RobustMad,
    /// No scaling.
    None,
}

/// Signal entropy estimator backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalEstimator {
    /// Gaussian entropy via covariance (fast, stable).
    GaussianCovariance { log_base: LogBase },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogBase {
    /// Natural log (nats).
    E,
    /// Log base 2 (bits).
    Two,
}

/// Payload-level metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadMetricsConfig {
    /// Compute entropy over serialized frame bytes.
    pub frame_entropy: bool,
    /// Compute entropy per channel payload.
    pub per_channel_entropy: bool,
    /// Record frame and channel sizes.
    pub sizes: bool,
    /// Include raw byte histogram (256 bins).
    pub include_histogram: bool,
}

impl Default for PayloadMetricsConfig {
    fn default() -> Self {
        Self {
            frame_entropy: true,
            per_channel_entropy: false,
            sizes: true,
            include_histogram: false,
        }
    }
}

/// Resilience metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilienceConfig {
    /// Compute R (normalized redundancy index).
    pub enabled: bool,
    /// Per-channel criticality via leave-one-out ΔR.
    pub criticality: CriticalityConfig,
    /// Zone thresholds for R interpretation.
    pub thresholds: ResilienceThresholds,
    /// Minimum total univariate entropy to consider R valid.
    pub min_sum_h: f64,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Separate opt-in
            criticality: CriticalityConfig::default(),
            thresholds: ResilienceThresholds::default(),
            min_sum_h: 0.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalityConfig {
    /// Enable leave-one-out ΔR ranking.
    pub enabled: bool,
    /// Maximum channels for criticality computation.
    pub max_channels: usize,
    /// Compute criticality every N signal computations.
    pub every_n_signal_computes: u32,
}

impl Default for CriticalityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_channels: 16,
            every_n_signal_computes: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilienceThresholds {
    /// R >= this is "healthy".
    pub healthy_min: f64,
    /// R >= this is "attention" (below healthy).
    pub attention_min: f64,
    // Below attention_min is "critical".
}

impl Default for ResilienceThresholds {
    fn default() -> Self {
        Self {
            healthy_min: 0.5,
            attention_min: 0.2,
        }
    }
}

/// Numerical stability configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumericsConfig {
    /// Minimum aligned samples required for signal metrics.
    pub min_aligned_samples: usize,
    /// Diagonal regularization for covariance: Σ' = Σ + ε·I
    pub covariance_epsilon: f64,
    /// Maximum channels for joint entropy computation.
    pub max_channels_for_joint: usize,
}

impl Default for NumericsConfig {
    fn default() -> Self {
        Self {
            min_aligned_samples: 32,
            covariance_epsilon: 1e-8,
            max_channels_for_joint: 32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MetricsConfig::default();
        assert!(!config.enabled);
        assert!(matches!(
            config.signal_window,
            SignalWindow::TimeMillis(60_000)
        ));
        assert!(matches!(config.alignment, AlignmentStrategy::SampleAndHold));
    }

    #[test]
    fn test_config_serialization() {
        let config = MetricsConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: MetricsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.enabled, parsed.enabled);
    }

    #[test]
    fn test_resilience_thresholds() {
        let thresholds = ResilienceThresholds::default();
        assert!(thresholds.healthy_min > thresholds.attention_min);
    }

    #[test]
    fn test_numerics_config() {
        let config = NumericsConfig::default();
        assert!(config.covariance_epsilon > 0.0);
        assert!(config.min_aligned_samples > 0);
    }

    #[test]
    fn test_log_base() {
        assert_eq!(LogBase::Two, LogBase::Two);
        assert_ne!(LogBase::E, LogBase::Two);
    }
}
