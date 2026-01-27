// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Multi-channel sample alignment for joint entropy computation.

use super::config::{AlignmentStrategy, MissingDataPolicy};
use super::window::{Sample, SlidingWindow};
use std::collections::VecDeque;

/// An aligned multi-channel snapshot at a reference time.
#[derive(Debug, Clone)]
pub struct AlignedSnapshot {
    /// Values indexed by channel order.
    pub values: Vec<f64>,
    /// Channel IDs in order (for debugging and future extensions).
    #[allow(dead_code)]
    pub channel_ids: Vec<String>,
    /// Reference timestamp (for debugging and future extensions).
    #[allow(dead_code)]
    pub timestamp_ms: u64,
}

/// Aligner for creating multi-channel snapshots.
pub struct Aligner {
    strategy: AlignmentStrategy,
    missing_policy: MissingDataPolicy,
}

impl Aligner {
    pub fn new(strategy: AlignmentStrategy, missing_policy: MissingDataPolicy) -> Self {
        Self {
            strategy,
            missing_policy,
        }
    }

    /// Align channels at reference timestamps and return valid snapshots.
    pub fn align(&self, window: &SlidingWindow, reference_times: &[u64]) -> Vec<AlignedSnapshot> {
        let channel_ids: Vec<String> = window.channel_ids().cloned().collect();
        if channel_ids.is_empty() {
            return Vec::new();
        }

        let mut snapshots = Vec::new();

        for &t_ref in reference_times {
            let mut values = Vec::with_capacity(channel_ids.len());
            let mut valid_channels = Vec::new();
            let mut missing_count = 0;

            for channel_id in &channel_ids {
                if let Some(samples) = window.get_samples(channel_id) {
                    if let Some(value) = self.interpolate(samples, t_ref) {
                        values.push(value);
                        valid_channels.push(channel_id.clone());
                    } else {
                        missing_count += 1;
                    }
                } else {
                    missing_count += 1;
                }
            }

            // Apply missing data policy
            let include = match &self.missing_policy {
                MissingDataPolicy::DropIncompleteSnapshots => missing_count == 0,
                MissingDataPolicy::AllowPartial { min_channels } => {
                    valid_channels.len() >= *min_channels
                }
                MissingDataPolicy::FillWithLastKnown => {
                    // For now, treat as drop incomplete
                    // TODO: Implement last-known fill
                    missing_count == 0
                }
            };

            if include && !values.is_empty() {
                snapshots.push(AlignedSnapshot {
                    values,
                    channel_ids: valid_channels,
                    timestamp_ms: t_ref,
                });
            }
        }

        snapshots
    }

