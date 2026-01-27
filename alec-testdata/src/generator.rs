// ALEC Testdata - Core generator
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Core dataset generation logic.
//!
//! This module provides the main generation API for creating
//! realistic sensor datasets.

use crate::anomalies::{AnomalyConfig, AnomalyState};
use crate::dataset::{Dataset, DatasetMetadata, DatasetRow};
use crate::patterns::{PatternState, SignalPattern};
use rand::prelude::*;
use rand::rngs::StdRng;
use rand_distr::Normal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Generator configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorConfig {
    /// Start timestamp in milliseconds.
    pub start_time_ms: u64,
    /// Interval between samples in milliseconds.
    pub sample_interval_ms: u64,
    /// Number of samples to generate.
    pub num_samples: usize,
    /// Random seed for reproducibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            start_time_ms: 1706745600000, // 2024-02-01 00:00:00 UTC
            sample_interval_ms: 60_000,   // 1 minute
            num_samples: 60,              // 1 hour
            seed: None,
        }
    }
}

impl GeneratorConfig {
    /// Create a new generator config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set start timestamp.
    pub fn with_start_time(mut self, timestamp_ms: u64) -> Self {
        self.start_time_ms = timestamp_ms;
        self
    }

    /// Set sample interval.
    pub fn with_sample_interval_ms(mut self, interval_ms: u64) -> Self {
        self.sample_interval_ms = interval_ms;
        self
    }

    /// Set sample interval in seconds.
    pub fn with_sample_interval_secs(mut self, secs: u64) -> Self {
        self.sample_interval_ms = secs * 1000;
        self
    }

    /// Set number of samples.
    pub fn with_num_samples(mut self, n: usize) -> Self {
        self.num_samples = n;
        self
    }

    /// Set duration in hours (calculates num_samples from interval).
    pub fn with_duration_hours(mut self, hours: f64) -> Self {
        let total_ms = hours * 3_600_000.0;
        self.num_samples = (total_ms / self.sample_interval_ms as f64).ceil() as usize;
        self
    }

    /// Set duration in minutes.
    pub fn with_duration_minutes(mut self, minutes: f64) -> Self {
        let total_ms = minutes * 60_000.0;
        self.num_samples = (total_ms / self.sample_interval_ms as f64).ceil() as usize;
        self
    }

    /// Set random seed.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Get total duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.sample_interval_ms * (self.num_samples.saturating_sub(1)) as u64
    }

    /// Get end timestamp.
    pub fn end_time_ms(&self) -> u64 {
        self.start_time_ms + self.duration_ms()
    }
}

/// Sensor configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorConfig {
    /// Sensor identifier.
    pub id: String,
    /// Unit of measurement.
    pub unit: String,
    /// Minimum valid value.
    pub min: f64,
    /// Maximum valid value.
    pub max: f64,
    /// Signal pattern.
    pub pattern: SignalPattern,
    /// Standard deviation of added noise.
    pub noise_std: f64,
    /// Anomaly configuration (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomaly: Option<AnomalyConfig>,
    /// Correlation with another sensor.
    #[serde(skip)]
    pub correlation: Option<SensorCorrelation>,
}

/// Correlation configuration between sensors.
#[derive(Debug, Clone)]
pub struct SensorCorrelation {
    /// Source sensor ID to correlate with.
    pub source_id: String,
    /// Correlation coefficient (-1 to 1).
    pub coefficient: f64,
    /// Lag in samples (0 = same time).
    pub lag_samples: usize,
}

impl SensorConfig {
    /// Create a new sensor config.
    pub fn new(id: &str, unit: &str, min: f64, max: f64, pattern: SignalPattern) -> Self {
        Self {
            id: id.to_string(),
            unit: unit.to_string(),
            min,
            max,
            pattern,
            noise_std: 0.0,
            anomaly: None,
            correlation: None,
        }
    }

    /// Add noise to the sensor.
    pub fn with_noise(mut self, std: f64) -> Self {
        self.noise_std = std;
        self
    }

