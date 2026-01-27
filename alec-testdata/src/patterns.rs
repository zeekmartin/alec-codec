// ALEC Testdata - Signal patterns
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Signal pattern generators for realistic sensor data.
//!
//! This module provides various signal patterns that can be combined
//! to create realistic sensor behaviors.

use rand::prelude::*;
use rand_distr::{LogNormal, Normal, Poisson};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Signal pattern definition.
///
/// Patterns can be combined using `Composite` to create complex behaviors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalPattern {
    /// Constant value with optional noise.
    Constant { value: f64 },

    /// Sinusoidal wave.
    ///
    /// `value = offset + amplitude * sin(2*PI*t/period_ms + phase)`
    Sine {
        amplitude: f64,
        period_ms: u64,
        phase: f64,
        offset: f64,
    },

    /// Linear trend.
    ///
    /// `value = start + slope_per_ms * t`
    Linear { start: f64, slope_per_ms: f64 },

    /// Random walk (Brownian motion).
    RandomWalk { start: f64, step_std: f64 },

    /// Step function with predefined level changes.
    ///
    /// Levels are (timestamp_ms, value) pairs. The value persists
    /// until the next timestamp.
    Step { levels: Vec<(u64, f64)> },

    /// Exponential decay toward target.
    ///
    /// `value = target + (start - target) * exp(-t/tau_ms)`
    Decay {
        start: f64,
        target: f64,
        tau_ms: f64,
    },

    /// Exponential growth toward target.
    ///
    /// `value = start + (target - start) * (1 - exp(-t/tau_ms))`
    Growth {
        start: f64,
        target: f64,
        tau_ms: f64,
    },

    /// Sawtooth wave (linear ramp with reset).
    Sawtooth {
        min: f64,
        max: f64,
        period_ms: u64,
        ascending: bool,
    },

    /// Log-normal distributed values (good for wind, traffic).
    LogNormal { mu: f64, sigma: f64 },

    /// Poisson events (good for rain, sparse events).
    Poisson { lambda: f64, scale: f64 },

    /// Diurnal pattern (24-hour cycle with customizable shape).
    Diurnal {
        min: f64,
        max: f64,
        peak_hour: f64,
        spread: f64,
    },

    /// Bimodal diurnal (two peaks, good for traffic).
    BimodalDiurnal {
        min: f64,
        max: f64,
        peak1_hour: f64,
        peak2_hour: f64,
        spread: f64,
    },

    /// Binary state (0 or 1) with transition probabilities.
    Binary {
        initial: bool,
        p_on: f64,
        p_off: f64,
    },

    /// Correlated with another sensor's output.
    ///
    /// Note: This is resolved during generation, not stored.
    #[serde(skip)]
    Correlated {
        source_id: String,
        scale: f64,
        offset: f64,
        lag_samples: usize,
        noise_std: f64,
    },

    /// Inverse correlation (e.g., humidity vs temperature).
    #[serde(skip)]
    InverseCorrelated {
        source_id: String,
        source_range: (f64, f64),
        target_range: (f64, f64),
        noise_std: f64,
    },

    /// Composite: sum of multiple patterns.
    Composite(Vec<SignalPattern>),

    /// Machine state pattern (idle -> startup -> running -> shutdown).
    MachineState {
        idle_value: f64,
        running_value: f64,
        startup_duration_ms: u64,
        running_duration_ms: u64,
        shutdown_duration_ms: u64,
        cycle_period_ms: u64,
    },

    /// GPS trajectory following a path.
    #[serde(skip)]
    GpsTrajectory {
        waypoints: Vec<(f64, f64)>,
        speed_mps: f64,
        noise_meters: f64,
        is_latitude: bool,
    },
}

