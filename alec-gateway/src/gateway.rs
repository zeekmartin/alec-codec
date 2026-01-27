// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! High-level Gateway API
//!
//! The [`Gateway`] struct provides a simple, high-level interface for managing
//! multiple sensor channels and aggregating their data into transmission frames.
//!
//! # Example
//!
//! ```rust
//! use alec_gateway::{Gateway, ChannelConfig, GatewayConfig};
//!
//! let config = GatewayConfig {
//!     max_frame_size: 242, // LoRaWAN
//!     ..Default::default()
//! };
//!
//! let mut gateway = Gateway::with_config(config);
//!
//! gateway.add_channel("temp", ChannelConfig::default()).unwrap();
//! gateway.push("temp", 22.5, 1000).unwrap();
//!
//! let frame = gateway.flush().unwrap();
//! // Send frame.to_bytes() over LoRaWAN, MQTT, etc.
//! ```

use crate::aggregator::Aggregator;
use crate::channel_manager::ChannelManager;
use crate::config::{ChannelConfig, GatewayConfig};
use crate::error::Result;
use crate::frame::Frame;

/// High-level API for managing sensor channels
pub struct Gateway {
    /// Channel manager
    manager: ChannelManager,
    /// Aggregator for combining channel data
    aggregator: Aggregator,
    /// Gateway configuration
    config: GatewayConfig,
}

impl Gateway {
    /// Create a new gateway with default configuration
    pub fn new() -> Self {
        Self::with_config(GatewayConfig::default())
    }

    /// Create a new gateway with custom configuration
    pub fn with_config(config: GatewayConfig) -> Self {
        Self {
            manager: ChannelManager::new(config.max_channels),
            aggregator: Aggregator::new(config.clone()),
            config,
        }
    }

    /// Add a new sensor channel
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the channel
    /// * `config` - Channel configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A channel with the same ID already exists
    /// - The maximum number of channels has been reached
    /// - The preload file (if specified) cannot be loaded
    pub fn add_channel(&mut self, id: impl Into<String>, config: ChannelConfig) -> Result<()> {
        self.manager.add(id, config)
    }

    /// Remove a channel
    ///
    /// # Arguments
    ///
    /// * `id` - ID of the channel to remove
    ///
    /// # Errors
    ///
    /// Returns an error if the channel does not exist.
    pub fn remove_channel(&mut self, id: &str) -> Result<()> {
        self.manager.remove(id)?;
        Ok(())
    }

    /// Push a value to a channel
    ///
    /// # Arguments
    ///
    /// * `channel_id` - ID of the target channel
    /// * `value` - Sensor value to push
    /// * `timestamp` - Timestamp of the measurement
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The channel does not exist
    /// - The channel's buffer is full
    pub fn push(&mut self, channel_id: &str, value: f64, timestamp: u64) -> Result<()> {
        self.manager.get_mut(channel_id)?.push(value, timestamp)
    }

    /// Push multiple values to a channel
    ///
    /// # Arguments
    ///
    /// * `channel_id` - ID of the target channel
    /// * `values` - Slice of (value, timestamp) tuples
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The channel does not exist
    /// - The channel's buffer becomes full
    pub fn push_multi(&mut self, channel_id: &str, values: &[(f64, u64)]) -> Result<()> {
        let channel = self.manager.get_mut(channel_id)?;
        for (value, timestamp) in values {
            channel.push(*value, *timestamp)?;
        }
        Ok(())
    }

    /// Flush all channels and return aggregated frame
    ///
    /// Channels are processed in priority order. The frame respects the
    /// configured maximum size.
    pub fn flush(&mut self) -> Result<Frame> {
        self.aggregator.aggregate(&mut self.manager)
    }

    /// Flush specific channels and return aggregated frame
    ///
    /// Only the specified channels will be flushed.
    pub fn flush_channels(&mut self, channel_ids: &[&str]) -> Result<Frame> {
        self.aggregator
            .aggregate_channels(&mut self.manager, channel_ids)
    }

    /// Get list of channel IDs
    pub fn channels(&self) -> Vec<String> {
        self.manager.list().cloned().collect()
    }

    /// Get number of channels
    pub fn channel_count(&self) -> usize {
        self.manager.count()
    }

    /// Check if a channel exists
    pub fn has_channel(&self, id: &str) -> bool {
        self.manager.contains(id)
    }

    /// Get pending value count for a channel
    ///
    /// # Errors
    ///
    /// Returns an error if the channel does not exist.
    pub fn pending(&self, channel_id: &str) -> Result<usize> {
        Ok(self.manager.get(channel_id)?.pending())
    }

    /// Get total pending values across all channels
    pub fn total_pending(&self) -> usize {
        self.manager.total_pending()
    }

    /// Clear all channel buffers without encoding
    pub fn clear_all(&mut self) {
        self.manager.clear_all_buffers();
    }

    /// Get a reference to the gateway configuration
    pub fn config(&self) -> &GatewayConfig {
        &self.config
    }

    /// Get the maximum frame size
    pub fn max_frame_size(&self) -> usize {
        self.config.max_frame_size
    }

