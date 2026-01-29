// ALEC Testdata - Anomaly injection
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Anomaly injection for testing anomaly detection systems.
//!
//! This module provides various anomaly types that can be injected
//! into generated sensor data to test ALEC Complexity detection.

use rand::prelude::*;
use rand_distr::Normal;
use serde::{Deserialize, Serialize};

/// Anomaly injection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyConfig {
    /// Type of anomaly to inject.
    pub anomaly_type: AnomalyType,
    /// Sample index when anomaly starts.
    pub start_sample: usize,
    /// Duration in samples (None = until end).
    pub duration_samples: Option<usize>,
}

impl AnomalyConfig {
    /// Create a new anomaly configuration.
    pub fn new(anomaly_type: AnomalyType, start_sample: usize) -> Self {
        Self {
            anomaly_type,
            start_sample,
            duration_samples: None,
        }
    }

    /// Set duration in samples.
    pub fn with_duration(mut self, samples: usize) -> Self {
        self.duration_samples = Some(samples);
        self
    }

    /// Check if anomaly is active at given sample index.
    pub fn is_active(&self, sample_idx: usize) -> bool {
        if sample_idx < self.start_sample {
            return false;
        }
        match self.duration_samples {
            Some(duration) => sample_idx < self.start_sample + duration,
            None => true,
        }
    }

    /// Get samples since anomaly start.
    pub fn samples_since_start(&self, sample_idx: usize) -> usize {
        sample_idx.saturating_sub(self.start_sample)
    }
}

/// Type of anomaly to inject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyType {
    /// Sensor value stuck at last reading.
    ///
    /// Expected detection: STRUCTURE_BREAK (correlation lost)
    Stuck,

    /// Sudden spike in value.
    ///
    /// Expected detection: PAYLOAD_ENTROPY_SPIKE
    Spike {
        /// Magnitude relative to normal range.
        magnitude: f64,
    },

    /// Gradual drift away from normal.
    ///
    /// Expected detection: COMPLEXITY_SURGE
    Drift {
        /// Rate of drift per sample.
        rate_per_sample: f64,
    },

    /// Value becomes independent (decorrelated) noise.
    ///
    /// Expected detection: STRUCTURE_BREAK
    Decorrelate {
        /// Standard deviation of random values.
        noise_std: f64,
        /// Center value for random distribution.
        center: f64,
    },

    /// Complete dropout (NaN or missing).
    ///
    /// Note: Results in missing data points.
    Dropout,

    /// Oscillation with unusual frequency.
    ///
    /// Expected detection: COMPLEXITY_SURGE or PAYLOAD_ENTROPY_SPIKE
    Oscillation {
        /// Amplitude of oscillation.
        amplitude: f64,
        /// Frequency in Hz.
        frequency_hz: f64,
    },

    /// Bias shift (constant offset added).
    ///
    /// Expected detection: Depends on magnitude
    BiasShift {
        /// Offset to add.
        offset: f64,
    },

    /// Noise increase (higher variance).
    ///
    /// Expected detection: PAYLOAD_ENTROPY_SPIKE
    NoiseIncrease {
        /// Factor to multiply noise by (>1 increases).
        factor: f64,
    },

    /// Sensor clipping at min/max.
    ///
    /// Expected detection: STRUCTURE_BREAK
    Clipping {
        /// Clip to this minimum.
        min: f64,
        /// Clip to this maximum.
        max: f64,
    },

    /// Intermittent failure (random dropouts).
    ///
    /// Expected detection: Depends on frequency
    Intermittent {
        /// Probability of failure per sample.
        failure_prob: f64,
    },
}

impl AnomalyType {
    /// Create a stuck sensor anomaly.
    pub fn stuck() -> Self {
        AnomalyType::Stuck
    }

    /// Create a spike anomaly.
    pub fn spike(magnitude: f64) -> Self {
        AnomalyType::Spike { magnitude }
    }

    /// Create a drift anomaly.
    pub fn drift(rate_per_sample: f64) -> Self {
        AnomalyType::Drift { rate_per_sample }
    }

    /// Create a decorrelation anomaly.
    pub fn decorrelate(center: f64, noise_std: f64) -> Self {
        AnomalyType::Decorrelate { noise_std, center }
    }

