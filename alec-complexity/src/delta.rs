// ALEC Complexity - Standalone complexity monitoring
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Delta and z-score computation.

use crate::baseline::Baseline;
use crate::config::DeltaConfig;
use serde::{Deserialize, Serialize};

/// Delta values (current - baseline mean).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Deltas {
    pub tc: Option<f64>,
    pub h_joint: Option<f64>,
    pub h_bytes: f64,
    pub r: Option<f64>,
}

/// Z-scores ((current - mean) / std).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ZScores {
    pub tc: Option<f64>,
    pub h_joint: Option<f64>,
    pub h_bytes: f64,
    pub r: Option<f64>,
}

impl ZScores {
    /// Get the maximum absolute z-score.
    pub fn max_abs(&self) -> f64 {
        let mut max = self.h_bytes.abs();
        if let Some(tc) = self.tc {
            max = max.max(tc.abs());
        }
        if let Some(hj) = self.h_joint {
            max = max.max(hj.abs());
        }
        if let Some(r) = self.r {
            max = max.max(r.abs());
        }
        max
    }
}

/// Calculator for deltas and z-scores.
pub struct DeltaCalculator {
    config: DeltaConfig,
    smoothed_deltas: Deltas,
}

impl DeltaCalculator {
    pub fn new(config: DeltaConfig) -> Self {
        Self {
            config,
            smoothed_deltas: Deltas::default(),
        }
    }

    /// Compute deltas and z-scores from baseline and current values.
    pub fn compute(
        &mut self,
        baseline: &Baseline,
        tc: Option<f64>,
        h_joint: Option<f64>,
        h_bytes: f64,
        r: Option<f64>,
    ) -> (Deltas, ZScores) {
        let mut deltas = Deltas::default();
        let mut z_scores = ZScores::default();

        // Compute payload entropy delta/z
        if self.config.compute_payload_entropy && baseline.h_bytes.is_valid() {
            deltas.h_bytes = h_bytes - baseline.h_bytes.mean;
            z_scores.h_bytes = if baseline.h_bytes.std > 0.0 {
                deltas.h_bytes / baseline.h_bytes.std
            } else {
                0.0
            };
        }

        // Compute TC delta/z
        if self.config.compute_tc {
            if let Some(tc_val) = tc {
                if baseline.tc.is_valid() {
                    deltas.tc = Some(tc_val - baseline.tc.mean);
                    z_scores.tc = Some(if baseline.tc.std > 0.0 {
                        deltas.tc.unwrap() / baseline.tc.std
                    } else {
                        0.0
                    });
                }
            }
        }

        // Compute h_joint delta/z
        if self.config.compute_h_joint {
            if let Some(hj_val) = h_joint {
                if baseline.h_joint.is_valid() {
                    deltas.h_joint = Some(hj_val - baseline.h_joint.mean);
                    z_scores.h_joint = Some(if baseline.h_joint.std > 0.0 {
                        deltas.h_joint.unwrap() / baseline.h_joint.std
                    } else {
                        0.0
                    });
                }
            }
        }

        // Compute R delta/z
        if self.config.compute_r {
            if let (Some(r_val), Some(ref r_stats)) = (r, &baseline.r) {
                if r_stats.is_valid() {
                    deltas.r = Some(r_val - r_stats.mean);
                    z_scores.r = Some(if r_stats.std > 0.0 {
                        deltas.r.unwrap() / r_stats.std
                    } else {
                        0.0
                    });
                }
            }
        }

        // Apply smoothing if enabled
        if self.config.smoothing.enabled {
            let alpha = self.config.smoothing.alpha;
            self.smoothed_deltas.h_bytes =
                alpha * deltas.h_bytes + (1.0 - alpha) * self.smoothed_deltas.h_bytes;

            if let Some(tc_delta) = deltas.tc {
                let prev = self.smoothed_deltas.tc.unwrap_or(tc_delta);
                self.smoothed_deltas.tc = Some(alpha * tc_delta + (1.0 - alpha) * prev);
            }

            if let Some(hj_delta) = deltas.h_joint {
                let prev = self.smoothed_deltas.h_joint.unwrap_or(hj_delta);
                self.smoothed_deltas.h_joint = Some(alpha * hj_delta + (1.0 - alpha) * prev);
            }

            if let Some(r_delta) = deltas.r {
                let prev = self.smoothed_deltas.r.unwrap_or(r_delta);
                self.smoothed_deltas.r = Some(alpha * r_delta + (1.0 - alpha) * prev);
            }

            // Return smoothed deltas
            (self.smoothed_deltas.clone(), z_scores)
        } else {
            (deltas, z_scores)
        }
    }

    /// Get last smoothed deltas.
    pub fn smoothed_deltas(&self) -> &Deltas {
        &self.smoothed_deltas
    }

