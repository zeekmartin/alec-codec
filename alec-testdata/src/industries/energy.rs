// ALEC Testdata - Energy industry
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Energy (Smart Grid) sensor configurations.
//!
//! Power grid monitoring and renewable integration.

use crate::anomalies::{AnomalyConfig, AnomalyType};
use crate::generator::SensorConfig;
use crate::patterns::SignalPattern;

/// Energy/Grid scenario types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnergyScenario {
    /// Normal 24-hour grid operation.
    Normal,
    /// Industrial load profile.
    IndustrialLoad,
    /// Phase imbalance event.
    PhaseImbalance,
    /// Harmonic distortion event.
    HarmonicEvent,
    /// Power factor degradation.
    PowerFactorDrop,
    /// Frequency deviation.
    FrequencyDeviation,
}

/// Create grid sensor configurations for a scenario.
pub fn create_grid_sensors(scenario: EnergyScenario) -> Vec<SensorConfig> {
    let mut sensors = match scenario {
        EnergyScenario::IndustrialLoad => industrial_grid_sensors(),
        _ => base_grid_sensors(),
    };

    match scenario {
        EnergyScenario::Normal | EnergyScenario::IndustrialLoad => {}
        EnergyScenario::PhaseImbalance => {
            // One phase voltage differs
            if let Some(v1) = sensors.iter_mut().find(|s| s.id == "voltage_l1") {
                v1.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::BiasShift { offset: -8.0 }, // Low voltage
                    400,
                ).with_duration(200));
            }
        }
        EnergyScenario::HarmonicEvent => {
            // THD increases
            if let Some(thd) = sensors.iter_mut().find(|s| s.id == "thd") {
                thd.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::Drift {
                        rate_per_sample: 0.02,
                    },
                    300,
                ));
            }
        }
        EnergyScenario::PowerFactorDrop => {
            // Power factor drops below 0.85
            if let Some(pf) = sensors.iter_mut().find(|s| s.id == "power_factor") {
                pf.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::Drift {
                        rate_per_sample: -0.002,
                    },
                    200,
                ));
            }
        }
        EnergyScenario::FrequencyDeviation => {
            // Frequency deviates outside 49.95-50.05 Hz
            if let Some(freq) = sensors.iter_mut().find(|s| s.id == "frequency") {
                freq.anomaly = Some(AnomalyConfig::new(
                    AnomalyType::Drift {
                        rate_per_sample: -0.0005,
                    },
                    500,
                ));
            }
        }
    }

    sensors
}

/// Base grid sensor configurations (residential load profile).
fn base_grid_sensors() -> Vec<SensorConfig> {
    vec![
        // Voltage L1 - tight regulation
        SensorConfig::new(
            "voltage_l1",
            "V",
            220.0,
            240.0,
            SignalPattern::Composite(vec![
                SignalPattern::Constant { value: 230.0 },
                SignalPattern::Sine {
                    amplitude: 2.0,
                    period_ms: 86_400_000, // Daily variation
                    phase: 0.0,
                    offset: 0.0,
                },
            ]),
        )
        .with_noise(0.5),
        // Voltage L2 - correlated with L1
        SensorConfig::new(
            "voltage_l2",
            "V",
            220.0,
            240.0,
            SignalPattern::Composite(vec![
                SignalPattern::Constant { value: 230.0 },
                SignalPattern::Sine {
                    amplitude: 2.0,
                    period_ms: 86_400_000,
                    phase: 0.1,
                    offset: 0.0,
                },
            ]),
        )
        .with_noise(0.5),
        // Voltage L3 - correlated with L1, L2
        SensorConfig::new(
            "voltage_l3",
            "V",
            220.0,
            240.0,
            SignalPattern::Composite(vec![
                SignalPattern::Constant { value: 230.0 },
                SignalPattern::Sine {
                    amplitude: 2.0,
                    period_ms: 86_400_000,
                    phase: 0.2,
                    offset: 0.0,
                },
            ]),
        )
        .with_noise(0.5),
        // Current L1 - residential evening peak
        SensorConfig::new(
            "current_l1",
            "A",
            0.0,
            100.0,
            SignalPattern::Diurnal {
                min: 15.0,
                max: 55.0,
                peak_hour: 19.0, // Evening peak (residential)
                spread: 3.0,
            },
        )
        .with_noise(2.0),
        // Power factor - load dependent
        SensorConfig::new(
            "power_factor",
            "ratio",
            0.85,
            1.0,
            SignalPattern::Composite(vec![
                SignalPattern::Constant { value: 0.95 },
                SignalPattern::Sine {
                    amplitude: 0.03,
                    period_ms: 86_400_000,
                    phase: 3.14, // Worst at peak
                    offset: 0.0,
                },
            ]),
        )
        .with_noise(0.01),
        // Frequency - extremely stable
        SensorConfig::new(
            "frequency",
            "Hz",
            49.9,
            50.1,
            SignalPattern::Constant { value: 50.0 },
        )
        .with_noise(0.005),
        // Active power
        SensorConfig::new(
            "active_power",
            "kW",
            0.0,
            50.0,
            SignalPattern::Diurnal {
                min: 8.0,
                max: 35.0,
                peak_hour: 19.0,
                spread: 3.0,
            },
        )
        .with_noise(1.0),
        // Reactive power - inductive loads
        SensorConfig::new(
            "reactive_power",
            "kVAR",
            0.0,
            20.0,
            SignalPattern::Diurnal {
                min: 2.0,
                max: 10.0,
                peak_hour: 19.0,
                spread: 3.0,
            },
        )
        .with_noise(0.5),
        // Total harmonic distortion
        SensorConfig::new(
            "thd",
            "%",
            0.0,
            10.0,
            SignalPattern::Composite(vec![
                SignalPattern::Constant { value: 3.0 },
                SignalPattern::Sine {
                    amplitude: 1.0,
                    period_ms: 86_400_000,
                    phase: 0.0,
                    offset: 0.0,
                },
            ]),
        )
        .with_noise(0.3),
    ]
}

