// ALEC Testdata - Anomaly scenarios
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Anomaly scenario definitions for testing detection systems.
//!
//! Scenarios describe anomaly injection patterns and expected
//! detection outcomes.

use crate::anomalies::AnomalyType;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Anomaly scenario definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyScenario {
    /// Scenario name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Whether this scenario is industry-agnostic.
    pub industry_agnostic: bool,
    /// Injection configuration.
    pub injection: ScenarioInjection,
    /// Expected detection events.
    pub expected_events: Vec<ExpectedEvent>,
    /// Validation criteria.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<ScenarioValidation>,
}

/// Scenario injection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioInjection {
    /// Type of anomaly to inject.
    #[serde(rename = "type")]
    pub anomaly_type: String,
    /// Target sensor ("any" for any sensor).
    pub target_sensor: String,
    /// Start position as percentage (0.0 - 1.0).
    pub start_percent: f64,
    /// Duration as percentage (None = until end).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_percent: Option<f64>,
    /// Additional parameters.
    #[serde(default)]
    pub params: ScenarioParams,
}

/// Additional scenario parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScenarioParams {
    /// Magnitude for spike anomalies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magnitude: Option<f64>,
    /// Rate for drift anomalies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<f64>,
    /// Noise standard deviation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub noise_std: Option<f64>,
    /// Frequency for oscillation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_hz: Option<f64>,
    /// Offset for bias shift.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<f64>,
}

/// Expected detection event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedEvent {
    /// Event type (e.g., "STRUCTURE_BREAK").
    #[serde(rename = "type")]
    pub event_type: String,
    /// Minimum samples after anomaly start to detect.
    pub min_delay_samples: usize,
    /// Maximum samples after anomaly start to detect.
    pub max_delay_samples: usize,
    /// Expected severity (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
}

/// Validation criteria for scenario testing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScenarioValidation {
    /// R should decrease during anomaly.
    #[serde(default)]
    pub r_should_decrease: bool,
    /// R should increase during anomaly.
    #[serde(default)]
    pub r_should_increase: bool,
    /// TC should change during anomaly.
    #[serde(default)]
    pub tc_should_change: bool,
    /// H_bytes should increase during anomaly.
    #[serde(default)]
    pub h_bytes_should_increase: bool,
    /// Minimum z-score expected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_z_score: Option<f64>,
}

impl AnomalyScenario {
    /// Load scenario from JSON file.
    pub fn from_json_file(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let json = fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Save scenario to JSON file.
    pub fn to_json_file(&self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, json)
    }

    /// Convert injection config to AnomalyType.
    pub fn to_anomaly_type(&self) -> Option<AnomalyType> {
        match self.injection.anomaly_type.to_lowercase().as_str() {
            "stuck" => Some(AnomalyType::Stuck),
            "spike" => Some(AnomalyType::Spike {
                magnitude: self.injection.params.magnitude.unwrap_or(10.0),
            }),
            "drift" => Some(AnomalyType::Drift {
                rate_per_sample: self.injection.params.rate.unwrap_or(0.1),
            }),
            "decorrelate" | "correlation_break" => Some(AnomalyType::Decorrelate {
                center: 0.0,
                noise_std: self.injection.params.noise_std.unwrap_or(1.0),
            }),
            "dropout" => Some(AnomalyType::Dropout),
            "oscillation" => Some(AnomalyType::Oscillation {
                amplitude: self.injection.params.magnitude.unwrap_or(5.0),
                frequency_hz: self.injection.params.frequency_hz.unwrap_or(0.1),
            }),
            "bias_shift" => Some(AnomalyType::BiasShift {
                offset: self.injection.params.offset.unwrap_or(10.0),
            }),
            "noise_increase" => Some(AnomalyType::NoiseIncrease {
                factor: self.injection.params.magnitude.unwrap_or(3.0),
            }),
            _ => None,
        }
    }

    /// Create sensor failure scenario.
    pub fn sensor_failure() -> Self {
        Self {
            name: "sensor_failure".to_string(),
            description: "Single sensor stops updating (stuck at last value)".to_string(),
            industry_agnostic: true,
            injection: ScenarioInjection {
                anomaly_type: "stuck".to_string(),
                target_sensor: "any".to_string(),
                start_percent: 0.3,
                duration_percent: Some(0.5),
                params: ScenarioParams::default(),
            },
            expected_events: vec![ExpectedEvent {
                event_type: "STRUCTURE_BREAK".to_string(),
                min_delay_samples: 10,
                max_delay_samples: 50,
                severity: Some("Warning".to_string()),
            }],
            validation: Some(ScenarioValidation {
                r_should_decrease: true,
                tc_should_change: true,
                ..Default::default()
            }),
        }
    }