    /// Reset smoothed state.
    pub fn reset(&mut self) {
        self.smoothed_deltas = Deltas::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baseline::FieldStats;

    fn create_test_baseline() -> Baseline {
        let mut baseline = Baseline::new(true);

        // Set up h_bytes stats
        baseline.h_bytes = FieldStats {
            mean: 5.0,
            std: 1.0,
            count: 10,
            ..Default::default()
        };

        // Set up tc stats
        baseline.tc = FieldStats {
            mean: 2.0,
            std: 0.5,
            count: 10,
            ..Default::default()
        };

        // Set up h_joint stats
        baseline.h_joint = FieldStats {
            mean: 8.0,
            std: 1.0,
            count: 10,
            ..Default::default()
        };

        // Set up r stats
        baseline.r = Some(FieldStats {
            mean: 0.5,
            std: 0.1,
            count: 10,
            ..Default::default()
        });

        baseline
    }

    #[test]
    fn test_delta_computation() {
        let config = DeltaConfig {
            smoothing: crate::config::SmoothingConfig {
                enabled: false,
                alpha: 0.2,
            },
            ..Default::default()
        };
        let mut calculator = DeltaCalculator::new(config);
        let baseline = create_test_baseline();

        let (deltas, z_scores) =
            calculator.compute(&baseline, Some(3.0), Some(10.0), 7.0, Some(0.3));

        // h_bytes: delta = 7.0 - 5.0 = 2.0, z = 2.0 / 1.0 = 2.0
        assert!((deltas.h_bytes - 2.0).abs() < 0.001);
        assert!((z_scores.h_bytes - 2.0).abs() < 0.001);

        // tc: delta = 3.0 - 2.0 = 1.0, z = 1.0 / 0.5 = 2.0
        assert!((deltas.tc.unwrap() - 1.0).abs() < 0.001);
        assert!((z_scores.tc.unwrap() - 2.0).abs() < 0.001);

        // h_joint: delta = 10.0 - 8.0 = 2.0, z = 2.0 / 1.0 = 2.0
        assert!((deltas.h_joint.unwrap() - 2.0).abs() < 0.001);
        assert!((z_scores.h_joint.unwrap() - 2.0).abs() < 0.001);

        // r: delta = 0.3 - 0.5 = -0.2, z = -0.2 / 0.1 = -2.0
        assert!((deltas.r.unwrap() - (-0.2)).abs() < 0.001);
        assert!((z_scores.r.unwrap() - (-2.0)).abs() < 0.001);
    }

    #[test]
    fn test_z_score_max_abs() {
        let z = ZScores {
            tc: Some(-3.0),
            h_joint: Some(2.0),
            h_bytes: 1.5,
            r: Some(-1.0),
        };

        assert!((z.max_abs() - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_smoothing() {
        let config = DeltaConfig {
            smoothing: crate::config::SmoothingConfig {
                enabled: true,
                alpha: 0.5,
            },
            ..Default::default()
        };
        let mut calculator = DeltaCalculator::new(config);
        let baseline = create_test_baseline();

        // First computation
        calculator.compute(&baseline, Some(3.0), Some(10.0), 7.0, None);
        let first_smoothed = calculator.smoothed_deltas().h_bytes;

        // Second computation with different value
        calculator.compute(&baseline, Some(3.0), Some(10.0), 5.0, None);
        let second_smoothed = calculator.smoothed_deltas().h_bytes;

        // Smoothed value should be between the two raw deltas
        assert!(second_smoothed.abs() < first_smoothed.abs());
    }

    #[test]
    fn test_missing_values() {
        let config = DeltaConfig {
            smoothing: crate::config::SmoothingConfig {
                enabled: false,
                alpha: 0.2,
            },
            ..Default::default()
        };
        let mut calculator = DeltaCalculator::new(config);
        let baseline = create_test_baseline();

        let (deltas, z_scores) = calculator.compute(&baseline, None, None, 6.0, None);

        // Only h_bytes should have values
        assert!(deltas.tc.is_none());
        assert!(deltas.h_joint.is_none());
        assert!(deltas.r.is_none());
        assert!(z_scores.tc.is_none());
        assert!(z_scores.h_joint.is_none());
        assert!(z_scores.r.is_none());
    }

    #[test]
    fn test_zero_std() {
        let config = DeltaConfig::default();
        let mut calculator = DeltaCalculator::new(config);

        let mut baseline = create_test_baseline();
        baseline.h_bytes.std = 0.0; // Zero std

        let (_, z_scores) = calculator.compute(&baseline, None, None, 6.0, None);

        // Z-score should be 0 when std is 0
        assert_eq!(z_scores.h_bytes, 0.0);
    }
}
