// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Frame format for aggregated channel data
//!
//! This module defines the [`Frame`] type for combining data from multiple
//! channels into a single transmission unit.
//!
//! # Frame Format
//!
//! ```text
//! [version: 1] [channel_count: 1] [channel_data...]
//!
//! channel_data:
//! [id_len: 1] [id: N] [data_len: 2 LE] [data: M]
//! ```

/// Aggregated frame containing data from multiple channels
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Frame format version
    pub version: u8,
    /// Channel data entries
    pub channels: Vec<ChannelData>,
}

/// Data from a single channel within a frame
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelData {
    /// Channel identifier
    pub id: String,
    /// Encoded data bytes
    pub data: Vec<u8>,
}

impl Frame {
    /// Current frame format version
    pub const VERSION: u8 = 1;

    /// Create a new empty frame
    pub fn new() -> Self {
        Self {
            version: Self::VERSION,
            channels: Vec::new(),
        }
    }

    /// Add channel data to the frame
    pub fn add_channel(&mut self, id: String, data: Vec<u8>) {
        self.channels.push(ChannelData { id, data });
    }

    /// Check if the frame is empty
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    /// Get the number of channels in the frame
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Serialize the frame to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Version
        buf.push(self.version);

        // Channel count
        buf.push(self.channels.len() as u8);

        // Channel data
        for ch in &self.channels {
            // ID length
            buf.push(ch.id.len() as u8);
            // ID
            buf.extend_from_slice(ch.id.as_bytes());
            // Data length (little-endian u16)
            buf.extend_from_slice(&(ch.data.len() as u16).to_le_bytes());
            // Data
            buf.extend_from_slice(&ch.data);
        }

        buf
    }

    /// Parse a frame from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameParseError> {
        if data.len() < 2 {
            return Err(FrameParseError::TooShort);
        }

        let version = data[0];
        if version != Self::VERSION {
            return Err(FrameParseError::UnsupportedVersion(version));
        }

        let channel_count = data[1] as usize;
        let mut pos = 2;
        let mut channels = Vec::with_capacity(channel_count);

        for _ in 0..channel_count {
            if pos >= data.len() {
                return Err(FrameParseError::Truncated);
            }

            // ID length
            let id_len = data[pos] as usize;
            pos += 1;

            // ID
            if pos + id_len > data.len() {
                return Err(FrameParseError::TruncatedChannelId);
            }
            let id = String::from_utf8_lossy(&data[pos..pos + id_len]).to_string();
            pos += id_len;

            // Data length
            if pos + 2 > data.len() {
                return Err(FrameParseError::TruncatedDataLength);
            }
            let data_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
            pos += 2;

            // Data
            if pos + data_len > data.len() {
                return Err(FrameParseError::TruncatedChannelData);
            }
            let channel_data = data[pos..pos + data_len].to_vec();
            pos += data_len;

            channels.push(ChannelData {
                id,
                data: channel_data,
            });
        }

        Ok(Self { version, channels })
    }

    /// Calculate the total size of the frame in bytes
    pub fn size(&self) -> usize {
        let mut size = 2; // version + channel_count
        for ch in &self.channels {
            size += 1; // id_len
            size += ch.id.len(); // id
            size += 2; // data_len
            size += ch.data.len(); // data
        }
        size
    }

    /// Get channel data by ID
    pub fn get_channel(&self, id: &str) -> Option<&ChannelData> {
        self.channels.iter().find(|ch| ch.id == id)
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur when parsing a frame
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameParseError {
    /// Frame data is too short
    TooShort,
    /// Unsupported frame version
    UnsupportedVersion(u8),
    /// Frame data is truncated
    Truncated,
    /// Channel ID is truncated
    TruncatedChannelId,
    /// Data length field is truncated
    TruncatedDataLength,
    /// Channel data is truncated
    TruncatedChannelData,
}

impl std::fmt::Display for FrameParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "Frame too short"),
            Self::UnsupportedVersion(v) => write!(f, "Unsupported frame version: {}", v),
            Self::Truncated => write!(f, "Frame truncated"),
            Self::TruncatedChannelId => write!(f, "Truncated channel ID"),
            Self::TruncatedDataLength => write!(f, "Truncated data length"),
            Self::TruncatedChannelData => write!(f, "Truncated channel data"),
        }
    }
}

impl std::error::Error for FrameParseError {}

/// Builder for constructing frames with size limits
pub struct FrameBuilder {
    frame: Frame,
    max_size: usize,
}

