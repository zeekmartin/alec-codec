// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! MetricsEngine - main orchestration for metrics computation.

use super::alignment::Aligner;
use super::config::{LogBase, MetricsConfig, SignalComputeSchedule, SignalWindow};
use super::payload::PayloadEntropyCalculator;
use super::resilience::ResilienceCalculator;
use super::signal::GaussianEntropyEstimator;
use super::snapshot::MetricsSnapshot;
use super::window::{SlidingWindow, WindowConfig};

/// Main metrics engine for the gateway.
pub struct MetricsEngine {
    config: MetricsConfig,
    window: SlidingWindow,
    aligner: Aligner,
    signal_estimator: GaussianEntropyEstimator,
    payload_calculator: PayloadEntropyCalculator,
    resilience_calculator: ResilienceCalculator,

    // Scheduling state
    flush_count: u64,
    last_signal_compute_ms: u64,
    signal_compute_count: u64,
    last_snapshot: Option<MetricsSnapshot>,
}

impl MetricsEngine {
    pub fn new(config: MetricsConfig) -> Self {
        let window_config = match config.signal_window {
            SignalWindow::TimeMillis(ms) => WindowConfig::TimeMillis(ms),
            SignalWindow::LastNSamples(n) => WindowConfig::LastNSamples(n),
        };

        let log_base = match &config.signal_estimator {
            super::config::SignalEstimator::GaussianCovariance { log_base } => *log_base,
        };

        Self {
            window: SlidingWindow::new(window_config),
            aligner: Aligner::new(config.alignment.clone(), config.missing_data.clone()),
            signal_estimator: GaussianEntropyEstimator::new(log_base, config.numerics.clone()),
            payload_calculator: PayloadEntropyCalculator::new(config.payload.clone()),
            resilience_calculator: ResilienceCalculator::new(config.resilience.clone()),
            config,
            flush_count: 0,
            last_signal_compute_ms: 0,
            signal_compute_count: 0,
            last_snapshot: None,
        }
    }

    /// Observe an incoming sample (called from Gateway::push).
    /// This is a no-op if metrics are disabled.
    pub fn observe_sample(&mut self, channel_id: &str, value: f64, timestamp_ms: u64) {
        if !self.config.enabled {
            return;
        }
        self.window.push(channel_id, value, timestamp_ms);
    }

    /// Observe a flushed frame (called from Gateway::flush).
    /// Returns a MetricsSnapshot if computation was triggered.
    pub fn observe_frame(
        &mut self,
        frame_bytes: &[u8],
        current_time_ms: u64,
    ) -> Option<MetricsSnapshot> {
        if !self.config.enabled {
            return None;
        }

        self.flush_count += 1;

        // Always compute payload metrics
        let payload = self.payload_calculator.compute(frame_bytes);

        // Check if we should compute signal metrics
        let should_compute_signal = self.should_compute_signal(current_time_ms);

        let (signal, resilience) = if should_compute_signal {
            self.last_signal_compute_ms = current_time_ms;
            self.signal_compute_count += 1;

            // Generate reference timestamps from window
            let reference_times = self.generate_reference_times(current_time_ms);
            let channel_ids: Vec<String> = self.window.channel_ids().cloned().collect();

            // Align samples
            let snapshots = self.aligner.align(&self.window, &reference_times);

            // Compute signal metrics
            let signal = self.signal_estimator.compute(&snapshots, &channel_ids);

            // Compute resilience if signal is valid
            let resilience = signal.as_ref().and_then(|s| {
                let r = self.resilience_calculator.compute(s)?;

                // Compute criticality if enabled and on schedule
                let should_compute_crit = self.config.resilience.criticality.enabled
                    && (self.signal_compute_count
                        % self.config.resilience.criticality.every_n_signal_computes as u64
                        == 0);

                if should_compute_crit {
                    let criticality = self.resilience_calculator.compute_criticality(s, r.r);
                    Some(super::resilience::ResilienceMetrics { criticality, ..r })
                } else {
                    Some(r)
                }
            });

            (signal, resilience)
        } else {
            (None, None)
        };

        // Build snapshot
        let (window_kind, window_value) = match self.config.signal_window {
            SignalWindow::TimeMillis(ms) => ("time_ms", ms),
            SignalWindow::LastNSamples(n) => ("last_n", n as u64),
        };

        let log_base_str = match &self.config.signal_estimator {
            super::config::SignalEstimator::GaussianCovariance { log_base } => match log_base {
                LogBase::E => "ln",
                LogBase::Two => "log2",
            },
        };

        let flags = self.build_flags();

        let snapshot = MetricsSnapshot::new(
            current_time_ms,
            window_kind,
            window_value,
            signal.as_ref(),
            &payload,
            resilience.as_ref(),
            log_base_str,
            flags,
        );

        self.last_snapshot = Some(snapshot.clone());
        Some(snapshot)
    }

