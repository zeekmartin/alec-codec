// ALEC Testdata - Satellite IoT industry
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Satellite IoT sensor configurations.
//!
//! Remote devices with intermittent connectivity, battery-powered.

use crate::anomalies::{AnomalyConfig, AnomalyType};
use crate::generator::SensorConfig;
use crate::patterns::SignalPattern;

/// Satellite IoT scenario types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SatelliteScenario {
    /// Normal stationary device operation.
    Stationary,
    /// Moving asset tracking.
    MovingAsset,
    /// Battery critical condition.
    BatteryCritical,
    /// Signal loss event.
    SignalLoss,
    /// GPS drift anomaly.
    GpsDrift,
}

/// Create satellite sensor configurations for a scenario.
pub fn create_satellite_sensors(scenario: SatelliteScenario) -> Vec<SensorConfig> {
    let mut sensors = base_satellite_sensors();

    match scenario {
        SatelliteScenario::Stationary => {
            // GPS stays around fixed point with small noise
            if let Some(lat) = sensors.iter_mut().find(|s| s.id == "gps_lat") {
                lat.pattern = SignalPattern::Constant { value: 48.8566 };
                lat.noise_std = 0.0001; // ~10m noise
            }
            if let Some(lon) = sensors.iter_mut().find(|s| s.id == "gps_lon") {
                lon.pattern = SignalPattern::Constant { value: 2.3522 };
                lon.noise_std = 0.0001;
            }
        }
        SatelliteScenario::MovingAsset => {
            // GPS follows a trajectory (simplified linear movement)
            if let Some(lat) = sensors.iter_mut().find(|s| s.id == "gps_lat") {
                lat.pattern = SignalPattern::Linear {
                    start: 48.8566,
                    slope_per_ms: 0.000000001, // Slow movement north
                };
                lat.noise_std = 0.0002;
            }
            if let Some(lon) = sensors.iter_mut().find(|s| s.id == "gps_lon") {
                lon.pattern = SignalPattern::Linear {
                    start: 2.3522,
                    slope_per_ms: 0.000000002, // Slow movement east
                };
                lon.noise_std = 0.0002;
            }
        }
        SatelliteScenario::BatteryCritical => {
            // Battery drops below critical threshold
            if let Some(battery) = sensors.iter_mut().find(|s| s.id == "battery_voltage") {
                battery.pattern = SignalPattern::Linear {
                    start: 3.5,
                    slope_per_ms: -0.0000002, // Faster discharge
                };
            }
        }
        SatelliteScenario::SignalLoss => {
            // RSSI stuck at minimum (no signal)
            if let Some(rssi) = sensors.iter_mut().find(|s| s.id == "signal_rssi") {
                rssi.anomaly = Some(
                    AnomalyConfig::new(
                        AnomalyType::Stuck,
                        50, // After some normal operation
                    )
                    .with_duration(100),
                );
            }
        }
        SatelliteScenario::GpsDrift => {
            // GPS suddenly jumps to wrong position
            if let Some(lat) = sensors.iter_mut().find(|s| s.id == "gps_lat") {
                lat.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::Spike { magnitude: 0.5 }, // ~50km jump
                    100,
                ));
            }
        }
    }

    sensors
}

/// Base satellite sensor configurations.
fn base_satellite_sensors() -> Vec<SensorConfig> {
    vec![
        // Battery voltage - slow decay, charge cycles
        SensorConfig::new(
            "battery_voltage",
            "V",
            3.0,
            4.2,
            SignalPattern::Sawtooth {
                min: 3.3,
                max: 4.1,
                period_ms: 86_400_000, // Daily charge cycle
                ascending: false,      // Discharge
            },
        )
        .with_noise(0.02),
        // Signal RSSI - random walk with bounds
        SensorConfig::new(
            "signal_rssi",
            "dBm",
            -120.0,
            -60.0,
            SignalPattern::RandomWalk {
                start: -85.0,
                step_std: 3.0,
            },
        )
        .with_noise(2.0),
        // GPS latitude
        SensorConfig::new(
            "gps_lat",
            "°",
            -90.0,
            90.0,
            SignalPattern::Constant { value: 48.8566 },
        )
        .with_noise(0.0001),
        // GPS longitude
        SensorConfig::new(
            "gps_lon",
            "°",
            -180.0,
            180.0,
            SignalPattern::Constant { value: 2.3522 },
        )
        .with_noise(0.0001),
        // Internal temperature
        SensorConfig::new(
            "internal_temp",
            "°C",
            -20.0,
            60.0,
            SignalPattern::Diurnal {
                min: 15.0,
                max: 35.0,
                peak_hour: 14.0,
                spread: 4.0,
            },
        )
        .with_noise(1.0),
        // Packet counter - monotonic increase
        SensorConfig::new(
            "packet_counter",
            "count",
            0.0,
            65535.0,
            SignalPattern::Linear {
                start: 0.0,
                slope_per_ms: 0.000016, // ~1 packet per minute
            },
        ),
        // TX power - adaptive, inverse of RSSI
        SensorConfig::new(
            "tx_power",
            "dBm",
            10.0,
            20.0,
            SignalPattern::Constant { value: 14.0 },
        )
        .with_noise(1.0),
    ]
}

/// Get expected metrics for satellite datasets.
pub fn expected_satellite_metrics() -> crate::manifest::ExpectedMetrics {
    crate::manifest::ExpectedMetrics {
        compression_ratio_range: Some((0.65, 0.80)),
        tc_range: Some((0.8, 2.5)),
        r_range: Some((0.25, 0.50)),
        h_bytes_range: Some((5.0, 7.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_stationary_sensors() {
        let sensors = create_satellite_sensors(SatelliteScenario::Stationary);
        assert_eq!(sensors.len(), 7);

        let gps_lat = sensors.iter().find(|s| s.id == "gps_lat").unwrap();
        assert!(matches!(gps_lat.pattern, SignalPattern::Constant { .. }));
    }

    #[test]
    fn test_create_moving_sensors() {
        let sensors = create_satellite_sensors(SatelliteScenario::MovingAsset);
        let gps_lat = sensors.iter().find(|s| s.id == "gps_lat").unwrap();
        assert!(matches!(gps_lat.pattern, SignalPattern::Linear { .. }));
    }

    #[test]
    fn test_signal_loss_anomaly() {
        let sensors = create_satellite_sensors(SatelliteScenario::SignalLoss);
        let rssi = sensors.iter().find(|s| s.id == "signal_rssi").unwrap();
        assert!(rssi.anomaly.is_some());
    }
}
