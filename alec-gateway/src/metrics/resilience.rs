// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Resilience index (R) and criticality computation.

use super::config::ResilienceConfig;
use super::signal::SignalMetrics;

/// Resilience zone classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResilienceZone {
    Healthy,
    Attention,
    Critical,
}

impl ResilienceZone {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResilienceZone::Healthy => "healthy",
            ResilienceZone::Attention => "attention",
            ResilienceZone::Critical => "critical",
        }
    }
}

/// Resilience metrics result.
#[derive(Debug, Clone)]
pub struct ResilienceMetrics {
    /// Normalized redundancy index R ∈ [0, 1].
    pub r: f64,
    /// Zone classification based on thresholds.
    pub zone: ResilienceZone,
    /// Per-channel criticality ranking (if enabled).
    pub criticality: Option<Vec<ChannelCriticality>>,
}

#[derive(Debug, Clone)]
pub struct ChannelCriticality {
    pub channel_id: String,
    /// ΔR = R_all - R_without_this_channel
    pub delta_r: f64,
}

/// Resilience calculator.
pub struct ResilienceCalculator {
    config: ResilienceConfig,
}

impl ResilienceCalculator {
    pub fn new(config: ResilienceConfig) -> Self {
        Self { config }
    }

    /// Compute resilience metrics from signal metrics.
    pub fn compute(&self, signal: &SignalMetrics) -> Option<ResilienceMetrics> {
        if !self.config.enabled {
            return None;
        }

        // Check minimum entropy threshold
        if signal.sum_h < self.config.min_sum_h {
            return None;
        }

        // R = TC / Σ H_i = 1 - H_joint / Σ H_i
        let r = if signal.sum_h > 0.0 {
            (signal.total_correlation / signal.sum_h).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let zone = self.classify_zone(r);

        // Criticality is computed separately (expensive)
        let criticality = None;

        Some(ResilienceMetrics {
            r,
            zone,
            criticality,
        })
    }

    /// Compute criticality ranking via leave-one-out analysis.
    /// This is expensive and should be rate-limited.
    pub fn compute_criticality(
        &self,
        signal: &SignalMetrics,
        r_all: f64,
    ) -> Option<Vec<ChannelCriticality>> {
        if !self.config.criticality.enabled {
            return None;
        }

        let n = signal.h_per_channel.len();
        if n <= 1 || n > self.config.criticality.max_channels {
            return None;
        }

        let mut criticality = Vec::with_capacity(n);

        for channel in &signal.h_per_channel {
            // Compute R without this channel
            // R_without_k = (TC - contribution_k) / (sum_h - h_k)
            // Approximation: assume TC scales proportionally
            let sum_h_without = signal.sum_h - channel.h;
            if sum_h_without > self.config.min_sum_h {
                // Simplified: assume TC contribution is proportional
                // More accurate would require recomputing joint entropy
                let tc_without_approx = signal.total_correlation * (sum_h_without / signal.sum_h);
                let r_without = tc_without_approx / sum_h_without;
                let delta_r = r_all - r_without;

                criticality.push(ChannelCriticality {
                    channel_id: channel.channel_id.clone(),
                    delta_r,
                });
            }
        }

        // Sort by absolute delta_r descending (most critical first)
        criticality.sort_by(|a, b| {
            b.delta_r
                .abs()
                .partial_cmp(&a.delta_r.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Some(criticality)
    }

    fn classify_zone(&self, r: f64) -> ResilienceZone {
        let thresholds = &self.config.thresholds;
        if r >= thresholds.healthy_min {
            ResilienceZone::Healthy
        } else if r >= thresholds.attention_min {
            ResilienceZone::Attention
        } else {
            ResilienceZone::Critical
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::signal::ChannelEntropy;

    fn create_test_signal() -> SignalMetrics {
        SignalMetrics {
            h_per_channel: vec![
                ChannelEntropy {
                    channel_id: "ch1".to_string(),
                    h: 2.0,
                },
                ChannelEntropy {
                    channel_id: "ch2".to_string(),
                    h: 2.0,
                },
                ChannelEntropy {
                    channel_id: "ch3".to_string(),
                    h: 2.0,
                },
            ],
            sum_h: 6.0,
            h_joint: 4.0,
            total_correlation: 2.0, // TC = sum_h - h_joint = 6 - 4 = 2
            aligned_samples: 100,
            channels_included: 3,
        }
    }

    #[test]
    fn test_resilience_disabled() {
        let config = ResilienceConfig {
            enabled: false,
            ..Default::default()
        };
        let calculator = ResilienceCalculator::new(config);
        let signal = create_test_signal();

        let result = calculator.compute(&signal);
        assert!(result.is_none());
    }

    #[test]
    fn test_resilience_enabled() {
        let config = ResilienceConfig {
            enabled: true,
            ..Default::default()
        };
        let calculator = ResilienceCalculator::new(config);
        let signal = create_test_signal();

        let result = calculator.compute(&signal).unwrap();
        // R = TC / sum_h = 2 / 6 ≈ 0.333
        assert!((result.r - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_zone_classification_healthy() {
        let config = ResilienceConfig {
            enabled: true,
            thresholds: super::super::config::ResilienceThresholds {
                healthy_min: 0.5,
                attention_min: 0.2,
            },
            ..Default::default()
        };
        let calculator = ResilienceCalculator::new(config);

        // Create signal with high TC (high redundancy)
        let signal = SignalMetrics {
            h_per_channel: vec![],
            sum_h: 10.0,
            h_joint: 4.0,
            total_correlation: 6.0, // R = 0.6 (healthy)
            aligned_samples: 100,
            channels_included: 2,
        };

        let result = calculator.compute(&signal).unwrap();
        assert_eq!(result.zone, ResilienceZone::Healthy);
    }

    #[test]
    fn test_zone_classification_attention() {
        let config = ResilienceConfig {
            enabled: true,
            thresholds: super::super::config::ResilienceThresholds {
                healthy_min: 0.5,
                attention_min: 0.2,
            },
            ..Default::default()
        };
        let calculator = ResilienceCalculator::new(config);

        let signal = SignalMetrics {
            h_per_channel: vec![],
            sum_h: 10.0,
            h_joint: 7.0,
            total_correlation: 3.0, // R = 0.3 (attention)
            aligned_samples: 100,
            channels_included: 2,
        };

        let result = calculator.compute(&signal).unwrap();
        assert_eq!(result.zone, ResilienceZone::Attention);
    }

    #[test]
    fn test_zone_classification_critical() {
        let config = ResilienceConfig {
            enabled: true,
            thresholds: super::super::config::ResilienceThresholds {
                healthy_min: 0.5,
                attention_min: 0.2,
            },
            ..Default::default()
        };
        let calculator = ResilienceCalculator::new(config);

        let signal = SignalMetrics {
            h_per_channel: vec![],
            sum_h: 10.0,
            h_joint: 9.0,
            total_correlation: 1.0, // R = 0.1 (critical)
            aligned_samples: 100,
            channels_included: 2,
        };

        let result = calculator.compute(&signal).unwrap();
        assert_eq!(result.zone, ResilienceZone::Critical);
    }

    #[test]
    fn test_min_sum_h_threshold() {
        let config = ResilienceConfig {
            enabled: true,
            min_sum_h: 1.0,
            ..Default::default()
        };
        let calculator = ResilienceCalculator::new(config);

        let signal = SignalMetrics {
            h_per_channel: vec![],
            sum_h: 0.05, // Below threshold
            h_joint: 0.04,
            total_correlation: 0.01,
            aligned_samples: 100,
            channels_included: 2,
        };

        let result = calculator.compute(&signal);
        assert!(result.is_none());
    }

    #[test]
    fn test_r_clamped_to_valid_range() {
        let config = ResilienceConfig {
            enabled: true,
            ..Default::default()
        };
        let calculator = ResilienceCalculator::new(config);

        // Edge case: sum_h > 0 but TC could be higher due to numerical issues
        let signal = SignalMetrics {
            h_per_channel: vec![],
            sum_h: 1.0,
            h_joint: 0.1,
            total_correlation: 1.5, // Artificially high
            aligned_samples: 100,
            channels_included: 2,
        };

        let result = calculator.compute(&signal).unwrap();
        assert!(result.r >= 0.0 && result.r <= 1.0);
    }

    #[test]
    fn test_criticality_computation() {
        let config = ResilienceConfig {
            enabled: true,
            criticality: super::super::config::CriticalityConfig {
                enabled: true,
                max_channels: 16,
                every_n_signal_computes: 1,
            },
            ..Default::default()
        };
        let calculator = ResilienceCalculator::new(config);
        let signal = create_test_signal();

        let r = signal.total_correlation / signal.sum_h;
        let criticality = calculator.compute_criticality(&signal, r).unwrap();

        assert_eq!(criticality.len(), 3);
    }

    #[test]
    fn test_criticality_disabled() {
        let config = ResilienceConfig {
            enabled: true,
            criticality: super::super::config::CriticalityConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let calculator = ResilienceCalculator::new(config);
        let signal = create_test_signal();

        let criticality = calculator.compute_criticality(&signal, 0.33);
        assert!(criticality.is_none());
    }

    #[test]
    fn test_criticality_sorted_by_delta_r() {
        let config = ResilienceConfig {
            enabled: true,
            criticality: super::super::config::CriticalityConfig {
                enabled: true,
                max_channels: 16,
                every_n_signal_computes: 1,
            },
            min_sum_h: 0.1,
            ..Default::default()
        };
        let calculator = ResilienceCalculator::new(config);

        let signal = SignalMetrics {
            h_per_channel: vec![
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
            ],
            sum_h: 6.0,
            h_joint: 4.0,
            total_correlation: 2.0,
            aligned_samples: 100,
            channels_included: 3,
        };

        let r = signal.total_correlation / signal.sum_h;
        let criticality = calculator.compute_criticality(&signal, r).unwrap();

        // Should be sorted by absolute delta_r descending
        for i in 1..criticality.len() {
            assert!(criticality[i - 1].delta_r.abs() >= criticality[i].delta_r.abs());
        }
    }

    #[test]
    fn test_zone_as_str() {
        assert_eq!(ResilienceZone::Healthy.as_str(), "healthy");
        assert_eq!(ResilienceZone::Attention.as_str(), "attention");
        assert_eq!(ResilienceZone::Critical.as_str(), "critical");
    }
}
