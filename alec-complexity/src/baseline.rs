// ALEC Complexity - Standalone complexity monitoring
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Baseline building and management.

use crate::config::{BaselineConfig, BaselineUpdateMode};
use serde::{Deserialize, Serialize};

/// Current state of the baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaselineState {
    Building,
    Locked,
}

/// Statistics for a single tracked field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldStats {
    pub mean: f64,
    pub std: f64,
    pub count: u64,
    #[serde(skip)]
    pub(crate) sum: f64,
    #[serde(skip)]
    pub(crate) sum_sq: f64,
}

impl FieldStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_sample(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.sum_sq += value * value;
        self.recompute();
    }

    pub fn update_ema(&mut self, value: f64, alpha: f64) {
        self.mean = alpha * value + (1.0 - alpha) * self.mean;
        let variance = (value - self.mean).powi(2);
        let current_var = self.std * self.std;
        let new_var = alpha * variance + (1.0 - alpha) * current_var;
        self.std = new_var.sqrt();
    }

    fn recompute(&mut self) {
        if self.count == 0 {
            self.mean = 0.0;
            self.std = 0.0;
            return;
        }

        let n = self.count as f64;
        self.mean = self.sum / n;

        if self.count > 1 {
            let variance = (self.sum_sq - n * self.mean * self.mean) / (n - 1.0);
            self.std = variance.max(0.0).sqrt();
        } else {
            self.std = 0.0;
        }
    }

    pub fn is_valid(&self) -> bool {
        self.count >= 2 && self.std > 0.0
    }
}

/// Complete baseline state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub state: BaselineState,
    pub build_progress: f64,
    pub tc: FieldStats,
    pub h_joint: FieldStats,
    pub h_bytes: FieldStats,
    pub r: Option<FieldStats>,
    start_time_ms: u64,
    valid_signal_count: u32,
}

impl Baseline {
    pub fn new(track_r: bool) -> Self {
        Self {
            state: BaselineState::Building,
            build_progress: 0.0,
            tc: FieldStats::new(),
            h_joint: FieldStats::new(),
            h_bytes: FieldStats::new(),
            r: if track_r {
                Some(FieldStats::new())
            } else {
                None
            },
            start_time_ms: 0,
            valid_signal_count: 0,
        }
    }

    pub fn start(&mut self, timestamp_ms: u64) {
        self.start_time_ms = timestamp_ms;
    }

    pub fn add_sample(
        &mut self,
        tc: Option<f64>,
        h_joint: Option<f64>,
        h_bytes: f64,
        r: Option<f64>,
        timestamp_ms: u64,
        config: &BaselineConfig,
    ) {
        self.h_bytes.add_sample(h_bytes);

        if let (Some(tc_val), Some(hj_val)) = (tc, h_joint) {
            self.tc.add_sample(tc_val);
            self.h_joint.add_sample(hj_val);
            self.valid_signal_count += 1;
        }

        if let (Some(ref mut r_stats), Some(r_val)) = (&mut self.r, r) {
            r_stats.add_sample(r_val);
        }

        self.update_progress(timestamp_ms, config);
    }

    pub fn update_ema(
        &mut self,
        tc: Option<f64>,
        h_joint: Option<f64>,
        h_bytes: f64,
        r: Option<f64>,
        alpha: f64,
    ) {
        self.h_bytes.update_ema(h_bytes, alpha);

        if let (Some(tc_val), Some(hj_val)) = (tc, h_joint) {
            self.tc.update_ema(tc_val, alpha);
            self.h_joint.update_ema(hj_val, alpha);
        }

        if let (Some(ref mut r_stats), Some(r_val)) = (&mut self.r, r) {
            r_stats.update_ema(r_val, alpha);
        }
    }

    pub fn should_lock(&self, timestamp_ms: u64, config: &BaselineConfig) -> bool {
        if self.state == BaselineState::Locked {
            return false;
        }

        let time_elapsed = timestamp_ms.saturating_sub(self.start_time_ms) >= config.build_time_ms;
        let enough_samples = self.valid_signal_count >= config.min_valid_snapshots;

        time_elapsed && enough_samples
    }

