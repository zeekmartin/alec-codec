// ALEC Testdata - Smart City industry
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Smart City sensor configurations.
//!
//! Urban sensors for traffic, environment, and infrastructure.

use crate::anomalies::{AnomalyConfig, AnomalyType};
use crate::generator::SensorConfig;
use crate::patterns::SignalPattern;

/// Smart City scenario types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartCityScenario {
    /// Normal weekday traffic patterns.
    Weekday,
    /// Weekend traffic patterns (flatter, later peaks).
    Weekend,
    /// Traffic accident event.
    Accident,
    /// Festival/event with elevated counts.
    Festival,
    /// Pollution event uncorrelated with traffic.
    PollutionEvent,
}

/// Create city sensor configurations for a scenario.
pub fn create_city_sensors(scenario: SmartCityScenario) -> Vec<SensorConfig> {
    let mut sensors = match scenario {
        SmartCityScenario::Weekend => weekend_city_sensors(),
        _ => weekday_city_sensors(),
    };

    match scenario {
        SmartCityScenario::Weekday | SmartCityScenario::Weekend => {}
        SmartCityScenario::Accident => {
            // Traffic speed drops, count increases
            if let Some(speed) = sensors.iter_mut().find(|s| s.id == "traffic_speed") {
                speed.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::BiasShift { offset: -30.0 },
                    720, // At noon
                ).with_duration(60)); // 1 hour
            }
            if let Some(count) = sensors.iter_mut().find(|s| s.id == "traffic_count") {
                count.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::BiasShift { offset: 20.0 },
                    720,
                ).with_duration(60));
            }
        }
        SmartCityScenario::Festival => {
            // All counts elevated
            for sensor in &mut sensors {
                if sensor.id == "pedestrian_count" || sensor.id == "traffic_count" {
                    sensor.anomaly = Some(AnomalyConfig::new(
                        AnomalyType::BiasShift { offset: 50.0 },
                        600, // Evening start
                    ).with_duration(360)); // 6 hours
                }
            }
        }
        SmartCityScenario::PollutionEvent => {
            // PM2.5 spike uncorrelated with traffic
            if let Some(pm25) = sensors.iter_mut().find(|s| s.id == "air_quality_pm25") {
                pm25.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::Spike { magnitude: 80.0 },
                    400,
                ));
            }
        }
    }

    sensors
}

/// Weekday city sensor configurations.
fn weekday_city_sensors() -> Vec<SensorConfig> {
    vec![
        // Traffic count - bimodal (morning/evening rush)
        SensorConfig::new(
            "traffic_count",
            "vehicles/min",
            0.0,
            60.0,
            SignalPattern::BimodalDiurnal {
                min: 5.0,
                max: 45.0,
                peak1_hour: 8.0,
                peak2_hour: 18.0,
                spread: 1.5,
            },
        )
        .with_noise(3.0),
        // Traffic speed - inverse of count
        SensorConfig::new(
            "traffic_speed",
            "km/h",
            0.0,
            60.0,
            SignalPattern::BimodalDiurnal {
                min: 50.0,
                max: 20.0, // Inverted: min at peaks
                peak1_hour: 8.0,
                peak2_hour: 18.0,
                spread: 1.5,
            },
        )
        .with_noise(5.0),
        // Air quality PM2.5 - correlated with traffic
        SensorConfig::new(
            "air_quality_pm25",
            "µg/m³",
            5.0,
            150.0,
            SignalPattern::BimodalDiurnal {
                min: 15.0,
                max: 55.0,
                peak1_hour: 9.0, // Lag behind traffic
                peak2_hour: 19.0,
                spread: 2.0,
            },
        )
        .with_noise(5.0),
        // Noise level - correlated with traffic
        SensorConfig::new(
            "noise_level",
            "dB",
            40.0,
            90.0,
            SignalPattern::BimodalDiurnal {
                min: 45.0,
                max: 75.0,
                peak1_hour: 8.0,
                peak2_hour: 18.0,
                spread: 2.0,
            },
        )
        .with_noise(3.0),
        // Parking occupancy - fills morning, empties evening
        SensorConfig::new(
            "parking_occupancy",
            "%",
            0.0,
            100.0,
            SignalPattern::Diurnal {
                min: 10.0,
                max: 90.0,
                peak_hour: 12.0, // Full at midday
                spread: 3.0,
            },
        )
        .with_noise(5.0),
        // Street light status - on at night
        SensorConfig::new(
            "street_light_status",
            "binary",
            0.0,
            1.0,
            SignalPattern::Step {
                levels: vec![
                    (0, 1.0),                  // On at midnight
                    (6 * 3_600_000, 0.0),      // Off at 6am
                    (18 * 3_600_000, 1.0),     // On at 6pm
                ],
            },
        ),
        // Pedestrian count - rush hours + lunch
        SensorConfig::new(
            "pedestrian_count",
            "count/min",
            0.0,
            200.0,
            SignalPattern::Composite(vec![
                SignalPattern::BimodalDiurnal {
                    min: 10.0,
                    max: 80.0,
                    peak1_hour: 8.5,
                    peak2_hour: 17.5,
                    spread: 1.0,
                },
                SignalPattern::Diurnal {
                    min: 0.0,
                    max: 40.0,
                    peak_hour: 12.5, // Lunch hour
                    spread: 0.5,
                },
            ]),
        )
        .with_noise(8.0),
    ]
}

