// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Signal-level entropy computation using Gaussian approximation.

use super::alignment::AlignedSnapshot;
use super::config::{LogBase, NumericsConfig};
use nalgebra::{DMatrix, DVector};

/// Signal-level metrics result.
#[derive(Debug, Clone)]
pub struct SignalMetrics {
    /// Entropy per channel: H(X_i).
    pub h_per_channel: Vec<ChannelEntropy>,
    /// Sum of univariate entropies: Σ H(X_i).
    pub sum_h: f64,
    /// Joint entropy: H(X_1, ..., X_n).
    pub h_joint: f64,
    /// Total Correlation (Watanabe): TC = Σ H(X_i) - H_joint.
    pub total_correlation: f64,
    /// Number of aligned samples used.
    pub aligned_samples: usize,
    /// Number of channels included.
    pub channels_included: usize,
}

#[derive(Debug, Clone)]
pub struct ChannelEntropy {
    pub channel_id: String,
    pub h: f64,
}

/// Gaussian-based signal entropy estimator.
pub struct GaussianEntropyEstimator {
    log_base: LogBase,
    numerics: NumericsConfig,
}

impl GaussianEntropyEstimator {
    pub fn new(log_base: LogBase, numerics: NumericsConfig) -> Self {
        Self { log_base, numerics }
    }

    /// Compute signal metrics from aligned snapshots.
    pub fn compute(
        &self,
        snapshots: &[AlignedSnapshot],
        channel_ids: &[String],
    ) -> Option<SignalMetrics> {
        if snapshots.len() < self.numerics.min_aligned_samples {
            return None;
        }

        let n_channels = channel_ids.len();
        if n_channels == 0 || n_channels > self.numerics.max_channels_for_joint {
            return None;
        }

        let n_samples = snapshots.len();

        // Build data matrix (samples x channels)
        let mut data = DMatrix::<f64>::zeros(n_samples, n_channels);
        for (i, snapshot) in snapshots.iter().enumerate() {
            for (j, value) in snapshot.values.iter().enumerate() {
                if j < n_channels {
                    data[(i, j)] = *value;
                }
            }
        }

        // Compute per-channel entropies
        let mut h_per_channel = Vec::with_capacity(n_channels);
        let mut sum_h = 0.0;

        for (j, channel_id) in channel_ids.iter().enumerate() {
            let column: Vec<f64> = (0..n_samples).map(|i| data[(i, j)]).collect();
            let variance = Self::variance(&column);
            let h = self.gaussian_entropy_1d(variance);
            h_per_channel.push(ChannelEntropy {
                channel_id: channel_id.clone(),
                h,
            });
            sum_h += h;
        }

        // Compute covariance matrix
        let cov = self.covariance_matrix(&data);

        // Add regularization
        let cov_reg = &cov
            + DMatrix::<f64>::identity(n_channels, n_channels) * self.numerics.covariance_epsilon;

        // Compute joint entropy from covariance determinant
        let h_joint = self.gaussian_entropy_nd(&cov_reg, n_channels);

        // Total Correlation
        let total_correlation = (sum_h - h_joint).max(0.0);

        Some(SignalMetrics {
            h_per_channel,
            sum_h,
            h_joint,
            total_correlation,
            aligned_samples: n_samples,
            channels_included: n_channels,
        })
    }

    /// Gaussian entropy for 1D: H(X) = 0.5 * log(2πeσ²)
    fn gaussian_entropy_1d(&self, variance: f64) -> f64 {
        if variance <= 0.0 {
            return 0.0;
        }
        let entropy_nats = 0.5 * (2.0 * std::f64::consts::PI * std::f64::consts::E * variance).ln();
        self.convert_log_base(entropy_nats)
    }

    /// Gaussian entropy for nD: H(X) = 0.5 * log((2πe)^n * |Σ|)
    fn gaussian_entropy_nd(&self, cov: &DMatrix<f64>, n: usize) -> f64 {
        let det = cov.determinant();
        if det <= 0.0 {
            return 0.0;
        }
        let coeff = (2.0 * std::f64::consts::PI * std::f64::consts::E).powi(n as i32);
        let entropy_nats = 0.5 * (coeff * det).ln();
        self.convert_log_base(entropy_nats)
    }

    fn convert_log_base(&self, nats: f64) -> f64 {
        match self.log_base {
            LogBase::E => nats,
            LogBase::Two => nats / std::f64::consts::LN_2,
        }
    }

    fn variance(values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let sum_sq: f64 = values.iter().map(|x| (x - mean).powi(2)).sum();
        sum_sq / (n - 1.0).max(1.0) // Sample variance
    }