impl SignalPattern {
    /// Evaluate the pattern at a given timestamp.
    ///
    /// For patterns that need state (RandomWalk, Binary), use `evaluate_stateful`.
    pub fn evaluate(&self, timestamp_ms: u64, rng: &mut (impl Rng + ?Sized)) -> f64 {
        match self {
            SignalPattern::Constant { value } => *value,

            SignalPattern::Sine {
                amplitude,
                period_ms,
                phase,
                offset,
            } => {
                let t = timestamp_ms as f64;
                let period = *period_ms as f64;
                offset + amplitude * (2.0 * PI * t / period + phase).sin()
            }

            SignalPattern::Linear {
                start,
                slope_per_ms,
            } => start + slope_per_ms * timestamp_ms as f64,

            SignalPattern::RandomWalk { start, step_std } => {
                // For stateless evaluation, we return start + noise
                // True random walk requires stateful evaluation
                let normal = Normal::new(0.0, *step_std).unwrap();
                start + normal.sample(rng)
            }

            SignalPattern::Step { levels } => {
                // Find the current level
                let mut current_value = levels.first().map(|(_, v)| *v).unwrap_or(0.0);
                for (ts, val) in levels {
                    if timestamp_ms >= *ts {
                        current_value = *val;
                    } else {
                        break;
                    }
                }
                current_value
            }

            SignalPattern::Decay {
                start,
                target,
                tau_ms,
            } => {
                let t = timestamp_ms as f64;
                target + (start - target) * (-t / tau_ms).exp()
            }

            SignalPattern::Growth {
                start,
                target,
                tau_ms,
            } => {
                let t = timestamp_ms as f64;
                start + (target - start) * (1.0 - (-t / tau_ms).exp())
            }

            SignalPattern::Sawtooth {
                min,
                max,
                period_ms,
                ascending,
            } => {
                let t = timestamp_ms % period_ms;
                let fraction = t as f64 / *period_ms as f64;
                if *ascending {
                    min + (max - min) * fraction
                } else {
                    max - (max - min) * fraction
                }
            }

            SignalPattern::LogNormal { mu, sigma } => {
                let dist = LogNormal::new(*mu, *sigma).unwrap();
                dist.sample(rng)
            }

            SignalPattern::Poisson { lambda, scale } => {
                if *lambda <= 0.0 {
                    return 0.0;
                }
                let dist = Poisson::new(*lambda).unwrap();
                dist.sample(rng) * scale
            }

            SignalPattern::Diurnal {
                min,
                max,
                peak_hour,
                spread,
            } => {
                // Convert timestamp to hour of day
                let hour = (timestamp_ms as f64 / 3_600_000.0) % 24.0;
                // Gaussian-like curve around peak
                let diff = (hour - peak_hour).abs();
                let diff = if diff > 12.0 { 24.0 - diff } else { diff };
                let factor = (-diff * diff / (2.0 * spread * spread)).exp();
                min + (max - min) * factor
            }

            SignalPattern::BimodalDiurnal {
                min,
                max,
                peak1_hour,
                peak2_hour,
                spread,
            } => {
                let hour = (timestamp_ms as f64 / 3_600_000.0) % 24.0;

                // Two Gaussian curves
                let diff1 = (hour - peak1_hour).abs();
                let diff1 = if diff1 > 12.0 { 24.0 - diff1 } else { diff1 };
                let factor1 = (-diff1 * diff1 / (2.0 * spread * spread)).exp();

                let diff2 = (hour - peak2_hour).abs();
                let diff2 = if diff2 > 12.0 { 24.0 - diff2 } else { diff2 };
                let factor2 = (-diff2 * diff2 / (2.0 * spread * spread)).exp();

                let factor = factor1.max(factor2);
                min + (max - min) * factor
            }

            SignalPattern::Binary {
                initial: _,
                p_on,
                p_off,
            } => {
                // Stateless approximation - use probability
                let steady_state = p_on / (p_on + p_off);
                if rng.gen::<f64>() < steady_state {
                    1.0
                } else {
                    0.0
                }
            }

            SignalPattern::Correlated { .. } | SignalPattern::InverseCorrelated { .. } => {
                // These need to be resolved during generation
                0.0
            }

            SignalPattern::Composite(patterns) => {
                patterns.iter().map(|p| p.evaluate(timestamp_ms, rng)).sum()
            }

            SignalPattern::MachineState {
                idle_value,
                running_value,
                startup_duration_ms,
                running_duration_ms,
                shutdown_duration_ms,
                cycle_period_ms,
            } => {
                let t = timestamp_ms % cycle_period_ms;
                let startup_end = *startup_duration_ms;
                let running_end = startup_end + running_duration_ms;
                let shutdown_end = running_end + shutdown_duration_ms;

                if t < startup_end {
                    // Startup: ramp from idle to running
                    let progress = t as f64 / *startup_duration_ms as f64;
                    idle_value + (running_value - idle_value) * progress
                } else if t < running_end {
                    // Running
                    *running_value
                } else if t < shutdown_end {
                    // Shutdown: ramp from running to idle
                    let progress = (t - running_end) as f64 / *shutdown_duration_ms as f64;
                    running_value + (idle_value - running_value) * progress
                } else {
                    // Idle
                    *idle_value
                }
            }

            SignalPattern::GpsTrajectory { .. } => {
                // Needs stateful evaluation
                0.0
            }
        }
    }

