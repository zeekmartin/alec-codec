// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Payload-level entropy computation from frame bytes.

use super::config::PayloadMetricsConfig;

/// Payload-level metrics result.
#[derive(Debug, Clone)]
pub struct PayloadMetrics {
    /// Total frame size in bytes.
    pub frame_size_bytes: usize,
    /// Shannon entropy over frame bytes (bits).
    pub h_bytes: f64,
    /// Optional: byte histogram (256 bins).
    pub histogram: Option<[u32; 256]>,
    /// Optional: per-channel payload metrics.
    pub per_channel: Option<Vec<ChannelPayloadMetrics>>,
}

#[derive(Debug, Clone)]
pub struct ChannelPayloadMetrics {
    pub channel_id: String,
    pub size_bytes: usize,
    pub h_bytes: f64,
}

/// Payload entropy calculator.
pub struct PayloadEntropyCalculator {
    config: PayloadMetricsConfig,
}

impl PayloadEntropyCalculator {
    pub fn new(config: PayloadMetricsConfig) -> Self {
        Self { config }
    }

    /// Compute payload metrics from frame bytes.
    pub fn compute(&self, frame_bytes: &[u8]) -> PayloadMetrics {
        let frame_size_bytes = frame_bytes.len();
        let (h_bytes, histogram) = self.byte_entropy(frame_bytes);

        PayloadMetrics {
            frame_size_bytes,
            h_bytes,
            histogram: if self.config.include_histogram {
                Some(histogram)
            } else {
                None
            },
            per_channel: None, // Populated separately if needed
        }
    }

    /// Compute payload metrics with per-channel breakdown.
    #[allow(dead_code)]
    pub fn compute_with_channels(
        &self,
        frame_bytes: &[u8],
        channel_data: &[(String, &[u8])],
    ) -> PayloadMetrics {
        let mut metrics = self.compute(frame_bytes);

        if self.config.per_channel_entropy {
            let per_channel = channel_data
                .iter()
                .map(|(id, data)| {
                    let (h, _) = self.byte_entropy(data);
                    ChannelPayloadMetrics {
                        channel_id: id.clone(),
                        size_bytes: data.len(),
                        h_bytes: h,
                    }
                })
                .collect();
            metrics.per_channel = Some(per_channel);
        }

        metrics
    }

    /// Compute Shannon entropy over byte distribution.
    /// Returns (entropy_bits, histogram).
    fn byte_entropy(&self, data: &[u8]) -> (f64, [u32; 256]) {
        let mut histogram = [0u32; 256];

        for &byte in data {
            histogram[byte as usize] += 1;
        }

        if data.is_empty() {
            return (0.0, histogram);
        }

        let n = data.len() as f64;
        let mut entropy = 0.0;

        for &count in &histogram {
            if count > 0 {
                let p = count as f64 / n;
                entropy -= p * p.log2();
            }
        }

        (entropy, histogram)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculator_creation() {
        let config = PayloadMetricsConfig::default();
        let calculator = PayloadEntropyCalculator::new(config);
        assert!(calculator.config.frame_entropy);
    }

    #[test]
    fn test_empty_payload() {
        let config = PayloadMetricsConfig::default();
        let calculator = PayloadEntropyCalculator::new(config);

        let metrics = calculator.compute(&[]);
        assert_eq!(metrics.frame_size_bytes, 0);
        assert_eq!(metrics.h_bytes, 0.0);
    }

    #[test]
    fn test_uniform_distribution() {
        let config = PayloadMetricsConfig::default();
        let calculator = PayloadEntropyCalculator::new(config);

        // All same bytes = zero entropy
        let data = vec![0u8; 100];
        let metrics = calculator.compute(&data);
        assert_eq!(metrics.h_bytes, 0.0);
    }

    #[test]
    fn test_maximum_entropy() {
        let config = PayloadMetricsConfig::default();
        let calculator = PayloadEntropyCalculator::new(config);

        // 256 different bytes, each appearing once = max entropy (8 bits)
        let data: Vec<u8> = (0..=255).collect();
        let metrics = calculator.compute(&data);

        // Maximum entropy for 256 symbols is log2(256) = 8 bits
        assert!((metrics.h_bytes - 8.0).abs() < 0.001);
    }

    #[test]
    fn test_binary_distribution() {
        let config = PayloadMetricsConfig::default();
        let calculator = PayloadEntropyCalculator::new(config);

        // 50% zeros, 50% ones = 1 bit entropy
        let mut data = vec![0u8; 100];
        data.extend(vec![1u8; 100]);
        let metrics = calculator.compute(&data);

        // H = -0.5*log2(0.5) - 0.5*log2(0.5) = 1 bit
        assert!((metrics.h_bytes - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_histogram_included() {
        let config = PayloadMetricsConfig {
            include_histogram: true,
            ..Default::default()
        };
        let calculator = PayloadEntropyCalculator::new(config);

        let data = vec![0u8, 1, 2, 0, 1, 0];
        let metrics = calculator.compute(&data);

        let histogram = metrics.histogram.unwrap();
        assert_eq!(histogram[0], 3);
        assert_eq!(histogram[1], 2);
        assert_eq!(histogram[2], 1);
    }

    #[test]
    fn test_histogram_excluded() {
        let config = PayloadMetricsConfig {
            include_histogram: false,
            ..Default::default()
        };
        let calculator = PayloadEntropyCalculator::new(config);

        let data = vec![0u8, 1, 2];
        let metrics = calculator.compute(&data);
        assert!(metrics.histogram.is_none());
    }

    #[test]
    fn test_per_channel_entropy() {
        let config = PayloadMetricsConfig {
            per_channel_entropy: true,
            ..Default::default()
        };
        let calculator = PayloadEntropyCalculator::new(config);

        let frame_bytes = [1u8, 2, 3, 4, 5, 6];
        let ch1_data = [1u8, 2, 3];
        let ch2_data = [4u8, 5, 6];

        let channel_data = vec![
            ("ch1".to_string(), ch1_data.as_slice()),
            ("ch2".to_string(), ch2_data.as_slice()),
        ];

        let metrics = calculator.compute_with_channels(&frame_bytes, &channel_data);
        let per_channel = metrics.per_channel.unwrap();

        assert_eq!(per_channel.len(), 2);
        assert_eq!(per_channel[0].channel_id, "ch1");
        assert_eq!(per_channel[0].size_bytes, 3);
    }

    #[test]
    fn test_frame_size() {
        let config = PayloadMetricsConfig::default();
        let calculator = PayloadEntropyCalculator::new(config);

        let data = vec![0u8; 42];
        let metrics = calculator.compute(&data);
        assert_eq!(metrics.frame_size_bytes, 42);
    }
}