    /// Update the maximum frame size
    pub fn set_max_frame_size(&mut self, size: usize) {
        self.config.max_frame_size = size;
        self.aggregator.set_max_frame_size(size);
    }

    /// Get a channel's context version
    ///
    /// # Errors
    ///
    /// Returns an error if the channel does not exist.
    pub fn channel_context_version(&self, channel_id: &str) -> Result<u32> {
        Ok(self.manager.get(channel_id)?.context_version())
    }

    /// Check if the gateway has any pending data
    pub fn has_pending_data(&self) -> bool {
        self.manager.total_pending() > 0
    }
}

impl Default for Gateway {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::GatewayError;

    #[test]
    fn test_gateway_new() {
        let gateway = Gateway::new();
        assert_eq!(gateway.channel_count(), 0);
        assert_eq!(gateway.total_pending(), 0);
    }

    #[test]
    fn test_gateway_with_config() {
        let config = GatewayConfig {
            max_frame_size: 100,
            max_channels: 5,
            enable_checksums: false,
        };
        let gateway = Gateway::with_config(config);
        assert_eq!(gateway.max_frame_size(), 100);
    }

    #[test]
    fn test_gateway_add_channel() {
        let mut gateway = Gateway::new();
        gateway
            .add_channel("temp", ChannelConfig::default())
            .unwrap();
        assert_eq!(gateway.channel_count(), 1);
        assert!(gateway.has_channel("temp"));
    }

    #[test]
    fn test_gateway_remove_channel() {
        let mut gateway = Gateway::new();
        gateway
            .add_channel("temp", ChannelConfig::default())
            .unwrap();
        gateway.remove_channel("temp").unwrap();
        assert_eq!(gateway.channel_count(), 0);
        assert!(!gateway.has_channel("temp"));
    }

    #[test]
    fn test_gateway_push() {
        let mut gateway = Gateway::new();
        gateway
            .add_channel("temp", ChannelConfig::default())
            .unwrap();
        gateway.push("temp", 22.5, 1000).unwrap();
        assert_eq!(gateway.pending("temp").unwrap(), 1);
        assert_eq!(gateway.total_pending(), 1);
    }

    #[test]
    fn test_gateway_push_multi() {
        let mut gateway = Gateway::new();
        gateway
            .add_channel("temp", ChannelConfig::default())
            .unwrap();
        gateway
            .push_multi("temp", &[(22.5, 1000), (22.6, 2000), (22.7, 3000)])
            .unwrap();
        assert_eq!(gateway.pending("temp").unwrap(), 3);
    }

    #[test]
    fn test_gateway_flush() {
        let mut gateway = Gateway::new();
        gateway
            .add_channel("temp", ChannelConfig::default())
            .unwrap();
        gateway.push("temp", 22.5, 1000).unwrap();

        let frame = gateway.flush().unwrap();
        assert_eq!(frame.channel_count(), 1);
        assert_eq!(gateway.pending("temp").unwrap(), 0);
    }

    #[test]
    fn test_gateway_flush_empty() {
        let mut gateway = Gateway::new();
        gateway
            .add_channel("temp", ChannelConfig::default())
            .unwrap();

        let frame = gateway.flush().unwrap();
        assert!(frame.is_empty());
    }

    #[test]
    fn test_gateway_channels() {
        let mut gateway = Gateway::new();
        gateway
            .add_channel("temp", ChannelConfig::default())
            .unwrap();
        gateway
            .add_channel("humid", ChannelConfig::default())
            .unwrap();

        let channels = gateway.channels();
        assert_eq!(channels.len(), 2);
        assert!(channels.contains(&"temp".to_string()));
        assert!(channels.contains(&"humid".to_string()));
    }

    #[test]
    fn test_gateway_clear_all() {
        let mut gateway = Gateway::new();
        gateway
            .add_channel("temp", ChannelConfig::default())
            .unwrap();
        gateway
            .add_channel("humid", ChannelConfig::default())
            .unwrap();
        gateway.push("temp", 22.5, 1000).unwrap();
        gateway.push("humid", 65.0, 1000).unwrap();

        assert_eq!(gateway.total_pending(), 2);
        gateway.clear_all();
        assert_eq!(gateway.total_pending(), 0);
    }

    #[test]
    fn test_gateway_channel_not_found() {
        let mut gateway = Gateway::new();
        let result = gateway.push("nonexistent", 22.5, 1000);
        assert!(matches!(result, Err(GatewayError::ChannelNotFound(_))));
    }

    #[test]
    fn test_gateway_has_pending_data() {
        let mut gateway = Gateway::new();
        gateway
            .add_channel("temp", ChannelConfig::default())
            .unwrap();

        assert!(!gateway.has_pending_data());
        gateway.push("temp", 22.5, 1000).unwrap();
        assert!(gateway.has_pending_data());
    }

    #[test]
    fn test_gateway_set_max_frame_size() {
        let mut gateway = Gateway::new();
        assert_eq!(gateway.max_frame_size(), 242);
        gateway.set_max_frame_size(100);
        assert_eq!(gateway.max_frame_size(), 100);
    }
}
