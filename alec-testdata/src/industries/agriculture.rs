// ALEC Testdata - Agriculture industry
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Agriculture (AgTech) sensor configurations.
//!
//! Field sensors monitoring crop conditions and weather stations.

use crate::anomalies::{AnomalyConfig, AnomalyType};
use crate::generator::SensorConfig;
use crate::patterns::SignalPattern;
use std::f64::consts::PI;

/// Agricultural scenario types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgriculturalScenario {
    /// Normal farm operation.
    Normal,
    /// Drought conditions (soil moisture decline).
    Drought,
    /// Sensor failure (stuck soil moisture).
    SensorFailure,
    /// Irrigation cycle with multiple events.
    IrrigationCycle,
    /// Frost event (sudden temperature drop).
    FrostEvent,
}

/// Create farm sensor configurations for a scenario.
pub fn create_farm_sensors(scenario: AgriculturalScenario) -> Vec<SensorConfig> {
    let mut sensors = base_farm_sensors();

    match scenario {
        AgriculturalScenario::Normal => {}
        AgriculturalScenario::Drought => {
            // Soil moisture with gradual decline
            if let Some(sensor) = sensors.iter_mut().find(|s| s.id == "soil_moisture") {
                sensor.pattern = SignalPattern::Composite(vec![
                    SignalPattern::Linear {
                        start: 60.0,
                        slope_per_ms: -0.000003, // Slow decline
                    },
                    SignalPattern::Sine {
                        amplitude: 2.0,
                        period_ms: 86_400_000,
                        phase: 0.0,
                        offset: 0.0,
                    },
                ]);
            }
        }
        AgriculturalScenario::SensorFailure => {
            // Soil moisture gets stuck
            if let Some(sensor) = sensors.iter_mut().find(|s| s.id == "soil_moisture") {
                sensor.anomaly = Some(AnomalyConfig::new(AnomalyType::Stuck, 500));
            }
        }
        AgriculturalScenario::IrrigationCycle => {
            // Soil moisture with step increases (irrigation events)
            if let Some(sensor) = sensors.iter_mut().find(|s| s.id == "soil_moisture") {
                sensor.pattern = SignalPattern::Composite(vec![
                    SignalPattern::Step {
                        levels: vec![
                            (0, 45.0),
                            (60 * 60_000, 75.0),  // Irrigation at 1h
                            (180 * 60_000, 70.0), // Decay
                            (240 * 60_000, 80.0), // Irrigation at 4h
                            (360 * 60_000, 72.0), // Decay
                            (420 * 60_000, 78.0), // Irrigation at 7h
                        ],
                    },
                    SignalPattern::Decay {
                        start: 0.0,
                        target: -5.0,
                        tau_ms: 3_600_000.0,
                    },
                ]);
            }
        }
        AgriculturalScenario::FrostEvent => {
            // Air temperature drops below 0
            if let Some(sensor) = sensors.iter_mut().find(|s| s.id == "air_temp") {
                sensor.anomaly = Some(
                    AnomalyConfig::new(
                        AnomalyType::BiasShift { offset: -25.0 },
                        720, // At midnight (12 hours)
                    )
                    .with_duration(60), // 1 hour frost
                );
            }
        }
    }

    sensors
}

