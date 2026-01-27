// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Aggregator for combining channel data into frames
//!
//! The [`Aggregator`] handles the logic of collecting encoded data from
//! multiple channels and packing them into frames that respect size constraints.

use crate::channel_manager::ChannelManager;
use crate::config::GatewayConfig;
use crate::error::Result;
use crate::frame::{Frame, FrameBuilder};

/// Aggregates data from multiple channels into frames
pub struct Aggregator {
    /// Configuration for the aggregator
    config: GatewayConfig,
}

impl Aggregator {
    /// Create a new aggregator with the given configuration
    pub fn new(config: GatewayConfig) -> Self {
        Self { config }
    }

    /// Flush all channels and aggregate into a single frame
    ///
    /// Channels are processed in priority order (lower priority value = higher priority).
    /// If the frame reaches its maximum size, lower-priority channels may be skipped.
    pub fn aggregate(&self, manager: &mut ChannelManager) -> Result<Frame> {
        let mut builder = FrameBuilder::new(self.config.max_frame_size);

        // Collect channel IDs sorted by priority
        let mut channel_ids: Vec<_> = manager.list().cloned().collect();
        channel_ids.sort_by_key(|id| {
            manager
                .get(id)
                .map(|c| c.config.priority)
                .unwrap_or(u8::MAX)
        });

        // Process channels in priority order
        for id in channel_ids {
            let channel = manager.get_mut(&id)?;
            let data = channel.flush()?;

            if !data.is_empty() && !builder.try_add(id.clone(), data) {
                // Frame is full - in future, could return multiple frames
                // For now, we just stop adding channels
                break;
            }
        }

        Ok(builder.build())
    }

    /// Flush specific channels and aggregate into a frame
    ///
    /// Only the specified channels will be flushed. Channels are processed
    /// in the order provided.
    pub fn aggregate_channels(
        &self,
        manager: &mut ChannelManager,
        channel_ids: &[&str],
    ) -> Result<Frame> {
        let mut builder = FrameBuilder::new(self.config.max_frame_size);

        for id in channel_ids {
            if let Ok(channel) = manager.get_mut(id) {
                let data = channel.flush()?;
                if !data.is_empty() && !builder.try_add(id.to_string(), data) {
                    break;
                }
            }
        }

        Ok(builder.build())
    }

    /// Get the maximum frame size
    pub fn max_frame_size(&self) -> usize {
        self.config.max_frame_size
    }

    /// Update the maximum frame size
    pub fn set_max_frame_size(&mut self, size: usize) {
        self.config.max_frame_size = size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ChannelConfig;

    #[test]
    fn test_aggregator_empty() {
        let config = GatewayConfig::default();
        let aggregator = Aggregator::new(config);
        let mut manager = ChannelManager::new(10);

        let frame = aggregator.aggregate(&mut manager).unwrap();
        assert!(frame.is_empty());
    }

    #[test]
    fn test_aggregator_single_channel() {
        let config = GatewayConfig::default();
        let aggregator = Aggregator::new(config);
        let mut manager = ChannelManager::new(10);

        manager.add("temp", ChannelConfig::default()).unwrap();
        manager.get_mut("temp").unwrap().push(22.5, 1000).unwrap();

        let frame = aggregator.aggregate(&mut manager).unwrap();
        assert_eq!(frame.channel_count(), 1);
        assert!(frame.get_channel("temp").is_some());
    }

    #[test]
    fn test_aggregator_multiple_channels() {
        let config = GatewayConfig::default();
        let aggregator = Aggregator::new(config);
        let mut manager = ChannelManager::new(10);

        manager.add("temp", ChannelConfig::default()).unwrap();
        manager.add("humid", ChannelConfig::default()).unwrap();

        manager.get_mut("temp").unwrap().push(22.5, 1000).unwrap();
        manager.get_mut("humid").unwrap().push(65.0, 1000).unwrap();

        let frame = aggregator.aggregate(&mut manager).unwrap();
        assert_eq!(frame.channel_count(), 2);
    }

    #[test]
    fn test_aggregator_priority_order() {
        let config = GatewayConfig::default();
        let aggregator = Aggregator::new(config);
        let mut manager = ChannelManager::new(10);

        // Add channels with different priorities
        manager
            .add("low", ChannelConfig::with_priority(200))
            .unwrap();
        manager
            .add("high", ChannelConfig::with_priority(1))
            .unwrap();
        manager
            .add("medium", ChannelConfig::with_priority(100))
            .unwrap();

        manager.get_mut("low").unwrap().push(1.0, 1000).unwrap();
        manager.get_mut("high").unwrap().push(2.0, 1000).unwrap();
        manager.get_mut("medium").unwrap().push(3.0, 1000).unwrap();

        let frame = aggregator.aggregate(&mut manager).unwrap();

        // All channels should be included
        assert_eq!(frame.channel_count(), 3);

        // Check order (high priority first)
        assert_eq!(frame.channels[0].id, "high");
        assert_eq!(frame.channels[1].id, "medium");
        assert_eq!(frame.channels[2].id, "low");
    }

    #[test]
    fn test_aggregator_max_size() {
        let mut config = GatewayConfig::default();
        config.max_frame_size = 50; // Very small frame
        let aggregator = Aggregator::new(config);
        let mut manager = ChannelManager::new(10);

        // Add many channels - not all will fit
        for i in 0..10 {
            let name = format!("ch{}", i);
            manager
                .add(&name, ChannelConfig::with_priority(i as u8))
                .unwrap();
            manager
                .get_mut(&name)
                .unwrap()
                .push(i as f64, 1000)
                .unwrap();
        }

        let frame = aggregator.aggregate(&mut manager).unwrap();

        // Frame size should be within limits
        assert!(frame.size() <= 50);
    }

    #[test]
    fn test_aggregator_specific_channels() {
        let config = GatewayConfig::default();
        let aggregator = Aggregator::new(config);
        let mut manager = ChannelManager::new(10);

        manager.add("temp", ChannelConfig::default()).unwrap();
        manager.add("humid", ChannelConfig::default()).unwrap();
        manager.add("pressure", ChannelConfig::default()).unwrap();

        manager.get_mut("temp").unwrap().push(22.5, 1000).unwrap();
        manager.get_mut("humid").unwrap().push(65.0, 1000).unwrap();
        manager
            .get_mut("pressure")
            .unwrap()
            .push(1013.25, 1000)
            .unwrap();

        // Only aggregate temp and pressure
        let frame = aggregator
            .aggregate_channels(&mut manager, &["temp", "pressure"])
            .unwrap();

        assert_eq!(frame.channel_count(), 2);
        assert!(frame.get_channel("temp").is_some());
        assert!(frame.get_channel("pressure").is_some());
        assert!(frame.get_channel("humid").is_none());

        // Humid should still have pending data
        assert_eq!(manager.get("humid").unwrap().pending(), 1);
    }
}