    fn covariance_matrix(&self, data: &DMatrix<f64>) -> DMatrix<f64> {
        let n_samples = data.nrows();
        let n_channels = data.ncols();

        if n_samples < 2 {
            return DMatrix::<f64>::zeros(n_channels, n_channels);
        }

        // Compute means
        let means: DVector<f64> = DVector::from_iterator(
            n_channels,
            (0..n_channels)
                .map(|j| (0..n_samples).map(|i| data[(i, j)]).sum::<f64>() / n_samples as f64),
        );

        // Center data
        let mut centered = data.clone();
        for i in 0..n_samples {
            for j in 0..n_channels {
                centered[(i, j)] -= means[j];
            }
        }

        // Covariance = (1/(n-1)) * X^T * X
        let cov = centered.transpose() * &centered;
        cov / (n_samples - 1) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_snapshots(n_samples: usize, n_channels: usize) -> Vec<AlignedSnapshot> {
        (0..n_samples)
            .map(|i| AlignedSnapshot {
                values: (0..n_channels)
                    .map(|j| (i as f64) + (j as f64) * 0.1)
                    .collect(),
                channel_ids: (0..n_channels).map(|j| format!("ch{}", j)).collect(),
                timestamp_ms: i as u64 * 1000,
            })
            .collect()
    }

    #[test]
    fn test_estimator_creation() {
        let numerics = NumericsConfig::default();
        let estimator = GaussianEntropyEstimator::new(LogBase::Two, numerics);
        assert!(estimator.log_base == LogBase::Two);
    }

    #[test]
    fn test_insufficient_samples() {
        let numerics = NumericsConfig {
            min_aligned_samples: 32,
            ..Default::default()
        };
        let estimator = GaussianEntropyEstimator::new(LogBase::Two, numerics);

        let snapshots = create_test_snapshots(10, 2); // Only 10 samples
        let channel_ids = vec!["ch0".to_string(), "ch1".to_string()];

        let result = estimator.compute(&snapshots, &channel_ids);
        assert!(result.is_none());
    }

    #[test]
    fn test_compute_basic() {
        let numerics = NumericsConfig {
            min_aligned_samples: 10,
            ..Default::default()
        };
        let estimator = GaussianEntropyEstimator::new(LogBase::Two, numerics);

        let snapshots = create_test_snapshots(50, 2);
        let channel_ids = vec!["ch0".to_string(), "ch1".to_string()];

        let result = estimator.compute(&snapshots, &channel_ids).unwrap();

        assert_eq!(result.aligned_samples, 50);
        assert_eq!(result.channels_included, 2);
        assert_eq!(result.h_per_channel.len(), 2);
        assert!(result.sum_h > 0.0);
        // h_joint can be negative for low-variance or near-singular covariance (Gaussian entropy property)
        // Just verify it's finite
        assert!(result.h_joint.is_finite());
        assert!(result.total_correlation >= 0.0);
    }

    #[test]
    fn test_total_correlation_nonnegative() {
        let numerics = NumericsConfig {
            min_aligned_samples: 10,
            ..Default::default()
        };
        let estimator = GaussianEntropyEstimator::new(LogBase::Two, numerics);

        let snapshots = create_test_snapshots(100, 3);
        let channel_ids = vec!["ch0".to_string(), "ch1".to_string(), "ch2".to_string()];

        let result = estimator.compute(&snapshots, &channel_ids).unwrap();
        assert!(result.total_correlation >= 0.0);
    }

    #[test]
    fn test_variance_computation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let variance = GaussianEntropyEstimator::variance(&values);
        // Sample variance of 1-5 should be 2.5
        assert!((variance - 2.5).abs() < 0.001);
    }

    #[test]
    fn test_entropy_positive_variance() {
        let numerics = NumericsConfig::default();
        let estimator = GaussianEntropyEstimator::new(LogBase::Two, numerics);

        let h = estimator.gaussian_entropy_1d(1.0);
        assert!(h > 0.0);
    }

    #[test]
    fn test_entropy_zero_variance() {
        let numerics = NumericsConfig::default();
        let estimator = GaussianEntropyEstimator::new(LogBase::Two, numerics);

        let h = estimator.gaussian_entropy_1d(0.0);
        assert_eq!(h, 0.0);
    }

    #[test]
    fn test_log_base_conversion() {
        let numerics = NumericsConfig::default();

        let estimator_bits = GaussianEntropyEstimator::new(LogBase::Two, numerics.clone());
        let estimator_nats = GaussianEntropyEstimator::new(LogBase::E, numerics);

        let h_bits = estimator_bits.gaussian_entropy_1d(1.0);
        let h_nats = estimator_nats.gaussian_entropy_1d(1.0);

        // h_bits = h_nats / ln(2)
        assert!((h_bits - h_nats / std::f64::consts::LN_2).abs() < 0.001);
    }
}