/// Industrial load profile sensors.
fn industrial_grid_sensors() -> Vec<SensorConfig> {
    vec![
        // Voltage L1
        SensorConfig::new(
            "voltage_l1",
            "V",
            220.0,
            240.0,
            SignalPattern::Constant { value: 230.0 },
        )
        .with_noise(0.8),
        // Voltage L2
        SensorConfig::new(
            "voltage_l2",
            "V",
            220.0,
            240.0,
            SignalPattern::Constant { value: 230.0 },
        )
        .with_noise(0.8),
        // Voltage L3
        SensorConfig::new(
            "voltage_l3",
            "V",
            220.0,
            240.0,
            SignalPattern::Constant { value: 230.0 },
        )
        .with_noise(0.8),
        // Current L1 - industrial daytime peak
        SensorConfig::new(
            "current_l1",
            "A",
            0.0,
            100.0,
            SignalPattern::BimodalDiurnal {
                min: 10.0,
                max: 80.0,
                peak1_hour: 10.0, // Morning shift
                peak2_hour: 14.0, // Afternoon shift
                spread: 2.0,
            },
        )
        .with_noise(3.0),
        // Power factor - often lower in industrial
        SensorConfig::new(
            "power_factor",
            "ratio",
            0.85,
            1.0,
            SignalPattern::Constant { value: 0.88 },
        )
        .with_noise(0.02),
        // Frequency
        SensorConfig::new(
            "frequency",
            "Hz",
            49.9,
            50.1,
            SignalPattern::Constant { value: 50.0 },
        )
        .with_noise(0.008),
        // Active power - high during work hours
        SensorConfig::new(
            "active_power",
            "kW",
            0.0,
            50.0,
            SignalPattern::MachineState {
                idle_value: 5.0,
                running_value: 42.0,
                startup_duration_ms: 1_800_000, // 30 min ramp
                running_duration_ms: 28_800_000, // 8 hour shift
                shutdown_duration_ms: 1_800_000,
                cycle_period_ms: 86_400_000,
            },
        )
        .with_noise(2.0),
        // Reactive power
        SensorConfig::new(
            "reactive_power",
            "kVAR",
            0.0,
            20.0,
            SignalPattern::MachineState {
                idle_value: 1.0,
                running_value: 15.0,
                startup_duration_ms: 1_800_000,
                running_duration_ms: 28_800_000,
                shutdown_duration_ms: 1_800_000,
                cycle_period_ms: 86_400_000,
            },
        )
        .with_noise(1.0),
        // THD - higher with VFDs and rectifiers
        SensorConfig::new(
            "thd",
            "%",
            0.0,
            10.0,
            SignalPattern::MachineState {
                idle_value: 2.0,
                running_value: 6.0,
                startup_duration_ms: 1_800_000,
                running_duration_ms: 28_800_000,
                shutdown_duration_ms: 1_800_000,
                cycle_period_ms: 86_400_000,
            },
        )
        .with_noise(0.5),
    ]
}

/// Get expected metrics for energy datasets.
pub fn expected_energy_metrics() -> crate::manifest::ExpectedMetrics {
    crate::manifest::ExpectedMetrics {
        compression_ratio_range: Some((0.55, 0.70)),
        tc_range: Some((3.0, 6.0)),
        r_range: Some((0.40, 0.60)),
        h_bytes_range: Some((3.5, 5.5)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_normal_sensors() {
        let sensors = create_grid_sensors(EnergyScenario::Normal);
        assert_eq!(sensors.len(), 9);

        let ids: Vec<_> = sensors.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"voltage_l1"));
        assert!(ids.contains(&"frequency"));
        assert!(ids.contains(&"thd"));
    }

    #[test]
    fn test_create_industrial_sensors() {
        let sensors = create_grid_sensors(EnergyScenario::IndustrialLoad);
        assert_eq!(sensors.len(), 9);
    }

    #[test]
    fn test_phase_imbalance() {
        let sensors = create_grid_sensors(EnergyScenario::PhaseImbalance);
        let v1 = sensors.iter().find(|s| s.id == "voltage_l1").unwrap();
        assert!(v1.anomaly.is_some());
    }

    #[test]
    fn test_frequency_deviation() {
        let sensors = create_grid_sensors(EnergyScenario::FrequencyDeviation);
        let freq = sensors.iter().find(|s| s.id == "frequency").unwrap();
        assert!(freq.anomaly.is_some());
    }
}
