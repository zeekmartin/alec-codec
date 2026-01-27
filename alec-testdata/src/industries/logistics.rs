// ALEC Testdata - Logistics industry
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Logistics (Cold Chain) sensor configurations.
//!
//! Temperature-controlled transport and fleet tracking.

use crate::anomalies::{AnomalyConfig, AnomalyType};
use crate::generator::SensorConfig;
use crate::patterns::SignalPattern;

/// Logistics scenario types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogisticsScenario {
    /// Normal delivery route.
    NormalRoute,
    /// Multi-stop delivery with door events.
    MultiStop,
    /// Cold chain breach (temp > 8°C).
    ColdChainBreach,
    /// Refrigeration unit failure.
    RefrigerationFailure,
    /// Route deviation.
    RouteDeviation,
    /// Fuel theft event.
    FuelTheft,
}

/// Create logistics sensor configurations for a scenario.
pub fn create_logistics_sensors(scenario: LogisticsScenario) -> Vec<SensorConfig> {
    let mut sensors = base_logistics_sensors();

    match scenario {
        LogisticsScenario::NormalRoute => {}
        LogisticsScenario::MultiStop => {
            // Door opens at multiple stops, causing temp spikes
            if let Some(door) = sensors.iter_mut().find(|s| s.id == "door_status") {
                door.pattern = SignalPattern::Step {
                    levels: vec![
                        (0, 0.0),
                        (60 * 60_000, 1.0), // Stop 1
                        (65 * 60_000, 0.0),
                        (180 * 60_000, 1.0), // Stop 2
                        (185 * 60_000, 0.0),
                        (300 * 60_000, 1.0), // Stop 3
                        (310 * 60_000, 0.0),
                        (420 * 60_000, 1.0), // Stop 4
                        (430 * 60_000, 0.0),
                    ],
                };
            }
        }
        LogisticsScenario::ColdChainBreach => {
            // Cargo temp rises above 8°C
            if let Some(temp) = sensors.iter_mut().find(|s| s.id == "cargo_temp") {
                temp.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::Drift {
                        rate_per_sample: 0.02, // Gradual rise
                    },
                    150,
                ));
            }
        }
        LogisticsScenario::RefrigerationFailure => {
            // Temp rises continuously
            if let Some(temp) = sensors.iter_mut().find(|s| s.id == "cargo_temp") {
                temp.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::Drift {
                        rate_per_sample: 0.05, // Faster rise
                    },
                    100,
                ));
            }
        }
        LogisticsScenario::RouteDeviation => {
            // GPS suddenly deviates
            if let Some(lat) = sensors.iter_mut().find(|s| s.id == "gps_lat") {
                lat.anomaly = Some(
                    AnomalyConfig::new(
                        AnomalyType::BiasShift { offset: 0.1 }, // ~10km deviation
                        200,
                    )
                    .with_duration(100),
                );
            }
        }
        LogisticsScenario::FuelTheft => {
            // Fuel level sudden drop
            if let Some(fuel) = sensors.iter_mut().find(|s| s.id == "fuel_level") {
                fuel.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::Spike { magnitude: -30.0 }, // 30% stolen
                    180,
                ));
            }
        }
    }

    sensors
}

/// Base logistics sensor configurations.
fn base_logistics_sensors() -> Vec<SensorConfig> {
    vec![
        // Cargo temperature - tight control for pharma (2-8°C)
        SensorConfig::new(
            "cargo_temp",
            "°C",
            2.0,
            8.0,
            SignalPattern::Constant { value: 5.0 },
        )
        .with_noise(0.3),
        // Ambient temperature - external conditions
        SensorConfig::new(
            "ambient_temp",
            "°C",
            -10.0,
            40.0,
            SignalPattern::Diurnal {
                min: 8.0,
                max: 25.0,
                peak_hour: 14.0,
                spread: 4.0,
            },
        )
        .with_noise(1.0),
        // Door status - binary, sparse events
        SensorConfig::new(
            "door_status",
            "binary",
            0.0,
            1.0,
            SignalPattern::Constant { value: 0.0 }, // Mostly closed
        ),
        // GPS latitude - route following
        SensorConfig::new(
            "gps_lat",
            "°",
            -90.0,
            90.0,
            SignalPattern::Linear {
                start: 48.8566,             // Paris
                slope_per_ms: 0.0000000015, // Moving north
            },
        )
        .with_noise(0.0001),
        // GPS longitude
        SensorConfig::new(
            "gps_lon",
            "°",
            -180.0,
            180.0,
            SignalPattern::Linear {
                start: 2.3522,
                slope_per_ms: 0.0000000025, // Moving east
            },
        )
        .with_noise(0.0001),
        // GPS speed
        SensorConfig::new(
            "gps_speed",
            "km/h",
            0.0,
            120.0,
            SignalPattern::Composite(vec![
                SignalPattern::Constant { value: 80.0 }, // Highway average
                SignalPattern::RandomWalk {
                    start: 0.0,
                    step_std: 5.0,
                },
            ]),
        )
        .with_noise(3.0),
        // Fuel level - decreasing with refuel events
        SensorConfig::new(
            "fuel_level",
            "%",
            10.0,
            100.0,
            SignalPattern::Composite(vec![
                SignalPattern::Constant { value: 85.0 },
                SignalPattern::Linear {
                    start: 0.0,
                    slope_per_ms: -0.000002, // Slow consumption
                },
            ]),
        )
        .with_noise(0.5),
        // Engine hours - monotonic when running
        SensorConfig::new(
            "engine_hours",
            "h",
            0.0,
            100000.0,
            SignalPattern::Linear {
                start: 5432.0,
                slope_per_ms: 0.000000278, // 1 hour per hour when running
            },
        ),
    ]
}

/// Get expected metrics for logistics datasets.
pub fn expected_logistics_metrics() -> crate::manifest::ExpectedMetrics {
    crate::manifest::ExpectedMetrics {
        compression_ratio_range: Some((0.70, 0.85)),
        tc_range: Some((1.5, 3.5)),
        r_range: Some((0.30, 0.50)),
        h_bytes_range: Some((4.5, 6.5)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_normal_sensors() {
        let sensors = create_logistics_sensors(LogisticsScenario::NormalRoute);
        assert_eq!(sensors.len(), 8);

        let ids: Vec<_> = sensors.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"cargo_temp"));
        assert!(ids.contains(&"gps_lat"));
        assert!(ids.contains(&"fuel_level"));
    }

    #[test]
    fn test_cold_chain_breach() {
        let sensors = create_logistics_sensors(LogisticsScenario::ColdChainBreach);
        let temp = sensors.iter().find(|s| s.id == "cargo_temp").unwrap();
        assert!(temp.anomaly.is_some());
    }

    #[test]
    fn test_multi_stop() {
        let sensors = create_logistics_sensors(LogisticsScenario::MultiStop);
        let door = sensors.iter().find(|s| s.id == "door_status").unwrap();
        assert!(matches!(door.pattern, SignalPattern::Step { .. }));
    }

    #[test]
    fn test_fuel_theft() {
        let sensors = create_logistics_sensors(LogisticsScenario::FuelTheft);
        let fuel = sensors.iter().find(|s| s.id == "fuel_level").unwrap();
        assert!(fuel.anomaly.is_some());
    }
}