    /// Add an anomaly.
    pub fn with_anomaly(mut self, anomaly: AnomalyConfig) -> Self {
        self.anomaly = Some(anomaly);
        self
    }

    /// Add correlation with another sensor.
    pub fn with_correlation(mut self, source_id: &str, coefficient: f64) -> Self {
        self.correlation = Some(SensorCorrelation {
            source_id: source_id.to_string(),
            coefficient,
            lag_samples: 0,
        });
        self
    }

    /// Add correlation with lag.
    pub fn with_lagged_correlation(
        mut self,
        source_id: &str,
        coefficient: f64,
        lag_samples: usize,
    ) -> Self {
        self.correlation = Some(SensorCorrelation {
            source_id: source_id.to_string(),
            coefficient,
            lag_samples,
        });
        self
    }
}

/// Generate a dataset from configuration.
pub fn generate_dataset(config: &GeneratorConfig, sensors: &[SensorConfig]) -> Dataset {
    let mut rng: Box<dyn RngCore> = match config.seed {
        Some(s) => Box::new(StdRng::seed_from_u64(s)),
        None => Box::new(StdRng::from_entropy()),
    };

    let sensor_ids: Vec<String> = sensors.iter().map(|s| s.id.clone()).collect();
    let mut dataset = Dataset::new(sensor_ids);

    dataset.metadata = DatasetMetadata {
        name: None,
        industry: None,
        description: None,
        seed: config.seed,
        sample_interval_ms: Some(config.sample_interval_ms),
    };

    // Initialize pattern states
    let mut pattern_states: HashMap<String, PatternState> = sensors
        .iter()
        .map(|s| (s.id.clone(), PatternState::for_pattern(&s.pattern)))
        .collect();

    // Initialize anomaly states
    let mut anomaly_states: HashMap<String, AnomalyState> = sensors
        .iter()
        .map(|s| (s.id.clone(), AnomalyState::default()))
        .collect();

    // History for correlations
    let mut history: HashMap<String, Vec<f64>> =
        sensors.iter().map(|s| (s.id.clone(), Vec::new())).collect();

    // Generate samples
    for i in 0..config.num_samples {
        let timestamp = config.start_time_ms + (i as u64 * config.sample_interval_ms);
        // Use relative time for pattern evaluation (time since start)
        let relative_time = i as u64 * config.sample_interval_ms;
        let mut row = DatasetRow::new(timestamp);

        // First pass: generate base values for non-correlated sensors
        let mut base_values: HashMap<String, f64> = HashMap::new();

        for sensor in sensors {
            if sensor.correlation.is_none() {
                let state = pattern_states.get_mut(&sensor.id).unwrap();
                let base = state.evaluate(&sensor.pattern, relative_time, &mut *rng);
                base_values.insert(sensor.id.clone(), base);
            }
        }

        // Second pass: generate correlated values
        for sensor in sensors {
            if let Some(ref corr) = sensor.correlation {
                let source_value = match (corr.lag_samples > 0, history.get(&corr.source_id)) {
                    (true, Some(hist)) if hist.len() > corr.lag_samples => {
                        hist[hist.len() - corr.lag_samples - 1]
                    }
                    _ => *base_values.get(&corr.source_id).unwrap_or(&0.0),
                };

                // Apply correlation
                let base = apply_correlation(
                    source_value,
                    corr.coefficient,
                    sensor.min,
                    sensor.max,
                    &mut *rng,
                );
                base_values.insert(sensor.id.clone(), base);
            }
        }

        // Third pass: apply noise, anomalies, and clamp
        for sensor in sensors {
            let mut value = *base_values.get(&sensor.id).unwrap_or(&0.0);

            // Add noise
            if sensor.noise_std > 0.0 {
                let noise_dist = Normal::new(0.0, sensor.noise_std).unwrap();
                value += noise_dist.sample(&mut *rng);
            }

            // Apply anomaly
            let final_value = if let Some(ref anomaly) = sensor.anomaly {
                if anomaly.is_active(i) {
                    let state = anomaly_states.get_mut(&sensor.id).unwrap();
                    let samples_since = anomaly.samples_since_start(i);
                    state.apply(&anomaly.anomaly_type, value, samples_since, &mut *rng)
                } else {
                    Some(value)
                }
            } else {
                Some(value)
            };

            // Clamp to valid range and store
            let final_value = final_value.map(|v| v.clamp(sensor.min, sensor.max));
            row.values.insert(sensor.id.clone(), final_value);

            // Update history for correlations
            if let Some(v) = final_value {
                history.get_mut(&sensor.id).unwrap().push(v);
            }
        }

        dataset.add_row(row);
    }

    dataset
}