    /// Get the last computed snapshot.
    pub fn last_snapshot(&self) -> Option<&MetricsSnapshot> {
        self.last_snapshot.as_ref()
    }

    /// Check if metrics are enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get current configuration.
    pub fn config(&self) -> &MetricsConfig {
        &self.config
    }

    /// Get the number of flushes observed.
    pub fn flush_count(&self) -> u64 {
        self.flush_count
    }

    /// Get the number of signal computations performed.
    pub fn signal_compute_count(&self) -> u64 {
        self.signal_compute_count
    }

    /// Clear all accumulated data and reset counters.
    pub fn reset(&mut self) {
        self.window.clear();
        self.flush_count = 0;
        self.last_signal_compute_ms = 0;
        self.signal_compute_count = 0;
        self.last_snapshot = None;
    }

    /// Pre-register a channel in the metrics engine.
    /// This is optional - channels are also registered on first sample.
    pub fn register_channel(&mut self, channel_id: &str) {
        self.window.register_channel(channel_id);
    }

    fn should_compute_signal(&self, current_time_ms: u64) -> bool {
        match &self.config.signal_compute {
            SignalComputeSchedule::EveryNFlushes(n) => self.flush_count % (*n as u64) == 0,
            SignalComputeSchedule::EveryMillis(ms) => {
                current_time_ms >= self.last_signal_compute_ms + ms
            }
            SignalComputeSchedule::NFlushesOrMillis { n_flushes, millis } => {
                self.flush_count % (*n_flushes as u64) == 0
                    || current_time_ms >= self.last_signal_compute_ms + millis
            }
        }
    }

    fn generate_reference_times(&self, current_time_ms: u64) -> Vec<u64> {
        // Generate evenly spaced reference times within the window
        let window_ms = match self.config.signal_window {
            SignalWindow::TimeMillis(ms) => ms,
            SignalWindow::LastNSamples(_) => 60_000, // Default to 60s for sample-based
        };

        let n_samples = self.config.numerics.min_aligned_samples;
        let start = current_time_ms.saturating_sub(window_ms);
        let step = if n_samples > 1 {
            window_ms / (n_samples as u64 - 1).max(1)
        } else {
            window_ms
        };

        (0..n_samples).map(|i| start + (i as u64) * step).collect()
    }

