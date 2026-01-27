// ALEC Testdata - Dataset manifest
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Dataset manifest for describing pre-generated datasets.
//!
//! Manifests provide metadata about datasets, including expected
//! metrics ranges and anomaly information.

use crate::anomalies::AnomalyConfig;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Dataset manifest describing a pre-generated dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetManifest {
    /// Dataset name (matches filename without extension).
    pub name: String,
    /// Industry/domain.
    pub industry: String,
    /// Human-readable description.
    pub description: String,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Number of samples.
    pub sample_count: usize,
    /// Sample interval in milliseconds.
    pub sample_interval_ms: u64,
    /// Sensor definitions.
    pub sensors: Vec<SensorManifest>,
    /// Anomalies present in the dataset.
    #[serde(default)]
    pub anomalies: Vec<AnomalyManifest>,
    /// Expected metrics ranges.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_metrics: Option<ExpectedMetrics>,
    /// Generation timestamp.
    pub generated_at: DateTime<Utc>,
    /// Random seed used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

/// Sensor information in manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorManifest {
    /// Sensor identifier.
    pub id: String,
    /// Unit of measurement.
    pub unit: String,
    /// Minimum value in dataset.
    pub min: f64,
    /// Maximum value in dataset.
    pub max: f64,
    /// Pattern description.
    pub pattern: String,
    /// Expected entropy range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_entropy_range: Option<(f64, f64)>,
}

/// Anomaly information in manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyManifest {
    /// Sensor affected.
    pub sensor_id: String,
    /// Anomaly type name.
    pub anomaly_type: String,
    /// Start sample index.
    pub start_sample: usize,
    /// Duration in samples (None = until end).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_samples: Option<usize>,
    /// Expected detection event type.
    pub expected_event: String,
}

/// Expected metrics ranges for validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedMetrics {
    /// Expected compression ratio range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_ratio_range: Option<(f64, f64)>,
    /// Expected Total Correlation range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tc_range: Option<(f64, f64)>,
    /// Expected Resilience (R) range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_range: Option<(f64, f64)>,
    /// Expected H_bytes range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h_bytes_range: Option<(f64, f64)>,
}

impl DatasetManifest {
    /// Create a new manifest.
    pub fn new(name: &str, industry: &str) -> Self {
        Self {
            name: name.to_string(),
            industry: industry.to_string(),
            description: String::new(),
            duration_ms: 0,
            sample_count: 0,
            sample_interval_ms: 60_000,
            sensors: Vec::new(),
            anomalies: Vec::new(),
            expected_metrics: None,
            generated_at: Utc::now(),
            seed: None,
        }
    }

    /// Set description.
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// Set timing information.
    pub fn with_timing(mut self, duration_ms: u64, sample_count: usize, interval_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self.sample_count = sample_count;
        self.sample_interval_ms = interval_ms;
        self
    }

    /// Add a sensor.
    pub fn add_sensor(mut self, sensor: SensorManifest) -> Self {
        self.sensors.push(sensor);
        self
    }

    /// Add an anomaly.
    pub fn add_anomaly(mut self, anomaly: AnomalyManifest) -> Self {
        self.anomalies.push(anomaly);
        self
    }

    /// Set expected metrics.
    pub fn with_expected_metrics(mut self, metrics: ExpectedMetrics) -> Self {
        self.expected_metrics = Some(metrics);
        self
    }

    /// Set seed.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Save to JSON file.
    pub fn to_json_file(&self, path: impl AsRef<std::path::Path>) -> Result<(), std::io::Error> {
        let json = self.to_json().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        std::fs::write(path, json)
    }

    /// Load from JSON file.
    pub fn from_json_file(path: impl AsRef<std::path::Path>) -> Result<Self, std::io::Error> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })
    }
}

impl SensorManifest {
    /// Create a new sensor manifest.
    pub fn new(id: &str, unit: &str, min: f64, max: f64, pattern: &str) -> Self {
        Self {
            id: id.to_string(),
            unit: unit.to_string(),
            min,
            max,
            pattern: pattern.to_string(),
            expected_entropy_range: None,
        }
    }

    /// Set expected entropy range.
    pub fn with_entropy_range(mut self, min: f64, max: f64) -> Self {
        self.expected_entropy_range = Some((min, max));
        self
    }
}

impl AnomalyManifest {
    /// Create from anomaly config.
    pub fn from_config(sensor_id: &str, config: &AnomalyConfig) -> Self {
        Self {
            sensor_id: sensor_id.to_string(),
            anomaly_type: format!("{:?}", config.anomaly_type),
            start_sample: config.start_sample,
            duration_samples: config.duration_samples,
            expected_event: config.anomaly_type.expected_event().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_creation() {
        let manifest = DatasetManifest::new("test_dataset", "agriculture")
            .with_description("Test dataset")
            .with_timing(86400000, 1440, 60000)
            .with_seed(42)
            .add_sensor(SensorManifest::new("temp", "°C", 10.0, 35.0, "diurnal"));

        assert_eq!(manifest.name, "test_dataset");
        assert_eq!(manifest.industry, "agriculture");
        assert_eq!(manifest.sample_count, 1440);
        assert_eq!(manifest.sensors.len(), 1);
    }

    #[test]
    fn test_manifest_json() {
        let manifest = DatasetManifest::new("test", "testing")
            .with_description("Test")
            .with_timing(60000, 60, 1000);

        let json = manifest.to_json().unwrap();
        assert!(json.contains("\"name\": \"test\""));
        assert!(json.contains("\"industry\": \"testing\""));
    }

    #[test]
    fn test_sensor_manifest() {
        let sensor = SensorManifest::new("temp", "°C", 10.0, 35.0, "sine + noise")
            .with_entropy_range(2.5, 3.5);

        assert_eq!(sensor.id, "temp");
        assert_eq!(sensor.expected_entropy_range, Some((2.5, 3.5)));
    }
}