/// Base farm sensor configurations.
fn base_farm_sensors() -> Vec<SensorConfig> {
    vec![
        // Soil temperature - slow variation, diurnal cycle
        SensorConfig::new(
            "soil_temp",
            "°C",
            10.0,
            35.0,
            SignalPattern::Composite(vec![SignalPattern::Diurnal {
                min: 15.0,
                max: 28.0,
                peak_hour: 15.0, // Lags air temp
                spread: 5.0,
            }]),
        )
        .with_noise(0.5),
        // Soil moisture - step changes (irrigation) + slow drift
        SensorConfig::new(
            "soil_moisture",
            "%",
            20.0,
            80.0,
            SignalPattern::Composite(vec![
                SignalPattern::Constant { value: 55.0 },
                SignalPattern::Sine {
                    amplitude: 5.0,
                    period_ms: 86_400_000,
                    phase: PI,
                    offset: 0.0,
                },
            ]),
        )
        .with_noise(1.0),
        // Air temperature - diurnal + seasonal
        SensorConfig::new(
            "air_temp",
            "°C",
            -5.0,
            45.0,
            SignalPattern::Diurnal {
                min: 12.0,
                max: 28.0,
                peak_hour: 14.0,
                spread: 4.0,
            },
        )
        .with_noise(0.8),
        // Air humidity - inverse correlation with temp
        SensorConfig::new(
            "air_humidity",
            "%",
            30.0,
            95.0,
            SignalPattern::Diurnal {
                min: 45.0,
                max: 85.0,
                peak_hour: 5.0, // Highest at dawn
                spread: 4.0,
            },
        )
        .with_noise(2.0),
        // Rain gauge - sparse events, mostly 0
        SensorConfig::new(
            "rain_gauge",
            "mm",
            0.0,
            50.0,
            SignalPattern::Poisson {
                lambda: 0.02, // Rare events
                scale: 2.0,
            },
        ),
        // Solar radiation - bell curve during day
        SensorConfig::new(
            "solar_radiation",
            "W/m²",
            0.0,
            1200.0,
            SignalPattern::Diurnal {
                min: 0.0,
                max: 950.0,
                peak_hour: 12.0,
                spread: 3.0,
            },
        )
        .with_noise(20.0),
        // Wind speed - gusty, log-normal
        SensorConfig::new(
            "wind_speed",
            "m/s",
            0.0,
            25.0,
            SignalPattern::LogNormal {
                mu: 1.0,
                sigma: 0.8,
            },
        )
        .with_noise(0.5),
        // Leaf wetness - binary, correlated with humidity/rain
        SensorConfig::new(
            "leaf_wetness",
            "binary",
            0.0,
            1.0,
            SignalPattern::Binary {
                initial: false,
                p_on: 0.02,
                p_off: 0.1,
            },
        ),
    ]
}

/// Get expected metrics for agricultural datasets.
pub fn expected_agriculture_metrics() -> crate::manifest::ExpectedMetrics {
    crate::manifest::ExpectedMetrics {
        compression_ratio_range: Some((0.70, 0.85)),
        tc_range: Some((1.5, 4.0)),
        r_range: Some((0.3, 0.6)),
        h_bytes_range: Some((4.0, 6.5)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_normal_sensors() {
        let sensors = create_farm_sensors(AgriculturalScenario::Normal);
        assert_eq!(sensors.len(), 8);

        let ids: Vec<_> = sensors.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"soil_temp"));
        assert!(ids.contains(&"soil_moisture"));
        assert!(ids.contains(&"air_temp"));
        assert!(ids.contains(&"air_humidity"));
    }

    #[test]
    fn test_create_drought_sensors() {
        let sensors = create_farm_sensors(AgriculturalScenario::Drought);
        let soil_moisture = sensors.iter().find(|s| s.id == "soil_moisture").unwrap();

        // Should have linear decline pattern
        assert!(matches!(soil_moisture.pattern, SignalPattern::Composite(_)));
    }

    #[test]
    fn test_create_sensor_failure() {
        let sensors = create_farm_sensors(AgriculturalScenario::SensorFailure);
        let soil_moisture = sensors.iter().find(|s| s.id == "soil_moisture").unwrap();

        assert!(soil_moisture.anomaly.is_some());
        let anomaly = soil_moisture.anomaly.as_ref().unwrap();
        assert!(matches!(anomaly.anomaly_type, AnomalyType::Stuck));
    }

    #[test]
    fn test_sensor_ranges() {
        let sensors = create_farm_sensors(AgriculturalScenario::Normal);

        for sensor in sensors {
            assert!(sensor.min < sensor.max, "Invalid range for {}", sensor.id);
        }
    }
}