/// Apply correlation transformation.
fn apply_correlation(
    source_value: f64,
    coefficient: f64,
    target_min: f64,
    target_max: f64,
    rng: &mut dyn RngCore,
) -> f64 {
    // Normalize source to [0, 1]
    // We don't know source range, so use the value directly with some transformation

    // For negative correlation, invert
    let base = if coefficient < 0.0 {
        target_max - (source_value - target_min) * coefficient.abs()
    } else {
        target_min + (source_value - target_min) * coefficient
    };

    // Add some independent noise based on correlation strength
    // Higher correlation = less noise
    let noise_factor = (1.0 - coefficient.abs()).sqrt();
    let range = target_max - target_min;
    let noise = (rng.gen::<f64>() - 0.5) * range * noise_factor * 0.2;

    base + noise
}

/// Builder for creating datasets with common patterns.
pub struct DatasetBuilder {
    config: GeneratorConfig,
    sensors: Vec<SensorConfig>,
    name: Option<String>,
    industry: Option<String>,
    description: Option<String>,
}

impl DatasetBuilder {
    /// Create a new dataset builder.
    pub fn new() -> Self {
        Self {
            config: GeneratorConfig::default(),
            sensors: Vec::new(),
            name: None,
            industry: None,
            description: None,
        }
    }

    /// Set generator configuration.
    pub fn with_config(mut self, config: GeneratorConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a sensor.
    pub fn add_sensor(mut self, sensor: SensorConfig) -> Self {
        self.sensors.push(sensor);
        self
    }

    /// Add multiple sensors.
    pub fn add_sensors(mut self, sensors: impl IntoIterator<Item = SensorConfig>) -> Self {
        self.sensors.extend(sensors);
        self
    }

    /// Set dataset name.
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Set industry.
    pub fn with_industry(mut self, industry: &str) -> Self {
        self.industry = Some(industry.to_string());
        self
    }

    /// Set description.
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    /// Build the dataset.
    pub fn build(self) -> Dataset {
        let mut dataset = generate_dataset(&self.config, &self.sensors);
        dataset.metadata.name = self.name;
        dataset.metadata.industry = self.industry;
        dataset.metadata.description = self.description;
        dataset
    }
}

impl Default for DatasetBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anomalies::AnomalyType;

    #[test]
    fn test_generator_config_default() {
        let config = GeneratorConfig::default();
        assert_eq!(config.sample_interval_ms, 60_000);
        assert_eq!(config.num_samples, 60);
    }

    #[test]
    fn test_generator_config_duration() {
        let config = GeneratorConfig::new()
            .with_sample_interval_ms(60_000)
            .with_duration_hours(24.0);

        assert_eq!(config.num_samples, 1440);
    }

