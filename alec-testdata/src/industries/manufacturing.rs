// ALEC Testdata - Manufacturing industry
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Manufacturing (IIoT) sensor configurations.
//!
//! Factory floor sensors on production line equipment.

use crate::anomalies::{AnomalyConfig, AnomalyType};
use crate::generator::SensorConfig;
use crate::patterns::SignalPattern;

/// Manufacturing scenario types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManufacturingScenario {
    /// Normal shift operation with machine cycles.
    NormalShift,
    /// Single machine cycle (startup, run, shutdown).
    MachineCycle,
    /// Bearing wear developing.
    BearingFailure,
    /// Pressure leak event.
    LeakEvent,
    /// Motor overheating.
    MotorOverheat,
}

/// Create factory sensor configurations for a scenario.
pub fn create_factory_sensors(scenario: ManufacturingScenario) -> Vec<SensorConfig> {
    let mut sensors = base_factory_sensors();

    match scenario {
        ManufacturingScenario::NormalShift => {}
        ManufacturingScenario::MachineCycle => {
            // Shorter cycle for demo
            for sensor in &mut sensors {
                if let SignalPattern::MachineState { cycle_period_ms, .. } = &mut sensor.pattern {
                    *cycle_period_ms = 3_600_000; // 1 hour cycle
                }
            }
        }
        ManufacturingScenario::BearingFailure => {
            // Vibration increases with new harmonics
            if let Some(vib) = sensors.iter_mut().find(|s| s.id == "motor_vibration") {
                vib.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::Oscillation {
                        amplitude: 0.5,
                        frequency_hz: 0.05,
                    },
                    200, // After some normal operation
                ));
            }
        }
        ManufacturingScenario::LeakEvent => {
            // Pressure drops, flow increases
            if let Some(pressure) = sensors.iter_mut().find(|s| s.id == "pressure_outlet") {
                pressure.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::Drift {
                        rate_per_sample: -0.01,
                    },
                    150,
                ));
            }
            if let Some(flow) = sensors.iter_mut().find(|s| s.id == "flow_rate") {
                flow.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::Drift {
                        rate_per_sample: 0.5,
                    },
                    150,
                ));
            }
        }
        ManufacturingScenario::MotorOverheat => {
            // Temperature rises beyond normal
            if let Some(temp) = sensors.iter_mut().find(|s| s.id == "motor_temp") {
                temp.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::Drift {
                        rate_per_sample: 0.1,
                    },
                    300,
                ));
            }
        }
    }

    sensors
}

/// Base factory sensor configurations.
fn base_factory_sensors() -> Vec<SensorConfig> {
    vec![
        // Motor vibration - periodic + noise, machine state
        SensorConfig::new(
            "motor_vibration",
            "g",
            0.1,
            2.0,
            SignalPattern::Composite(vec![
                SignalPattern::MachineState {
                    idle_value: 0.2,
                    running_value: 0.8,
                    startup_duration_ms: 60_000,
                    running_duration_ms: 7_200_000,
                    shutdown_duration_ms: 60_000,
                    cycle_period_ms: 8 * 3_600_000, // 8 hour shift
                },
                SignalPattern::Sine {
                    amplitude: 0.1,
                    period_ms: 1000, // 1 Hz fundamental
                    phase: 0.0,
                    offset: 0.0,
                },
            ]),
        )
        .with_noise(0.05),
        // Motor temperature - slow rise during operation
        SensorConfig::new(
            "motor_temp",
            "°C",
            40.0,
            80.0,
            SignalPattern::MachineState {
                idle_value: 45.0,
                running_value: 68.0,
                startup_duration_ms: 300_000, // 5 min warmup
                running_duration_ms: 7_200_000,
                shutdown_duration_ms: 600_000, // 10 min cooldown
                cycle_period_ms: 8 * 3_600_000,
            },
        )
        .with_noise(0.5),
        // Inlet pressure - tight control, PID oscillations
        SensorConfig::new(
            "pressure_inlet",
            "bar",
            4.5,
            5.5,
            SignalPattern::Composite(vec![
                SignalPattern::Constant { value: 5.0 },
                SignalPattern::Sine {
                    amplitude: 0.1,
                    period_ms: 30_000, // PID cycling
                    phase: 0.0,
                    offset: 0.0,
                },
            ]),
        )
        .with_noise(0.02),
        // Outlet pressure - follows inlet with drop
        SensorConfig::new(
            "pressure_outlet",
            "bar",
            3.0,
            4.0,
            SignalPattern::Composite(vec![
                SignalPattern::Constant { value: 3.5 },
                SignalPattern::Sine {
                    amplitude: 0.08,
                    period_ms: 30_000,
                    phase: 0.5, // Lag behind inlet
                    offset: 0.0,
                },
            ]),
        )
        .with_noise(0.02),
        // Flow rate - follows setpoint
        SensorConfig::new(
            "flow_rate",
            "L/min",
            100.0,
            150.0,
            SignalPattern::MachineState {
                idle_value: 0.0,
                running_value: 125.0,
                startup_duration_ms: 60_000,
                running_duration_ms: 7_200_000,
                shutdown_duration_ms: 60_000,
                cycle_period_ms: 8 * 3_600_000,
            },
        )
        .with_noise(2.0),
        // Power consumption - correlated with vibration
        SensorConfig::new(
            "power_consumption",
            "kW",
            5.0,
            15.0,
            SignalPattern::MachineState {
                idle_value: 2.0,
                running_value: 12.0,
                startup_duration_ms: 30_000, // Quick ramp
                running_duration_ms: 7_200_000,
                shutdown_duration_ms: 30_000,
                cycle_period_ms: 8 * 3_600_000,
            },
        )
        .with_noise(0.5),
        // Product count - step increases
        SensorConfig::new(
            "product_count",
            "count",
            0.0,
            10000.0,
            SignalPattern::Linear {
                start: 0.0,
                slope_per_ms: 0.0001, // ~6 per minute
            },
        ),
    ]
}

/// Get expected metrics for manufacturing datasets.
pub fn expected_manufacturing_metrics() -> crate::manifest::ExpectedMetrics {
    crate::manifest::ExpectedMetrics {
        compression_ratio_range: Some((0.60, 0.75)),
        tc_range: Some((2.0, 5.0)),
        r_range: Some((0.35, 0.55)),
        h_bytes_range: Some((4.5, 6.5)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_normal_sensors() {
        let sensors = create_factory_sensors(ManufacturingScenario::NormalShift);
        assert_eq!(sensors.len(), 7);

        let ids: Vec<_> = sensors.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"motor_vibration"));
        assert!(ids.contains(&"motor_temp"));
        assert!(ids.contains(&"pressure_inlet"));
    }

    #[test]
    fn test_bearing_failure() {
        let sensors = create_factory_sensors(ManufacturingScenario::BearingFailure);
        let vib = sensors.iter().find(|s| s.id == "motor_vibration").unwrap();
        assert!(vib.anomaly.is_some());
    }

    #[test]
    fn test_leak_event() {
        let sensors = create_factory_sensors(ManufacturingScenario::LeakEvent);
        let pressure = sensors.iter().find(|s| s.id == "pressure_outlet").unwrap();
        let flow = sensors.iter().find(|s| s.id == "flow_rate").unwrap();

        assert!(pressure.anomaly.is_some());
        assert!(flow.anomaly.is_some());
    }
}