    /// Create a diurnal temperature pattern.
    pub fn temperature_diurnal(min: f64, max: f64) -> Self {
        SignalPattern::Diurnal {
            min,
            max,
            peak_hour: 14.0, // Peak at 2 PM
            spread: 4.0,
        }
    }

    /// Create a traffic pattern with morning and evening rush hours.
    pub fn traffic_diurnal(min: f64, max: f64) -> Self {
        SignalPattern::BimodalDiurnal {
            min,
            max,
            peak1_hour: 8.0,  // Morning rush
            peak2_hour: 18.0, // Evening rush
            spread: 2.0,
        }
    }

    /// Create a solar radiation pattern (bell curve during day, zero at night).
    pub fn solar_radiation(max_watts: f64) -> Self {
        SignalPattern::Diurnal {
            min: 0.0,
            max: max_watts,
            peak_hour: 12.0, // Peak at noon
            spread: 3.0,
        }
    }

    /// Create a battery discharge pattern (sawtooth).
    pub fn battery_discharge(full: f64, empty: f64, cycle_hours: f64) -> Self {
        SignalPattern::Sawtooth {
            min: empty,
            max: full,
            period_ms: (cycle_hours * 3_600_000.0) as u64,
            ascending: false,
        }
    }
}

/// State for patterns that need history.
#[derive(Debug, Clone)]
pub struct PatternState {
    /// Current value for random walk.
    pub random_walk_value: f64,
    /// Current binary state.
    pub binary_state: bool,
    /// GPS trajectory progress (0.0 to 1.0).
    pub trajectory_progress: f64,
    /// Previous values for correlation.
    pub history: Vec<f64>,
}

impl Default for PatternState {
    fn default() -> Self {
        Self {
            random_walk_value: 0.0,
            binary_state: false,
            trajectory_progress: 0.0,
            history: Vec::new(),
        }
    }
}

impl PatternState {
    /// Create state initialized for a pattern.
    pub fn for_pattern(pattern: &SignalPattern) -> Self {
        let mut state = Self::default();
        match pattern {
            SignalPattern::RandomWalk { start, .. } => {
                state.random_walk_value = *start;
            }
            SignalPattern::Binary { initial, .. } => {
                state.binary_state = *initial;
            }
            _ => {}
        }
        state
    }

    /// Evaluate pattern with state update.
    pub fn evaluate(
        &mut self,
        pattern: &SignalPattern,
        timestamp_ms: u64,
        rng: &mut (impl Rng + ?Sized),
    ) -> f64 {
        match pattern {
            SignalPattern::RandomWalk { step_std, .. } => {
                let normal = Normal::new(0.0, *step_std).unwrap();
                self.random_walk_value += normal.sample(rng);
                self.random_walk_value
            }

            SignalPattern::Binary { p_on, p_off, .. } => {
                let p = if self.binary_state { *p_off } else { *p_on };
                if rng.gen::<f64>() < p {
                    self.binary_state = !self.binary_state;
                }
                if self.binary_state {
                    1.0
                } else {
                    0.0
                }
            }

            SignalPattern::GpsTrajectory {
                waypoints,
                speed_mps,
                noise_meters,
                is_latitude,
            } => {
                if waypoints.is_empty() {
                    return 0.0;
                }

                // Calculate total path length
                let mut total_length = 0.0;
                for i in 1..waypoints.len() {
                    let (lat1, lon1) = waypoints[i - 1];
                    let (lat2, lon2) = waypoints[i];
                    total_length += haversine_distance(lat1, lon1, lat2, lon2);
                }

                // Update progress based on speed
                let dt_hours = 1.0 / 3600.0; // Assume 1-second updates
                let distance_moved = speed_mps * dt_hours * 3600.0 / 1000.0; // km
                self.trajectory_progress += distance_moved / total_length.max(0.001);

                if self.trajectory_progress >= 1.0 {
                    self.trajectory_progress = 0.0;
                }

                // Find current position
                let pos = interpolate_path(waypoints, self.trajectory_progress);
                let noise = Normal::new(0.0, *noise_meters / 111_000.0).unwrap(); // degrees

                if *is_latitude {
                    pos.0 + noise.sample(rng)
                } else {
                    pos.1 + noise.sample(rng)
                }
            }

            // For other patterns, use stateless evaluation
            _ => pattern.evaluate(timestamp_ms, rng),
        }
    }
}