    pub fn lock(&mut self) {
        self.state = BaselineState::Locked;
        self.build_progress = 1.0;
    }

    pub fn is_ready(&self) -> bool {
        self.state == BaselineState::Locked
    }

    fn update_progress(&mut self, timestamp_ms: u64, config: &BaselineConfig) {
        if self.state == BaselineState::Locked {
            return;
        }

        let time_progress = if config.build_time_ms > 0 {
            (timestamp_ms.saturating_sub(self.start_time_ms)) as f64 / config.build_time_ms as f64
        } else {
            1.0
        };

        let sample_progress = if config.min_valid_snapshots > 0 {
            self.valid_signal_count as f64 / config.min_valid_snapshots as f64
        } else {
            1.0
        };

        self.build_progress = time_progress.min(sample_progress).min(1.0);
    }
}

/// Builder for baseline (manages lifecycle).
pub struct BaselineBuilder {
    config: BaselineConfig,
    baseline: Baseline,
    initialized: bool,
}

impl BaselineBuilder {
    pub fn new(config: BaselineConfig, track_r: bool) -> Self {
        Self {
            config,
            baseline: Baseline::new(track_r),
            initialized: false,
        }
    }

    /// Process a sample. Returns true if baseline just locked.
    pub fn process(
        &mut self,
        tc: Option<f64>,
        h_joint: Option<f64>,
        h_bytes: f64,
        r: Option<f64>,
        timestamp_ms: u64,
    ) -> bool {
        if !self.initialized {
            self.baseline.start(timestamp_ms);
            self.initialized = true;
        }

        match self.baseline.state {
            BaselineState::Building => {
                self.baseline
                    .add_sample(tc, h_joint, h_bytes, r, timestamp_ms, &self.config);

                if self.baseline.should_lock(timestamp_ms, &self.config) {
                    self.baseline.lock();
                    return true;
                }
                false
            }
            BaselineState::Locked => {
                match &self.config.update_mode {
                    BaselineUpdateMode::Ema { alpha } => {
                        let alpha_f = *alpha as f64 * 0.01;
                        self.baseline.update_ema(tc, h_joint, h_bytes, r, alpha_f);
                    }
                    BaselineUpdateMode::Rolling => {
                        // Future implementation
                    }
                    BaselineUpdateMode::Frozen => {}
                }
                false
            }
        }
    }

    pub fn baseline(&self) -> &Baseline {
        &self.baseline
    }

    pub fn baseline_mut(&mut self) -> &mut Baseline {
        &mut self.baseline
    }

    pub fn export(&self) -> Baseline {
        self.baseline.clone()
    }