/// Weekend city sensor configurations (flatter patterns).
fn weekend_city_sensors() -> Vec<SensorConfig> {
    vec![
        // Traffic count - single midday peak
        SensorConfig::new(
            "traffic_count",
            "vehicles/min",
            0.0,
            60.0,
            SignalPattern::Diurnal {
                min: 8.0,
                max: 30.0,
                peak_hour: 14.0,
                spread: 4.0,
            },
        )
        .with_noise(3.0),
        // Traffic speed - generally better
        SensorConfig::new(
            "traffic_speed",
            "km/h",
            0.0,
            60.0,
            SignalPattern::Diurnal {
                min: 35.0,
                max: 55.0,
                peak_hour: 4.0, // Best at night
                spread: 6.0,
            },
        )
        .with_noise(4.0),
        // Air quality - generally better
        SensorConfig::new(
            "air_quality_pm25",
            "µg/m³",
            5.0,
            150.0,
            SignalPattern::Diurnal {
                min: 12.0,
                max: 35.0,
                peak_hour: 15.0,
                spread: 4.0,
            },
        )
        .with_noise(4.0),
        // Noise level - evening entertainment peak
        SensorConfig::new(
            "noise_level",
            "dB",
            40.0,
            90.0,
            SignalPattern::Diurnal {
                min: 42.0,
                max: 70.0,
                peak_hour: 21.0, // Evening entertainment
                spread: 3.0,
            },
        )
        .with_noise(4.0),
        // Parking occupancy - shopping hours
        SensorConfig::new(
            "parking_occupancy",
            "%",
            0.0,
            100.0,
            SignalPattern::Diurnal {
                min: 5.0,
                max: 75.0,
                peak_hour: 15.0, // Shopping peak
                spread: 3.0,
            },
        )
        .with_noise(5.0),
        // Street light status - same as weekday
        SensorConfig::new(
            "street_light_status",
            "binary",
            0.0,
            1.0,
            SignalPattern::Step {
                levels: vec![
                    (0, 1.0),
                    (7 * 3_600_000, 0.0),  // Later off on weekend
                    (18 * 3_600_000, 1.0),
                ],
            },
        ),
        // Pedestrian count - shopping and leisure
        SensorConfig::new(
            "pedestrian_count",
            "count/min",
            0.0,
            200.0,
            SignalPattern::Diurnal {
                min: 15.0,
                max: 100.0,
                peak_hour: 15.0,
                spread: 3.0,
            },
        )
        .with_noise(10.0),
    ]
}

/// Get expected metrics for smart city datasets.
pub fn expected_smart_city_metrics() -> crate::manifest::ExpectedMetrics {
    crate::manifest::ExpectedMetrics {
        compression_ratio_range: Some((0.65, 0.80)),
        tc_range: Some((2.5, 5.5)),
        r_range: Some((0.35, 0.55)),
        h_bytes_range: Some((4.0, 6.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_weekday_sensors() {
        let sensors = create_city_sensors(SmartCityScenario::Weekday);
        assert_eq!(sensors.len(), 7);

        let ids: Vec<_> = sensors.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"traffic_count"));
        assert!(ids.contains(&"air_quality_pm25"));
    }

    #[test]
    fn test_create_weekend_sensors() {
        let sensors = create_city_sensors(SmartCityScenario::Weekend);
        assert_eq!(sensors.len(), 7);
    }

    #[test]
    fn test_accident_anomaly() {
        let sensors = create_city_sensors(SmartCityScenario::Accident);
        let speed = sensors.iter().find(|s| s.id == "traffic_speed").unwrap();
        assert!(speed.anomaly.is_some());
    }

    #[test]
    fn test_festival_anomaly() {
        let sensors = create_city_sensors(SmartCityScenario::Festival);
        let ped = sensors.iter().find(|s| s.id == "pedestrian_count").unwrap();
        assert!(ped.anomaly.is_some());
    }
}