/// Calculate haversine distance in km.
fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0; // Earth radius in km
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    r * c
}

/// Interpolate position along a path.
fn interpolate_path(waypoints: &[(f64, f64)], progress: f64) -> (f64, f64) {
    if waypoints.is_empty() {
        return (0.0, 0.0);
    }
    if waypoints.len() == 1 {
        return waypoints[0];
    }

    let n = waypoints.len() - 1;
    let segment_progress = progress * n as f64;
    let segment = (segment_progress as usize).min(n - 1);
    let t = segment_progress - segment as f64;

    let (lat1, lon1) = waypoints[segment];
    let (lat2, lon2) = waypoints[segment + 1];

    (lat1 + (lat2 - lat1) * t, lon1 + (lon2 - lon1) * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn test_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    #[test]
    fn test_constant() {
        let mut rng = test_rng();
        let pattern = SignalPattern::Constant { value: 5.0 };
        assert_eq!(pattern.evaluate(0, &mut rng), 5.0);
        assert_eq!(pattern.evaluate(1000, &mut rng), 5.0);
    }

    #[test]
    fn test_sine() {
        let mut rng = test_rng();
        let pattern = SignalPattern::Sine {
            amplitude: 10.0,
            period_ms: 1000,
            phase: 0.0,
            offset: 20.0,
        };

        // At t=0, sin(0) = 0, so value = 20
        let v0 = pattern.evaluate(0, &mut rng);
        assert!((v0 - 20.0).abs() < 0.001);

        // At t=250 (quarter period), sin(PI/2) = 1, so value = 30
        let v250 = pattern.evaluate(250, &mut rng);
        assert!((v250 - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_linear() {
        let mut rng = test_rng();
        let pattern = SignalPattern::Linear {
            start: 10.0,
            slope_per_ms: 0.001,
        };

        assert_eq!(pattern.evaluate(0, &mut rng), 10.0);
        assert_eq!(pattern.evaluate(1000, &mut rng), 11.0);
    }

    #[test]
    fn test_decay() {
        let mut rng = test_rng();
        let pattern = SignalPattern::Decay {
            start: 100.0,
            target: 0.0,
            tau_ms: 1000.0,
        };

        // At t=0, value = 100
        assert!((pattern.evaluate(0, &mut rng) - 100.0).abs() < 0.001);

        // At t=tau, value = 100 * e^-1 ≈ 36.8
        let v_tau = pattern.evaluate(1000, &mut rng);
        assert!((v_tau - 36.788).abs() < 0.1);
    }

    #[test]
    fn test_step() {
        let mut rng = test_rng();
        let pattern = SignalPattern::Step {
            levels: vec![(0, 10.0), (500, 20.0), (1000, 15.0)],
        };

        assert_eq!(pattern.evaluate(0, &mut rng), 10.0);
        assert_eq!(pattern.evaluate(499, &mut rng), 10.0);
        assert_eq!(pattern.evaluate(500, &mut rng), 20.0);
        assert_eq!(pattern.evaluate(999, &mut rng), 20.0);
        assert_eq!(pattern.evaluate(1000, &mut rng), 15.0);
    }

    #[test]
    fn test_diurnal() {
        let mut rng = test_rng();
        let pattern = SignalPattern::Diurnal {
            min: 10.0,
            max: 30.0,
            peak_hour: 12.0,
            spread: 4.0,
        };

        // At midnight (hour 0), should be close to min
        let v_midnight = pattern.evaluate(0, &mut rng);
        assert!(v_midnight < 15.0);

        // At noon (hour 12), should be close to max
        let noon_ms = 12 * 3_600_000;
        let v_noon = pattern.evaluate(noon_ms, &mut rng);
        assert!((v_noon - 30.0).abs() < 0.1);
    }

    #[test]
    fn test_sawtooth() {
        let mut rng = test_rng();
        let pattern = SignalPattern::Sawtooth {
            min: 0.0,
            max: 100.0,
            period_ms: 1000,
            ascending: true,
        };

        assert_eq!(pattern.evaluate(0, &mut rng), 0.0);
        assert_eq!(pattern.evaluate(500, &mut rng), 50.0);
        // At 999ms we're almost at max
        assert!((pattern.evaluate(999, &mut rng) - 99.9).abs() < 0.2);
    }

    #[test]
    fn test_composite() {
        let mut rng = test_rng();
        let pattern = SignalPattern::Composite(vec![
            SignalPattern::Constant { value: 10.0 },
            SignalPattern::Constant { value: 5.0 },
        ]);

        assert_eq!(pattern.evaluate(0, &mut rng), 15.0);
    }

    #[test]
    fn test_random_walk_stateful() {
        let mut rng = test_rng();
        let pattern = SignalPattern::RandomWalk {
            start: 50.0,
            step_std: 1.0,
        };

        let mut state = PatternState::for_pattern(&pattern);

        // Initial value should be start
        assert_eq!(state.random_walk_value, 50.0);

        // Values should change
        let v1 = state.evaluate(&pattern, 0, &mut rng);
        let v2 = state.evaluate(&pattern, 1000, &mut rng);
        let v3 = state.evaluate(&pattern, 2000, &mut rng);

        // Should accumulate (not reset)
        assert!(v1 != v2 || v2 != v3);
    }

    #[test]
    fn test_binary_stateful() {
        let mut rng = test_rng();
        let pattern = SignalPattern::Binary {
            initial: false,
            p_on: 0.1,
            p_off: 0.1,
        };

        let mut state = PatternState::for_pattern(&pattern);
        assert!(!state.binary_state);

        // Run many iterations, should eventually transition
        let mut saw_on = false;
        let mut saw_off = false;
        for i in 0..100 {
            let v = state.evaluate(&pattern, i * 1000, &mut rng);
            if v > 0.5 {
                saw_on = true;
            } else {
                saw_off = true;
            }
        }
        assert!(saw_on || saw_off); // Should see at least one state
    }

    #[test]
    fn test_machine_state() {
        let mut rng = test_rng();
        let pattern = SignalPattern::MachineState {
            idle_value: 0.0,
            running_value: 100.0,
            startup_duration_ms: 1000,
            running_duration_ms: 5000,
            shutdown_duration_ms: 1000,
            cycle_period_ms: 10000,
        };

        // At t=0 (start of startup), should be idle
        assert_eq!(pattern.evaluate(0, &mut rng), 0.0);

        // At t=500 (mid startup), should be 50
        assert_eq!(pattern.evaluate(500, &mut rng), 50.0);

        // At t=1000 (end of startup / start running), should be 100
        assert_eq!(pattern.evaluate(1000, &mut rng), 100.0);

        // At t=3000 (middle of running), should be 100
        assert_eq!(pattern.evaluate(3000, &mut rng), 100.0);

        // At t=6500 (mid shutdown), should be 50
        assert_eq!(pattern.evaluate(6500, &mut rng), 50.0);

        // At t=8000 (idle phase), should be 0
        assert_eq!(pattern.evaluate(8000, &mut rng), 0.0);
    }

    #[test]
    fn test_haversine() {
        // New York to Los Angeles (approximate)
        let d = haversine_distance(40.7128, -74.0060, 34.0522, -118.2437);
        // Should be around 3940 km
        assert!((d - 3940.0).abs() < 100.0);
    }
}
