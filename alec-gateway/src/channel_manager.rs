// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Channel management for ALEC Gateway
//!
//! This module provides the [`ChannelManager`] and [`Channel`] types for
//! managing multiple sensor channels, each with its own ALEC encoder and context.

use std::collections::HashMap;
use std::path::Path;

use alec::{Classifier, Context, Encoder, RawData};

use crate::config::ChannelConfig;
use crate::error::{GatewayError, Result};

/// Unique identifier for a channel
pub type ChannelId = String;

/// A single sensor channel with its own encoder and context
pub struct Channel {
    /// Channel identifier
    pub id: ChannelId,
    /// Channel configuration
    pub config: ChannelConfig,
    /// ALEC encoder instance
    encoder: Encoder,
    /// ALEC classifier instance
    classifier: Classifier,
    /// Shared context for this channel
    context: Context,
    /// Buffer of pending values: (value, timestamp)
    buffer: Vec<(f64, u64)>,
}

impl Channel {
    /// Create a new channel with the given ID and configuration
    pub fn new(id: impl Into<String>, config: ChannelConfig) -> Result<Self> {
        let encoder = if config.enable_checksum {
            Encoder::with_checksum()
        } else {
            Encoder::new()
        };

        let context = if let Some(ref path) = config.preload_path {
            Context::load_from_file(Path::new(path)).map_err(|e| {
                GatewayError::InvalidConfig(format!("Failed to load preload '{}': {}", path, e))
            })?
        } else {
            Context::new()
        };

        Ok(Self {
            id: id.into(),
            config,
            encoder,
            classifier: Classifier::default(),
            context,
            buffer: Vec::new(),
        })
    }

    /// Push a value to the channel buffer
    pub fn push(&mut self, value: f64, timestamp: u64) -> Result<()> {
        if self.buffer.len() >= self.config.buffer_size {
            return Err(GatewayError::BufferFull(self.id.clone()));
        }
        self.buffer.push((value, timestamp));
        Ok(())
    }

    /// Encode all buffered values and clear buffer
    ///
    /// Returns the encoded bytes for all values in the buffer.
    pub fn flush(&mut self) -> Result<Vec<u8>> {
        if self.buffer.is_empty() {
            return Ok(Vec::new());
        }

        let mut encoded = Vec::new();

        for (value, timestamp) in self.buffer.drain(..) {
            let data = RawData::new(value, timestamp);
            let classification = self.classifier.classify(&data, &self.context);
            let bytes = self
                .encoder
                .encode_to_bytes(&data, &classification, &self.context);
            encoded.extend_from_slice(&bytes);

            // Update context after encoding
            self.context.observe(&data);
        }

        Ok(encoded)
    }

    /// Number of pending values in the buffer
    pub fn pending(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get the channel's context version
    pub fn context_version(&self) -> u32 {
        self.context.version()
    }

    /// Get a reference to the channel's context
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// Get a mutable reference to the channel's context
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    /// Reset the channel's encoder sequence
    pub fn reset_sequence(&mut self) {
        self.encoder.reset_sequence();
    }

    /// Clear the buffer without encoding
    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
    }
}

impl std::fmt::Debug for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channel")
            .field("id", &self.id)
            .field("config", &self.config)
            .field("pending", &self.buffer.len())
            .finish()
    }
}

/// Manages multiple channels
pub struct ChannelManager {
    /// Map of channel ID to channel
    channels: HashMap<ChannelId, Channel>,
    /// Maximum number of channels allowed
    max_channels: usize,
}

impl ChannelManager {
    /// Create a new channel manager with the specified maximum channels
    pub fn new(max_channels: usize) -> Self {
        Self {
            channels: HashMap::new(),
            max_channels,
        }
    }

    /// Add a new channel
    pub fn add(&mut self, id: impl Into<String>, config: ChannelConfig) -> Result<()> {
        let id = id.into();

        if self.channels.contains_key(&id) {
            return Err(GatewayError::ChannelAlreadyExists(id));
        }

        if self.channels.len() >= self.max_channels {
            return Err(GatewayError::MaxChannelsReached {
                max: self.max_channels,
            });
        }

        let channel = Channel::new(id.clone(), config)?;
        self.channels.insert(id, channel);
        Ok(())
    }

    /// Remove a channel
    pub fn remove(&mut self, id: &str) -> Result<Channel> {
        self.channels
            .remove(id)
            .ok_or_else(|| GatewayError::ChannelNotFound(id.to_string()))
    }