    fn interpolate(&self, samples: &VecDeque<Sample>, t_ref: u64) -> Option<f64> {
        if samples.is_empty() {
            return None;
        }

        match self.strategy {
            AlignmentStrategy::SampleAndHold => {
                // Find latest sample <= t_ref
                let mut result = None;
                for sample in samples.iter() {
                    if sample.timestamp_ms <= t_ref {
                        result = Some(sample.value);
                    } else {
                        break;
                    }
                }
                result
            }
            AlignmentStrategy::Nearest => {
                // Find sample closest to t_ref
                samples
                    .iter()
                    .min_by_key(|s| (s.timestamp_ms as i64 - t_ref as i64).unsigned_abs())
                    .map(|s| s.value)
            }
            AlignmentStrategy::LinearInterpolation => {
                // Find bracketing samples and interpolate
                let mut before: Option<&Sample> = None;
                let mut after: Option<&Sample> = None;

                for sample in samples.iter() {
                    if sample.timestamp_ms <= t_ref {
                        before = Some(sample);
                    } else if after.is_none() {
                        after = Some(sample);
                        break;
                    }
                }

                match (before, after) {
                    (Some(b), Some(a)) if b.timestamp_ms != a.timestamp_ms => {
                        let t_range = (a.timestamp_ms - b.timestamp_ms) as f64;
                        let t_offset = (t_ref - b.timestamp_ms) as f64;
                        let alpha = t_offset / t_range;
                        Some(b.value + alpha * (a.value - b.value))
                    }
                    (Some(b), _) => Some(b.value),
                    (_, Some(a)) => Some(a.value),
                    _ => None,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::window::WindowConfig;

    fn create_test_window() -> SlidingWindow {
        let mut window = SlidingWindow::new(WindowConfig::TimeMillis(60_000));
        window.push("ch1", 10.0, 1000);
        window.push("ch1", 20.0, 2000);
        window.push("ch1", 30.0, 3000);
        window.push("ch2", 100.0, 1000);
        window.push("ch2", 200.0, 2000);
        window.push("ch2", 300.0, 3000);
        window
    }

    #[test]
    fn test_sample_and_hold_alignment() {
        let window = create_test_window();
        let aligner = Aligner::new(
            AlignmentStrategy::SampleAndHold,
            MissingDataPolicy::DropIncompleteSnapshots,
        );

        let snapshots = aligner.align(&window, &[1500, 2500]);
        assert_eq!(snapshots.len(), 2);

        // At t=1500, should get values from t=1000
        let s0 = &snapshots[0];
        assert_eq!(s0.timestamp_ms, 1500);
        assert_eq!(s0.values.len(), 2);
    }

    #[test]
    fn test_nearest_alignment() {
        let window = create_test_window();
        let aligner = Aligner::new(
            AlignmentStrategy::Nearest,
            MissingDataPolicy::DropIncompleteSnapshots,
        );

        let snapshots = aligner.align(&window, &[1800]);
        assert_eq!(snapshots.len(), 1);

        // At t=1800, nearest is t=2000
        let s = &snapshots[0];
        assert!(s.values.contains(&20.0) || s.values.contains(&200.0));
    }

    #[test]
    fn test_linear_interpolation() {
        let mut window = SlidingWindow::new(WindowConfig::TimeMillis(60_000));
        window.push("ch1", 0.0, 0);
        window.push("ch1", 100.0, 1000);

        let aligner = Aligner::new(
            AlignmentStrategy::LinearInterpolation,
            MissingDataPolicy::DropIncompleteSnapshots,
        );

        let snapshots = aligner.align(&window, &[500]);
        assert_eq!(snapshots.len(), 1);

        // Linear interpolation: at t=500, value should be 50.0
        let s = &snapshots[0];
        assert!((s.values[0] - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_missing_data_drop() {
        let mut window = SlidingWindow::new(WindowConfig::TimeMillis(60_000));
        window.push("ch1", 10.0, 1000);
        // ch2 has no data

        let aligner = Aligner::new(
            AlignmentStrategy::SampleAndHold,
            MissingDataPolicy::DropIncompleteSnapshots,
        );

        // If we only have one channel, snapshots should be valid
        let snapshots = aligner.align(&window, &[1500]);
        assert_eq!(snapshots.len(), 1);
    }

    #[test]
    fn test_missing_data_allow_partial() {
        let mut window = SlidingWindow::new(WindowConfig::TimeMillis(60_000));
        window.push("ch1", 10.0, 1000);
        window.push("ch2", 20.0, 1000);
        // ch3 will be missing

        let aligner = Aligner::new(
            AlignmentStrategy::SampleAndHold,
            MissingDataPolicy::AllowPartial { min_channels: 2 },
        );

        let snapshots = aligner.align(&window, &[1500]);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].values.len(), 2);
    }

    #[test]
    fn test_empty_window() {
        let window = SlidingWindow::new(WindowConfig::TimeMillis(60_000));
        let aligner = Aligner::new(
            AlignmentStrategy::SampleAndHold,
            MissingDataPolicy::DropIncompleteSnapshots,
        );

        let snapshots = aligner.align(&window, &[1000, 2000]);
        assert!(snapshots.is_empty());
    }

    #[test]
    fn test_before_first_sample() {
        let mut window = SlidingWindow::new(WindowConfig::TimeMillis(60_000));
        window.push("ch1", 10.0, 1000);

        let aligner = Aligner::new(
            AlignmentStrategy::SampleAndHold,
            MissingDataPolicy::DropIncompleteSnapshots,
        );

        // Before first sample, sample-and-hold returns None
        let snapshots = aligner.align(&window, &[500]);
        assert!(snapshots.is_empty());
    }
}