    #[test]
    fn test_generate_constant() {
        let config = GeneratorConfig::new().with_num_samples(10).with_seed(42);

        let sensors = vec![SensorConfig::new(
            "temp",
            "°C",
            0.0,
            100.0,
            SignalPattern::Constant { value: 25.0 },
        )];

        let dataset = generate_dataset(&config, &sensors);

        assert_eq!(dataset.len(), 10);
        for row in dataset.rows() {
            let v = row.get("temp").unwrap();
            assert!((v - 25.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_generate_with_noise() {
        let config = GeneratorConfig::new().with_num_samples(100).with_seed(42);

        let sensors = vec![SensorConfig::new(
            "temp",
            "°C",
            0.0,
            100.0,
            SignalPattern::Constant { value: 50.0 },
        )
        .with_noise(5.0)];

        let dataset = generate_dataset(&config, &sensors);

        // Check that values vary (noise is applied)
        let values: Vec<f64> = dataset.column("temp").into_iter().flatten().collect();
        let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
        let variance: f64 =
            values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;

        // Mean should be close to 50
        assert!((mean - 50.0).abs() < 2.0);
        // Variance should be positive (noise present)
        assert!(variance > 1.0);
    }

    #[test]
    fn test_generate_with_anomaly() {
        let config = GeneratorConfig::new().with_num_samples(100).with_seed(42);

        let anomaly = AnomalyConfig::new(AnomalyType::Stuck, 50).with_duration(30);

        let sensors = vec![SensorConfig::new(
            "temp",
            "°C",
            0.0,
            100.0,
            SignalPattern::Linear {
                start: 20.0,
                slope_per_ms: 0.0001,
            },
        )
        .with_anomaly(anomaly)];

        let dataset = generate_dataset(&config, &sensors);

        // Before anomaly, values should increase
        let v49 = dataset.rows()[49].get("temp").unwrap();
        let v0 = dataset.rows()[0].get("temp").unwrap();
        assert!(v49 > v0);

        // During anomaly, values should be stuck
        let v50 = dataset.rows()[50].get("temp").unwrap();
        let v60 = dataset.rows()[60].get("temp").unwrap();
        assert!((v50 - v60).abs() < 0.001);
    }

    #[test]
    fn test_generate_sine() {
        let config = GeneratorConfig::new()
            .with_sample_interval_ms(1000)
            .with_num_samples(100)
            .with_seed(42);

        let sensors = vec![SensorConfig::new(
            "signal",
            "V",
            -10.0,
            10.0,
            SignalPattern::Sine {
                amplitude: 5.0,
                period_ms: 10_000,
                phase: 0.0,
                offset: 0.0,
            },
        )];

        let dataset = generate_dataset(&config, &sensors);

        // At t=2500ms (quarter period), sin(PI/2) = 1, value = 5
        // But sample 2 is at t=2000ms and sample 3 is at t=3000ms
        // Let's check sample 2 (t=2000ms): sin(2*PI*2000/10000) = sin(0.4*PI) ≈ 0.95
        let v2 = dataset.rows()[2].get("signal").unwrap();
        assert!(v2 > 4.0); // Should be close to 5

        // At sample 5 (t=5000ms): sin(2*PI*5000/10000) = sin(PI) = 0
        let v5 = dataset.rows()[5].get("signal").unwrap();
        assert!(v5.abs() < 0.1);
    }

    #[test]
    fn test_dataset_builder() {
        let dataset = DatasetBuilder::new()
            .with_config(GeneratorConfig::new().with_num_samples(10).with_seed(42))
            .add_sensor(SensorConfig::new(
                "temp",
                "°C",
                0.0,
                50.0,
                SignalPattern::Constant { value: 25.0 },
            ))
            .with_name("test_dataset")
            .with_industry("testing")
            .build();

        assert_eq!(dataset.len(), 10);
        assert_eq!(dataset.metadata.name, Some("test_dataset".to_string()));
        assert_eq!(dataset.metadata.industry, Some("testing".to_string()));
    }

    #[test]
    fn test_reproducibility() {
        let config = GeneratorConfig::new().with_num_samples(10).with_seed(12345);

        let sensors = vec![SensorConfig::new(
            "temp",
            "°C",
            0.0,
            100.0,
            SignalPattern::Constant { value: 50.0 },
        )
        .with_noise(10.0)];

        let dataset1 = generate_dataset(&config, &sensors);
        let dataset2 = generate_dataset(&config, &sensors);

        // Same seed should produce identical results
        for (r1, r2) in dataset1.rows().iter().zip(dataset2.rows().iter()) {
            assert_eq!(r1.get("temp"), r2.get("temp"));
        }
    }
}