    /// Get expected detection event type.
    pub fn expected_event(&self) -> &'static str {
        match self {
            AnomalyType::Stuck => "STRUCTURE_BREAK",
            AnomalyType::Spike { .. } => "PAYLOAD_ENTROPY_SPIKE",
            AnomalyType::Drift { .. } => "COMPLEXITY_SURGE",
            AnomalyType::Decorrelate { .. } => "STRUCTURE_BREAK",
            AnomalyType::Dropout => "REDUNDANCY_DROP",
            AnomalyType::Oscillation { .. } => "COMPLEXITY_SURGE",
            AnomalyType::BiasShift { .. } => "COMPLEXITY_SURGE",
            AnomalyType::NoiseIncrease { .. } => "PAYLOAD_ENTROPY_SPIKE",
            AnomalyType::Clipping { .. } => "STRUCTURE_BREAK",
            AnomalyType::Intermittent { .. } => "REDUNDANCY_DROP",
        }
    }
}

/// State for anomaly application.
#[derive(Debug, Clone, Default)]
pub struct AnomalyState {
    /// Last value before anomaly (for Stuck).
    pub last_value: Option<f64>,
    /// Accumulated drift.
    pub drift_accumulated: f64,
}

impl AnomalyState {
    /// Apply anomaly to a value.
    pub fn apply(
        &mut self,
        anomaly: &AnomalyType,
        value: f64,
        samples_since_start: usize,
        rng: &mut (impl Rng + ?Sized),
    ) -> Option<f64> {
        match anomaly {
            AnomalyType::Stuck => {
                if self.last_value.is_none() {
                    self.last_value = Some(value);
                }
                self.last_value
            }

            AnomalyType::Spike { magnitude } => {
                // Apply spike at the start, then return to normal
                if samples_since_start == 0 {
                    Some(value + magnitude)
                } else {
                    Some(value)
                }
            }

            AnomalyType::Drift { rate_per_sample } => {
                self.drift_accumulated += rate_per_sample;
                Some(value + self.drift_accumulated)
            }

            AnomalyType::Decorrelate { noise_std, center } => {
                let normal = Normal::new(*center, *noise_std).unwrap();
                Some(normal.sample(rng))
            }

            AnomalyType::Dropout => None,

            AnomalyType::Oscillation {
                amplitude,
                frequency_hz,
            } => {
                let t = samples_since_start as f64;
                let osc = amplitude * (2.0 * std::f64::consts::PI * frequency_hz * t).sin();
                Some(value + osc)
            }

            AnomalyType::BiasShift { offset } => Some(value + offset),

            AnomalyType::NoiseIncrease { factor } => {
                let noise_amount = (rng.gen::<f64>() - 0.5) * factor * 2.0;
                Some(value + noise_amount)
            }

            AnomalyType::Clipping { min, max } => Some(value.clamp(*min, *max)),

            AnomalyType::Intermittent { failure_prob } => {
                if rng.gen::<f64>() < *failure_prob {
                    self.last_value // Return last good value or None
                } else {
                    self.last_value = Some(value);
                    Some(value)
                }
            }
        }
    }
}

/// Builder for common anomaly scenarios.
pub struct AnomalyBuilder {
    start_percent: f64,
    duration_percent: Option<f64>,
}

impl AnomalyBuilder {
    /// Create a new anomaly builder.
    pub fn new() -> Self {
        Self {
            start_percent: 0.5,
            duration_percent: None,
        }
    }

    /// Set start position as percentage of total samples (0.0 - 1.0).
    pub fn start_at_percent(mut self, percent: f64) -> Self {
        self.start_percent = percent;
        self
    }

    /// Set duration as percentage of total samples.
    pub fn duration_percent(mut self, percent: f64) -> Self {
        self.duration_percent = Some(percent);
        self
    }

    /// Build anomaly config for given total samples.
    pub fn build(self, total_samples: usize, anomaly_type: AnomalyType) -> AnomalyConfig {
        let start = (total_samples as f64 * self.start_percent) as usize;
        let duration = self
            .duration_percent
            .map(|p| (total_samples as f64 * p) as usize);

        AnomalyConfig {
            anomaly_type,
            start_sample: start,
            duration_samples: duration,
        }
    }

    /// Build a sensor failure (stuck) anomaly.
    pub fn sensor_failure(self, total_samples: usize) -> AnomalyConfig {
        self.build(total_samples, AnomalyType::Stuck)
    }

    /// Build a gradual drift anomaly.
    pub fn gradual_drift(self, total_samples: usize, rate: f64) -> AnomalyConfig {
        self.build(
            total_samples,
            AnomalyType::Drift {
                rate_per_sample: rate,
            },
        )
    }

    /// Build a sudden spike anomaly.
    pub fn sudden_spike(self, total_samples: usize, magnitude: f64) -> AnomalyConfig {
        self.build(total_samples, AnomalyType::Spike { magnitude })
    }