    /// Get a reference to a channel
    pub fn get(&self, id: &str) -> Result<&Channel> {
        self.channels
            .get(id)
            .ok_or_else(|| GatewayError::ChannelNotFound(id.to_string()))
    }

    /// Get a mutable reference to a channel
    pub fn get_mut(&mut self, id: &str) -> Result<&mut Channel> {
        self.channels
            .get_mut(id)
            .ok_or_else(|| GatewayError::ChannelNotFound(id.to_string()))
    }

    /// Check if a channel exists
    pub fn contains(&self, id: &str) -> bool {
        self.channels.contains_key(id)
    }

    /// Get an iterator over channel IDs
    pub fn list(&self) -> impl Iterator<Item = &ChannelId> {
        self.channels.keys()
    }

    /// Get an iterator over channels
    pub fn iter(&self) -> impl Iterator<Item = (&ChannelId, &Channel)> {
        self.channels.iter()
    }

    /// Get a mutable iterator over channels
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&ChannelId, &mut Channel)> {
        self.channels.iter_mut()
    }

    /// Get the number of channels
    pub fn count(&self) -> usize {
        self.channels.len()
    }

    /// Check if there are no channels
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    /// Get total pending values across all channels
    pub fn total_pending(&self) -> usize {
        self.channels.values().map(|c| c.pending()).sum()
    }

    /// Clear all channel buffers without encoding
    pub fn clear_all_buffers(&mut self) {
        for channel in self.channels.values_mut() {
            channel.clear_buffer();
        }
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new(32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_new() {
        let channel = Channel::new("test", ChannelConfig::default()).unwrap();
        assert_eq!(channel.id, "test");
        assert_eq!(channel.pending(), 0);
        assert!(channel.is_empty());
    }

    #[test]
    fn test_channel_push() {
        let mut channel = Channel::new("test", ChannelConfig::default()).unwrap();
        channel.push(22.5, 1000).unwrap();
        assert_eq!(channel.pending(), 1);
        assert!(!channel.is_empty());
    }

    #[test]
    fn test_channel_buffer_full() {
        let config = ChannelConfig::with_buffer_size(2);
        let mut channel = Channel::new("test", config).unwrap();
        channel.push(22.5, 1000).unwrap();
        channel.push(22.6, 2000).unwrap();
        let result = channel.push(22.7, 3000);
        assert!(matches!(result, Err(GatewayError::BufferFull(_))));
    }

    #[test]
    fn test_channel_flush() {
        let mut channel = Channel::new("test", ChannelConfig::default()).unwrap();
        channel.push(22.5, 1000).unwrap();
        channel.push(22.6, 2000).unwrap();

        let encoded = channel.flush().unwrap();
        assert!(!encoded.is_empty());
        assert_eq!(channel.pending(), 0);
    }

    #[test]
    fn test_channel_flush_empty() {
        let mut channel = Channel::new("test", ChannelConfig::default()).unwrap();
        let encoded = channel.flush().unwrap();
        assert!(encoded.is_empty());
    }

    #[test]
    fn test_channel_manager_add() {
        let mut manager = ChannelManager::new(10);
        manager.add("temp", ChannelConfig::default()).unwrap();
        assert_eq!(manager.count(), 1);
        assert!(manager.contains("temp"));
    }

    #[test]
    fn test_channel_manager_duplicate() {
        let mut manager = ChannelManager::new(10);
        manager.add("temp", ChannelConfig::default()).unwrap();
        let result = manager.add("temp", ChannelConfig::default());
        assert!(matches!(result, Err(GatewayError::ChannelAlreadyExists(_))));
    }

    #[test]
    fn test_channel_manager_max_channels() {
        let mut manager = ChannelManager::new(2);
        manager.add("ch1", ChannelConfig::default()).unwrap();
        manager.add("ch2", ChannelConfig::default()).unwrap();
        let result = manager.add("ch3", ChannelConfig::default());
        assert!(matches!(
            result,
            Err(GatewayError::MaxChannelsReached { .. })
        ));
    }

    #[test]
    fn test_channel_manager_remove() {
        let mut manager = ChannelManager::new(10);
        manager.add("temp", ChannelConfig::default()).unwrap();
        let channel = manager.remove("temp").unwrap();
        assert_eq!(channel.id, "temp");
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_channel_manager_not_found() {
        let manager = ChannelManager::new(10);
        let result = manager.get("nonexistent");
        assert!(matches!(result, Err(GatewayError::ChannelNotFound(_))));
    }
}