    fn build_flags(&self) -> Vec<String> {
        let mut flags = Vec::new();

        if self.config.normalization.enabled {
            flags.push(format!("NORMALIZED_{:?}", self.config.normalization.method));
        }

        flags.push(format!("ALIGNMENT_{:?}", self.config.alignment));

        if self.config.resilience.enabled {
            flags.push("RESILIENCE_ENABLED".to_string());
        }

        flags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_enabled_config() -> MetricsConfig {
        MetricsConfig {
            enabled: true,
            signal_compute: SignalComputeSchedule::EveryNFlushes(1),
            numerics: super::super::config::NumericsConfig {
                min_aligned_samples: 5, // Low for testing
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_engine_creation() {
        let config = MetricsConfig::default();
        let engine = MetricsEngine::new(config);
        assert!(!engine.is_enabled());
    }

    #[test]
    fn test_engine_enabled() {
        let config = create_enabled_config();
        let engine = MetricsEngine::new(config);
        assert!(engine.is_enabled());
    }

    #[test]
    fn test_observe_sample_disabled() {
        let config = MetricsConfig::default(); // disabled
        let mut engine = MetricsEngine::new(config);

        engine.observe_sample("ch1", 1.0, 1000);

        // Window should be empty since metrics are disabled
        assert!(engine.window.is_empty());
    }

    #[test]
    fn test_observe_sample_enabled() {
        let config = create_enabled_config();
        let mut engine = MetricsEngine::new(config);

        engine.observe_sample("ch1", 1.0, 1000);
        engine.observe_sample("ch1", 2.0, 2000);

        assert_eq!(engine.window.total_samples(), 2);
    }

    #[test]
    fn test_observe_frame_disabled() {
        let config = MetricsConfig::default();
        let mut engine = MetricsEngine::new(config);

        let result = engine.observe_frame(&[1, 2, 3], 1000);
        assert!(result.is_none());
    }

    #[test]
    fn test_observe_frame_enabled() {
        let config = create_enabled_config();
        let mut engine = MetricsEngine::new(config);

        // Add some samples
        for i in 0..10 {
            engine.observe_sample("ch1", i as f64, i as u64 * 1000);
        }

        let result = engine.observe_frame(&[1, 2, 3, 4, 5], 10000);
        assert!(result.is_some());

        let snapshot = result.unwrap();
        assert_eq!(snapshot.payload.frame_size_bytes, 5);
    }

    #[test]
    fn test_flush_count() {
        let config = create_enabled_config();
        let mut engine = MetricsEngine::new(config);

        assert_eq!(engine.flush_count(), 0);

        engine.observe_frame(&[], 1000);
        assert_eq!(engine.flush_count(), 1);

        engine.observe_frame(&[], 2000);
        assert_eq!(engine.flush_count(), 2);
    }

    #[test]
    fn test_signal_compute_scheduling_every_n() {
        let config = MetricsConfig {
            enabled: true,
            signal_compute: SignalComputeSchedule::EveryNFlushes(3),
            ..Default::default()
        };
        let mut engine = MetricsEngine::new(config);

        // First flush (count=1) - no compute
        engine.flush_count = 1;
        assert!(!engine.should_compute_signal(1000));

        // Third flush (count=3) - compute
        engine.flush_count = 3;
        assert!(engine.should_compute_signal(1000));

        // Sixth flush (count=6) - compute
        engine.flush_count = 6;
        assert!(engine.should_compute_signal(1000));
    }

    #[test]
    fn test_signal_compute_scheduling_millis() {
        let config = MetricsConfig {
            enabled: true,
            signal_compute: SignalComputeSchedule::EveryMillis(5000),
            ..Default::default()
        };
        let mut engine = MetricsEngine::new(config);

        engine.last_signal_compute_ms = 0;

        // Not enough time passed
        assert!(!engine.should_compute_signal(3000));

        // Enough time passed
        assert!(engine.should_compute_signal(6000));
    }

    #[test]
    fn test_last_snapshot() {
        let config = create_enabled_config();
        let mut engine = MetricsEngine::new(config);

        assert!(engine.last_snapshot().is_none());

        engine.observe_frame(&[1, 2, 3], 1000);

        assert!(engine.last_snapshot().is_some());
    }

    #[test]
    fn test_reset() {
        let config = create_enabled_config();
        let mut engine = MetricsEngine::new(config);

        engine.observe_sample("ch1", 1.0, 1000);
        engine.observe_frame(&[1, 2, 3], 2000);

        assert_eq!(engine.flush_count(), 1);
        assert!(engine.last_snapshot().is_some());

        engine.reset();

        assert_eq!(engine.flush_count(), 0);
        assert!(engine.last_snapshot().is_none());
        assert!(engine.window.is_empty());
    }

    #[test]
    fn test_build_flags() {
        let config = MetricsConfig {
            enabled: true,
            resilience: super::super::config::ResilienceConfig {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let engine = MetricsEngine::new(config);

        let flags = engine.build_flags();
        assert!(flags.iter().any(|f| f.contains("ALIGNMENT")));
        assert!(flags.iter().any(|f| f == "RESILIENCE_ENABLED"));
    }

    #[test]
    fn test_generate_reference_times() {
        let config = MetricsConfig {
            enabled: true,
            signal_window: SignalWindow::TimeMillis(10000),
            numerics: super::super::config::NumericsConfig {
                min_aligned_samples: 5,
                ..Default::default()
            },
            ..Default::default()
        };
        let engine = MetricsEngine::new(config);

        let times = engine.generate_reference_times(10000);

        assert_eq!(times.len(), 5);
        // times[0] is u64, always >= 0
        assert!(times[4] <= 10000);
    }
}