    /// Build a correlation break anomaly.
    pub fn correlation_break(self, total_samples: usize, center: f64, std: f64) -> AnomalyConfig {
        self.build(
            total_samples,
            AnomalyType::Decorrelate {
                center,
                noise_std: std,
            },
        )
    }
}

impl Default for AnomalyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn test_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    #[test]
    fn test_anomaly_config_active() {
        let config = AnomalyConfig {
            anomaly_type: AnomalyType::Stuck,
            start_sample: 100,
            duration_samples: Some(50),
        };

        assert!(!config.is_active(99));
        assert!(config.is_active(100));
        assert!(config.is_active(149));
        assert!(!config.is_active(150));
    }

    #[test]
    fn test_anomaly_config_no_duration() {
        let config = AnomalyConfig {
            anomaly_type: AnomalyType::Stuck,
            start_sample: 100,
            duration_samples: None,
        };

        assert!(!config.is_active(99));
        assert!(config.is_active(100));
        assert!(config.is_active(1000));
    }

    #[test]
    fn test_stuck_anomaly() {
        let mut rng = test_rng();
        let mut state = AnomalyState::default();
        let anomaly = AnomalyType::Stuck;

        // First value gets stored
        let v1 = state.apply(&anomaly, 25.0, 0, &mut rng);
        assert_eq!(v1, Some(25.0));

        // Subsequent values return stored value
        let v2 = state.apply(&anomaly, 30.0, 1, &mut rng);
        assert_eq!(v2, Some(25.0));

        let v3 = state.apply(&anomaly, 35.0, 2, &mut rng);
        assert_eq!(v3, Some(25.0));
    }

    #[test]
    fn test_spike_anomaly() {
        let mut rng = test_rng();
        let mut state = AnomalyState::default();
        let anomaly = AnomalyType::Spike { magnitude: 100.0 };

        // Spike at start
        let v1 = state.apply(&anomaly, 50.0, 0, &mut rng);
        assert_eq!(v1, Some(150.0));

        // Normal after
        let v2 = state.apply(&anomaly, 50.0, 1, &mut rng);
        assert_eq!(v2, Some(50.0));
    }

    #[test]
    fn test_drift_anomaly() {
        let mut rng = test_rng();
        let mut state = AnomalyState::default();
        let anomaly = AnomalyType::Drift {
            rate_per_sample: 0.5,
        };

        let v1 = state.apply(&anomaly, 50.0, 0, &mut rng);
        assert_eq!(v1, Some(50.5));

        let v2 = state.apply(&anomaly, 50.0, 1, &mut rng);
        assert_eq!(v2, Some(51.0));

        let v3 = state.apply(&anomaly, 50.0, 2, &mut rng);
        assert_eq!(v3, Some(51.5));
    }

    #[test]
    fn test_dropout_anomaly() {
        let mut rng = test_rng();
        let mut state = AnomalyState::default();
        let anomaly = AnomalyType::Dropout;

        let v = state.apply(&anomaly, 50.0, 0, &mut rng);
        assert_eq!(v, None);
    }

    #[test]
    fn test_clipping_anomaly() {
        let mut rng = test_rng();
        let mut state = AnomalyState::default();
        let anomaly = AnomalyType::Clipping {
            min: 20.0,
            max: 80.0,
        };

        // Normal value passes through
        let v1 = state.apply(&anomaly, 50.0, 0, &mut rng);
        assert_eq!(v1, Some(50.0));

        // Low value gets clipped
        let v2 = state.apply(&anomaly, 10.0, 1, &mut rng);
        assert_eq!(v2, Some(20.0));

        // High value gets clipped
        let v3 = state.apply(&anomaly, 90.0, 2, &mut rng);
        assert_eq!(v3, Some(80.0));
    }

    #[test]
    fn test_anomaly_builder() {
        let config = AnomalyBuilder::new()
            .start_at_percent(0.3)
            .duration_percent(0.2)
            .sensor_failure(1000);

        assert_eq!(config.start_sample, 300);
        assert_eq!(config.duration_samples, Some(200));
    }

    #[test]
    fn test_expected_event() {
        assert_eq!(AnomalyType::Stuck.expected_event(), "STRUCTURE_BREAK");
        assert_eq!(
            AnomalyType::Spike { magnitude: 10.0 }.expected_event(),
            "PAYLOAD_ENTROPY_SPIKE"
        );
        assert_eq!(
            AnomalyType::Drift {
                rate_per_sample: 0.1
            }
            .expected_event(),
            "COMPLEXITY_SURGE"
        );
    }
}