    /// Create gradual drift scenario.
    pub fn gradual_drift() -> Self {
        Self {
            name: "gradual_drift".to_string(),
            description: "Sensor value gradually drifts away from normal".to_string(),
            industry_agnostic: true,
            injection: ScenarioInjection {
                anomaly_type: "drift".to_string(),
                target_sensor: "any".to_string(),
                start_percent: 0.3,
                duration_percent: None,
                params: ScenarioParams {
                    rate: Some(0.05),
                    ..Default::default()
                },
            },
            expected_events: vec![ExpectedEvent {
                event_type: "COMPLEXITY_SURGE".to_string(),
                min_delay_samples: 20,
                max_delay_samples: 100,
                severity: None,
            }],
            validation: Some(ScenarioValidation {
                tc_should_change: true,
                ..Default::default()
            }),
        }
    }

    /// Create sudden spike scenario.
    pub fn sudden_spike() -> Self {
        Self {
            name: "sudden_spike".to_string(),
            description: "Sudden large spike in sensor value".to_string(),
            industry_agnostic: true,
            injection: ScenarioInjection {
                anomaly_type: "spike".to_string(),
                target_sensor: "any".to_string(),
                start_percent: 0.5,
                duration_percent: Some(0.01),
                params: ScenarioParams {
                    magnitude: Some(50.0),
                    ..Default::default()
                },
            },
            expected_events: vec![ExpectedEvent {
                event_type: "PAYLOAD_ENTROPY_SPIKE".to_string(),
                min_delay_samples: 1,
                max_delay_samples: 10,
                severity: None,
            }],
            validation: Some(ScenarioValidation {
                h_bytes_should_increase: true,
                ..Default::default()
            }),
        }
    }

    /// Create correlation break scenario.
    pub fn correlation_break() -> Self {
        Self {
            name: "correlation_break".to_string(),
            description: "Sensor becomes decorrelated from related sensors".to_string(),
            industry_agnostic: true,
            injection: ScenarioInjection {
                anomaly_type: "decorrelate".to_string(),
                target_sensor: "any".to_string(),
                start_percent: 0.4,
                duration_percent: Some(0.3),
                params: ScenarioParams {
                    noise_std: Some(5.0),
                    ..Default::default()
                },
            },
            expected_events: vec![ExpectedEvent {
                event_type: "STRUCTURE_BREAK".to_string(),
                min_delay_samples: 5,
                max_delay_samples: 30,
                severity: None,
            }],
            validation: Some(ScenarioValidation {
                tc_should_change: true,
                ..Default::default()
            }),
        }
    }

    /// Create redundancy loss scenario.
    pub fn redundancy_loss() -> Self {
        Self {
            name: "redundancy_loss".to_string(),
            description: "Sensor dropout causing redundancy loss".to_string(),
            industry_agnostic: true,
            injection: ScenarioInjection {
                anomaly_type: "dropout".to_string(),
                target_sensor: "any".to_string(),
                start_percent: 0.5,
                duration_percent: Some(0.2),
                params: ScenarioParams::default(),
            },
            expected_events: vec![ExpectedEvent {
                event_type: "REDUNDANCY_DROP".to_string(),
                min_delay_samples: 1,
                max_delay_samples: 20,
                severity: None,
            }],
            validation: Some(ScenarioValidation {
                r_should_decrease: true,
                ..Default::default()
            }),
        }
    }
}

/// Collection of predefined scenarios.
pub fn predefined_scenarios() -> Vec<AnomalyScenario> {
    vec![
        AnomalyScenario::sensor_failure(),
        AnomalyScenario::gradual_drift(),
        AnomalyScenario::sudden_spike(),
        AnomalyScenario::correlation_break(),
        AnomalyScenario::redundancy_loss(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_scenario_creation() {
        let scenario = AnomalyScenario::sensor_failure();
        assert_eq!(scenario.name, "sensor_failure");
        assert!(scenario.industry_agnostic);
        assert!(!scenario.expected_events.is_empty());
    }

    #[test]
    fn test_scenario_json_roundtrip() {
        let scenario = AnomalyScenario::gradual_drift();
        let temp = NamedTempFile::new().unwrap();

        scenario.to_json_file(temp.path()).unwrap();
        let loaded = AnomalyScenario::from_json_file(temp.path()).unwrap();

        assert_eq!(loaded.name, scenario.name);
        assert_eq!(loaded.description, scenario.description);
    }

    #[test]
    fn test_to_anomaly_type() {
        let scenario = AnomalyScenario::sensor_failure();
        let anomaly = scenario.to_anomaly_type().unwrap();
        assert!(matches!(anomaly, AnomalyType::Stuck));

        let spike = AnomalyScenario::sudden_spike();
        let anomaly = spike.to_anomaly_type().unwrap();
        assert!(matches!(anomaly, AnomalyType::Spike { .. }));
    }

    #[test]
    fn test_predefined_scenarios() {
        let scenarios = predefined_scenarios();
        assert_eq!(scenarios.len(), 5);

        let names: Vec<_> = scenarios.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"sensor_failure"));
        assert!(names.contains(&"gradual_drift"));
        assert!(names.contains(&"sudden_spike"));
    }
}