    pub fn import(&mut self, baseline: Baseline) {
        self.baseline = baseline;
        self.initialized = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_stats_new() {
        let stats = FieldStats::new();
        assert_eq!(stats.count, 0);
        assert_eq!(stats.mean, 0.0);
        assert_eq!(stats.std, 0.0);
    }

    #[test]
    fn test_field_stats_add_sample() {
        let mut stats = FieldStats::new();
        stats.add_sample(10.0);
        stats.add_sample(20.0);
        stats.add_sample(30.0);

        assert_eq!(stats.count, 3);
        assert!((stats.mean - 20.0).abs() < 0.001);
        assert!(stats.std > 0.0);
    }

    #[test]
    fn test_field_stats_is_valid() {
        let mut stats = FieldStats::new();
        assert!(!stats.is_valid());

        stats.add_sample(10.0);
        assert!(!stats.is_valid()); // Need at least 2 samples

        stats.add_sample(20.0);
        assert!(stats.is_valid());
    }

    #[test]
    fn test_baseline_new() {
        let baseline = Baseline::new(true);
        assert_eq!(baseline.state, BaselineState::Building);
        assert!(baseline.r.is_some());
    }

    #[test]
    fn test_baseline_without_r() {
        let baseline = Baseline::new(false);
        assert!(baseline.r.is_none());
    }

    #[test]
    fn test_baseline_building() {
        let config = BaselineConfig {
            build_time_ms: 10_000,
            min_valid_snapshots: 5,
            ..Default::default()
        };

        let mut builder = BaselineBuilder::new(config, true);

        // Add samples
        for i in 0..5 {
            let locked = builder.process(Some(2.0), Some(8.0), 5.0, Some(0.45), i * 3000);
            if i < 4 {
                assert!(!locked);
                assert_eq!(builder.baseline().state, BaselineState::Building);
            }
        }
    }

    #[test]
    fn test_baseline_locking() {
        let config = BaselineConfig {
            build_time_ms: 1000,
            min_valid_snapshots: 3,
            ..Default::default()
        };

        let mut builder = BaselineBuilder::new(config, true);

        // Need both time and sample count
        builder.process(Some(2.0), Some(8.0), 5.0, Some(0.45), 0);
        builder.process(Some(2.0), Some(8.0), 5.0, Some(0.45), 400);
        builder.process(Some(2.0), Some(8.0), 5.0, Some(0.45), 800);
        assert!(!builder.baseline().is_ready()); // Not enough time

        let locked = builder.process(Some(2.0), Some(8.0), 5.0, Some(0.45), 1500);
        assert!(locked);
        assert!(builder.baseline().is_ready());
    }

    #[test]
    fn test_baseline_progress() {
        let config = BaselineConfig {
            build_time_ms: 10_000,
            min_valid_snapshots: 10,
            ..Default::default()
        };

        let mut builder = BaselineBuilder::new(config, true);

        // At timestamp 0, progress should be >= 0 (starts at 0)
        builder.process(Some(2.0), Some(8.0), 5.0, None, 0);
        let initial_progress = builder.baseline().build_progress;
        assert!(initial_progress >= 0.0);
        assert!(initial_progress < 1.0);

        // Process more samples, progress should increase
        builder.process(Some(2.0), Some(8.0), 5.0, None, 5000);
        let mid_progress = builder.baseline().build_progress;
        assert!(mid_progress > initial_progress);

        for i in 3..10 {
            builder.process(Some(2.0), Some(8.0), 5.0, None, i * 1500);
        }
    }

    #[test]
    fn test_baseline_ema_update() {
        let config = BaselineConfig {
            build_time_ms: 100,
            min_valid_snapshots: 2,
            update_mode: BaselineUpdateMode::Ema { alpha: 20 },
            ..Default::default()
        };

        let mut builder = BaselineBuilder::new(config, false);

        // Build baseline
        builder.process(Some(2.0), Some(8.0), 5.0, None, 0);
        builder.process(Some(2.0), Some(8.0), 5.0, None, 200);
        assert!(builder.baseline().is_ready());

        let old_mean = builder.baseline().h_bytes.mean;

        // Update with EMA
        builder.process(Some(2.0), Some(8.0), 10.0, None, 300);
        assert!(builder.baseline().h_bytes.mean > old_mean);
    }

    #[test]
    fn test_baseline_export_import() {
        let config = BaselineConfig {
            build_time_ms: 100,
            min_valid_snapshots: 2,
            ..Default::default()
        };

        let mut builder1 = BaselineBuilder::new(config.clone(), true);
        builder1.process(Some(2.0), Some(8.0), 5.0, Some(0.45), 0);
        builder1.process(Some(2.0), Some(8.0), 5.0, Some(0.45), 200);

        let exported = builder1.export();

        let mut builder2 = BaselineBuilder::new(config, true);
        builder2.import(exported);

        assert_eq!(builder1.baseline().state, builder2.baseline().state);
        assert_eq!(
            builder1.baseline().h_bytes.count,
            builder2.baseline().h_bytes.count
        );
    }
}