impl FrameBuilder {
    /// Create a new frame builder with the specified maximum size
    pub fn new(max_size: usize) -> Self {
        Self {
            frame: Frame::new(),
            max_size,
        }
    }

    /// Try to add channel data, returns false if frame would exceed max size
    pub fn try_add(&mut self, id: String, data: Vec<u8>) -> bool {
        // Calculate additional size needed
        let additional_size = 1 + id.len() + 2 + data.len();

        if self.frame.size() + additional_size > self.max_size {
            return false;
        }

        self.frame.add_channel(id, data);
        true
    }

    /// Get the remaining space in bytes
    pub fn remaining(&self) -> usize {
        self.max_size.saturating_sub(self.frame.size())
    }

    /// Get the current frame size
    pub fn current_size(&self) -> usize {
        self.frame.size()
    }

    /// Check if the frame is empty
    pub fn is_empty(&self) -> bool {
        self.frame.is_empty()
    }

    /// Build the frame
    pub fn build(self) -> Frame {
        self.frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_new() {
        let frame = Frame::new();
        assert_eq!(frame.version, Frame::VERSION);
        assert!(frame.is_empty());
        assert_eq!(frame.channel_count(), 0);
    }

    #[test]
    fn test_frame_add_channel() {
        let mut frame = Frame::new();
        frame.add_channel("temp".to_string(), vec![1, 2, 3]);
        assert_eq!(frame.channel_count(), 1);
        assert!(!frame.is_empty());
    }

    #[test]
    fn test_frame_roundtrip() {
        let mut frame = Frame::new();
        frame.add_channel("temp".to_string(), vec![1, 2, 3]);
        frame.add_channel("humid".to_string(), vec![4, 5, 6, 7]);

        let bytes = frame.to_bytes();
        let restored = Frame::from_bytes(&bytes).unwrap();

        assert_eq!(frame, restored);
    }

    #[test]
    fn test_frame_size() {
        let mut frame = Frame::new();
        assert_eq!(frame.size(), 2); // version + count

        frame.add_channel("t".to_string(), vec![1, 2]);
        // 2 (header) + 1 (id_len) + 1 (id) + 2 (data_len) + 2 (data) = 8
        assert_eq!(frame.size(), 8);
    }

    #[test]
    fn test_frame_get_channel() {
        let mut frame = Frame::new();
        frame.add_channel("temp".to_string(), vec![1, 2, 3]);
        frame.add_channel("humid".to_string(), vec![4, 5, 6]);

        let ch = frame.get_channel("temp").unwrap();
        assert_eq!(ch.data, vec![1, 2, 3]);

        assert!(frame.get_channel("nonexistent").is_none());
    }

    #[test]
    fn test_frame_parse_too_short() {
        let result = Frame::from_bytes(&[1]);
        assert!(matches!(result, Err(FrameParseError::TooShort)));
    }

    #[test]
    fn test_frame_parse_unsupported_version() {
        let result = Frame::from_bytes(&[255, 0]);
        assert!(matches!(
            result,
            Err(FrameParseError::UnsupportedVersion(255))
        ));
    }

    #[test]
    fn test_frame_parse_truncated() {
        let result = Frame::from_bytes(&[1, 1]); // Says 1 channel but no data
        assert!(matches!(result, Err(FrameParseError::Truncated)));
    }

    #[test]
    fn test_frame_builder() {
        let mut builder = FrameBuilder::new(100);
        assert!(builder.try_add("temp".to_string(), vec![1, 2, 3]));
        // 2 (header) + 1 (id_len) + 4 (id "temp") + 2 (data_len) + 3 (data) = 12
        assert_eq!(builder.current_size(), 12);

        let frame = builder.build();
        assert_eq!(frame.channel_count(), 1);
    }

    #[test]
    fn test_frame_builder_max_size() {
        let mut builder = FrameBuilder::new(10);
        // First channel fits (2 + 1 + 1 + 2 + 2 = 8)
        assert!(builder.try_add("a".to_string(), vec![1, 2]));
        // Second channel doesn't fit
        assert!(!builder.try_add("b".to_string(), vec![3, 4]));
    }

    #[test]
    fn test_frame_builder_remaining() {
        let mut builder = FrameBuilder::new(100);
        let initial_remaining = builder.remaining();
        builder.try_add("t".to_string(), vec![1, 2, 3]);
        assert!(builder.remaining() < initial_remaining);
    }
}
