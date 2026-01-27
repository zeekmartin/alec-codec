// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Sliding window management for signal samples.

use std::collections::{HashMap, VecDeque};

/// A timestamped sample.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    pub value: f64,
    pub timestamp_ms: u64,
}

/// Per-channel sliding window of samples.
#[derive(Debug)]
pub struct SlidingWindow {
    /// Samples per channel, ordered by timestamp.
    channels: HashMap<String, VecDeque<Sample>>,
    /// Window configuration.
    config: WindowConfig,
}

#[derive(Debug, Clone)]
pub enum WindowConfig {
    TimeMillis(u64),
    LastNSamples(usize),
}

impl SlidingWindow {
    pub fn new(config: WindowConfig) -> Self {
        Self {
            channels: HashMap::new(),
            config,
        }
    }

    /// Add a sample to a channel's window.
    pub fn push(&mut self, channel_id: &str, value: f64, timestamp_ms: u64) {
        let samples = self
            .channels
            .entry(channel_id.to_string())
            .or_default();

        samples.push_back(Sample {
            value,
            timestamp_ms,
        });

        // Prune based on config
        self.prune_channel(channel_id, timestamp_ms);
    }

    /// Get all samples for a channel within the window.
    pub fn get_samples(&self, channel_id: &str) -> Option<&VecDeque<Sample>> {
        self.channels.get(channel_id)
    }

    /// Get all channel IDs with data.
    pub fn channel_ids(&self) -> impl Iterator<Item = &String> {
        self.channels.keys()
    }

    /// Get the number of channels.
    #[allow(dead_code)]
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Get total sample count across all channels.
    #[allow(dead_code)]
    pub fn total_samples(&self) -> usize {
        self.channels.values().map(|s| s.len()).sum()
    }

    /// Check if the window is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty() || self.channels.values().all(|s| s.is_empty())
    }

    /// Clear all data.
    pub fn clear(&mut self) {
        self.channels.clear();
    }

    /// Pre-register a channel (creates an empty sample queue).
    pub fn register_channel(&mut self, channel_id: &str) {
        self.channels
            .entry(channel_id.to_string())
            .or_default();
    }

    /// Get the time range of samples in the window.
    #[allow(dead_code)]
    pub fn time_range(&self) -> Option<(u64, u64)> {
        let mut min_time = u64::MAX;
        let mut max_time = 0u64;

        for samples in self.channels.values() {
            if let Some(first) = samples.front() {
                min_time = min_time.min(first.timestamp_ms);
            }
            if let Some(last) = samples.back() {
                max_time = max_time.max(last.timestamp_ms);
            }
        }

        if min_time == u64::MAX || max_time == 0 {
            None
        } else {
            Some((min_time, max_time))
        }
    }

    fn prune_channel(&mut self, channel_id: &str, current_time_ms: u64) {
        if let Some(samples) = self.channels.get_mut(channel_id) {
            match self.config {
                WindowConfig::TimeMillis(window_ms) => {
                    let cutoff = current_time_ms.saturating_sub(window_ms);
                    while let Some(front) = samples.front() {
                        if front.timestamp_ms < cutoff {
                            samples.pop_front();
                        } else {
                            break;
                        }
                    }
                }
                WindowConfig::LastNSamples(n) => {
                    while samples.len() > n {
                        samples.pop_front();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_new() {
        let window = SlidingWindow::new(WindowConfig::TimeMillis(60_000));
        assert!(window.is_empty());
        assert_eq!(window.channel_count(), 0);
    }

    #[test]
    fn test_window_push() {
        let mut window = SlidingWindow::new(WindowConfig::TimeMillis(60_000));
        window.push("ch1", 1.0, 1000);
        window.push("ch1", 2.0, 2000);

        assert_eq!(window.channel_count(), 1);
        assert_eq!(window.total_samples(), 2);
    }

    #[test]
    fn test_window_time_based_pruning() {
        let mut window = SlidingWindow::new(WindowConfig::TimeMillis(5000));

        window.push("ch1", 1.0, 1000);
        window.push("ch1", 2.0, 3000);
        window.push("ch1", 3.0, 8000); // Should prune first sample

        let samples = window.get_samples("ch1").unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples.front().unwrap().timestamp_ms, 3000);
    }

    #[test]
    fn test_window_count_based_pruning() {
        let mut window = SlidingWindow::new(WindowConfig::LastNSamples(3));

        for i in 0..5 {
            window.push("ch1", i as f64, i as u64 * 1000);
        }

        let samples = window.get_samples("ch1").unwrap();
        assert_eq!(samples.len(), 3);
        assert_eq!(samples.front().unwrap().value, 2.0);
    }

    #[test]
    fn test_window_multi_channel() {
        let mut window = SlidingWindow::new(WindowConfig::TimeMillis(60_000));
        window.push("ch1", 1.0, 1000);
        window.push("ch2", 2.0, 1000);
        window.push("ch3", 3.0, 1000);

        assert_eq!(window.channel_count(), 3);
        assert_eq!(window.total_samples(), 3);
    }

    #[test]
    fn test_window_time_range() {
        let mut window = SlidingWindow::new(WindowConfig::TimeMillis(60_000));
        window.push("ch1", 1.0, 1000);
        window.push("ch2", 2.0, 5000);
        window.push("ch1", 3.0, 3000);

        let (min, max) = window.time_range().unwrap();
        assert_eq!(min, 1000);
        assert_eq!(max, 5000);
    }

    #[test]
    fn test_window_clear() {
        let mut window = SlidingWindow::new(WindowConfig::TimeMillis(60_000));
        window.push("ch1", 1.0, 1000);
        window.push("ch2", 2.0, 2000);

        window.clear();
        assert!(window.is_empty());
        assert_eq!(window.channel_count(), 0);
    }
}
