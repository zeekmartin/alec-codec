//! Protocol definitions for ALEC
//!
//! This module defines the core types used in the ALEC protocol:
//! - Message structure and headers
//! - Priority levels
//! - Encoding types
//! - Raw data representation

use crate::error::DecodeError;
use std::fmt;

/// Checksum size in bytes (xxHash32)
pub const CHECKSUM_SIZE: usize = 4;

/// Raw data from a sensor or source
#[derive(Debug, Clone, PartialEq)]
pub struct RawData {
    /// Unique identifier for the data source
    pub source_id: u32,
    /// Timestamp (relative or absolute)
    pub timestamp: u64,
    /// The measured value
    pub value: f64,
}

impl RawData {
    /// Create new raw data with default source_id of 0
    pub fn new(value: f64, timestamp: u64) -> Self {
        Self {
            source_id: 0,
            timestamp,
            value,
        }
    }

    /// Create new raw data with a specific source_id
    pub fn with_source(source_id: u32, value: f64, timestamp: u64) -> Self {
        Self {
            source_id,
            timestamp,
            value,
        }
    }

    /// Size of raw data in bytes (for comparison)
    pub fn raw_size(&self) -> usize {
        // source_id (4) + timestamp (8) + value (8) = 20 bytes
        20
    }
}

/// Priority levels for data classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum Priority {
    /// Critical - immediate transmission with acknowledgment required
    P1Critical = 0,
    /// Important - immediate transmission
    P2Important = 1,
    /// Normal - standard transmission
    #[default]
    P3Normal = 2,
    /// Deferred - stored locally, sent on request
    P4Deferred = 3,
    /// Disposable - never sent spontaneously
    P5Disposable = 4,
}

impl Priority {
    /// Check if this priority level should be transmitted immediately
    pub fn should_transmit(&self) -> bool {
        matches!(
            self,
            Priority::P1Critical | Priority::P2Important | Priority::P3Normal
        )
    }

    /// Check if this priority requires acknowledgment
    pub fn requires_ack(&self) -> bool {
        matches!(self, Priority::P1Critical)
    }

    /// Convert from u8
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Priority::P1Critical),
            1 => Some(Priority::P2Important),
            2 => Some(Priority::P3Normal),
            3 => Some(Priority::P4Deferred),
            4 => Some(Priority::P5Disposable),
            _ => None,
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Priority::P1Critical => write!(f, "P1-CRITICAL"),
            Priority::P2Important => write!(f, "P2-IMPORTANT"),
            Priority::P3Normal => write!(f, "P3-NORMAL"),
            Priority::P4Deferred => write!(f, "P4-DEFERRED"),
            Priority::P5Disposable => write!(f, "P5-DISPOSABLE"),
        }
    }
}

/// Message types in the ALEC protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum MessageType {
    /// Encoded data payload
    #[default]
    Data = 0,
    /// Context synchronization
    Sync = 1,
    /// Request from receiver
    Request = 2,
    /// Response to request
    Response = 3,
    /// Acknowledgment
    Ack = 4,
    /// Negative acknowledgment
    Nack = 5,
    /// Keep-alive heartbeat
    Heartbeat = 6,
    /// Reserved for future use
    Reserved = 7,
}

impl MessageType {
    /// Convert from u8
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(MessageType::Data),
            1 => Some(MessageType::Sync),
            2 => Some(MessageType::Request),
            3 => Some(MessageType::Response),
            4 => Some(MessageType::Ack),
            5 => Some(MessageType::Nack),
            6 => Some(MessageType::Heartbeat),
            7 => Some(MessageType::Reserved),
            _ => None,
        }
    }
}

/// Encoding types for data compression
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum EncodingType {
    /// Raw float64, big-endian (8 bytes)
    #[default]
    Raw64 = 0x00,
    /// Raw float32, big-endian (4 bytes)
    Raw32 = 0x01,
    /// Delta encoded as i8 (1 byte)
    Delta8 = 0x10,
    /// Delta encoded as i16 big-endian (2 bytes)
    Delta16 = 0x11,
    /// Delta encoded as i32 big-endian (4 bytes)
    Delta32 = 0x12,
    /// Reference to dictionary pattern
    Pattern = 0x20,
    /// Pattern reference with delta8 adjustment
    PatternDelta = 0x21,
    /// Same value as previous (0 bytes)
    Repeated = 0x30,
    /// Exact predicted value (0 bytes)
    Interpolated = 0x31,
    /// Multiple values in one message
    Multi = 0x40,
}

impl EncodingType {
    /// Convert from u8
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(EncodingType::Raw64),
            0x01 => Some(EncodingType::Raw32),
            0x10 => Some(EncodingType::Delta8),
            0x11 => Some(EncodingType::Delta16),
            0x12 => Some(EncodingType::Delta32),
            0x20 => Some(EncodingType::Pattern),
            0x21 => Some(EncodingType::PatternDelta),
            0x30 => Some(EncodingType::Repeated),
            0x31 => Some(EncodingType::Interpolated),
            0x40 => Some(EncodingType::Multi),
            _ => None,
        }
    }

    /// Get the typical size of this encoding (excluding header)
    pub fn typical_size(&self) -> usize {
        match self {
            EncodingType::Raw64 => 8,
            EncodingType::Raw32 => 4,
            EncodingType::Delta8 => 1,
            EncodingType::Delta16 => 2,
            EncodingType::Delta32 => 4,
            EncodingType::Pattern => 2,      // varint typically 1-2 bytes
            EncodingType::PatternDelta => 3, // varint + 1 byte
            EncodingType::Repeated => 0,
            EncodingType::Interpolated => 0,
            EncodingType::Multi => 0, // variable
        }
    }
}

/// Message header (13 bytes total)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageHeader {
    /// Protocol version (2 bits in header byte)
    pub version: u8,
    /// Message type (3 bits in header byte)
    pub message_type: MessageType,
    /// Priority level (3 bits in header byte)
    pub priority: Priority,
    /// Sequence number
    pub sequence: u32,
    /// Timestamp
    pub timestamp: u32,
    /// Context version used for encoding
    pub context_version: u32,
}

impl MessageHeader {
    /// Create a new header with default values
    pub fn new(message_type: MessageType, priority: Priority) -> Self {
        Self {
            version: crate::PROTOCOL_VERSION,
            message_type,
            priority,
            sequence: 0,
            timestamp: 0,
            context_version: 0,
        }
    }

    /// Header size in bytes
    pub const SIZE: usize = 13;

    /// Encode the header byte (version + type + priority)
    pub fn encode_header_byte(&self) -> u8 {
        let version_bits = (self.version & 0x03) << 6;
        let type_bits = (self.message_type as u8 & 0x07) << 3;
        let priority_bits = self.priority as u8 & 0x07;
        version_bits | type_bits | priority_bits
    }

    /// Decode the header byte
    pub fn decode_header_byte(byte: u8) -> (u8, Option<MessageType>, Option<Priority>) {
        let version = (byte >> 6) & 0x03;
        let msg_type = MessageType::from_u8((byte >> 3) & 0x07);
        let priority = Priority::from_u8(byte & 0x07);
        (version, msg_type, priority)
    }

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0] = self.encode_header_byte();
        bytes[1..5].copy_from_slice(&self.sequence.to_be_bytes());
        bytes[5..9].copy_from_slice(&self.timestamp.to_be_bytes());
        bytes[9..13].copy_from_slice(&self.context_version.to_be_bytes());
        bytes
    }

    /// Deserialize header from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }

        let (version, msg_type, priority) = Self::decode_header_byte(bytes[0]);
        let msg_type = msg_type?;
        let priority = priority?;

        let sequence = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
        let timestamp = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
        let context_version = u32::from_be_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]);

        Some(Self {
            version,
            message_type: msg_type,
            priority,
            sequence,
            timestamp,
            context_version,
        })
    }
}

impl Default for MessageHeader {
    fn default() -> Self {
        Self::new(MessageType::Data, Priority::P3Normal)
    }
}

/// An encoded message ready for transmission
#[derive(Debug, Clone, PartialEq)]
pub struct EncodedMessage {
    /// Message header
    pub header: MessageHeader,
    /// Encoded payload
    pub payload: Vec<u8>,
}

impl EncodedMessage {
    /// Create a new encoded message
    pub fn new(header: MessageHeader, payload: Vec<u8>) -> Self {
        Self { header, payload }
    }

    /// Total size of the message in bytes
    pub fn len(&self) -> usize {
        MessageHeader::SIZE + self.payload.len()
    }

    /// Check if the message is empty (no payload)
    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }

    /// Get the encoding type from the payload (first byte after source_id)
    pub fn encoding_type(&self) -> Option<EncodingType> {
        // Payload format: source_id (varint) + encoding_type (1 byte) + value
        // For simplicity, assuming source_id is 1 byte (< 128)
        if self.payload.len() >= 2 {
            EncodingType::from_u8(self.payload[1])
        } else {
            None
        }
    }

    /// Serialize the entire message to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.len());
        bytes.extend_from_slice(&self.header.to_bytes());
        bytes.extend_from_slice(&self.payload);
        bytes
    }

    /// Deserialize message from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < MessageHeader::SIZE {
            return None;
        }

        let header = MessageHeader::from_bytes(&bytes[..MessageHeader::SIZE])?;
        let payload = bytes[MessageHeader::SIZE..].to_vec();

        Some(Self { header, payload })
    }

    /// Compute checksum of the message (header + payload)
    pub fn compute_checksum(&self) -> u32 {
        use xxhash_rust::xxh32::xxh32;

        let mut data = Vec::with_capacity(MessageHeader::SIZE + self.payload.len());
        data.extend_from_slice(&self.header.to_bytes());
        data.extend_from_slice(&self.payload);

        xxh32(&data, 0) // seed = 0
    }

    /// Serialize message with checksum appended
    pub fn to_bytes_with_checksum(&self) -> Vec<u8> {
        let mut bytes = self.to_bytes();
        let checksum = self.compute_checksum();
        bytes.extend_from_slice(&checksum.to_be_bytes());
        bytes
    }

    /// Deserialize message from bytes with checksum verification
    pub fn from_bytes_with_checksum(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < MessageHeader::SIZE + CHECKSUM_SIZE {
            return Err(DecodeError::BufferTooShort {
                needed: MessageHeader::SIZE + CHECKSUM_SIZE,
                available: bytes.len(),
            });
        }

        let checksum_offset = bytes.len() - CHECKSUM_SIZE;
        let expected = u32::from_be_bytes(bytes[checksum_offset..].try_into().unwrap());

        let message =
            Self::from_bytes(&bytes[..checksum_offset]).ok_or(DecodeError::InvalidHeader)?;

        let actual = message.compute_checksum();

        if actual != expected {
            return Err(DecodeError::InvalidChecksum { expected, actual });
        }

        Ok(message)
    }
}

/// Decoded data result
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedData {
    /// Source identifier
    pub source_id: u32,
    /// Timestamp from header
    pub timestamp: u64,
    /// Decoded value
    pub value: f64,
    /// Original priority
    pub priority: Priority,
    /// Whether deferred data is available
    pub deferred_available: bool,
}

impl DecodedData {
    /// Create new decoded data
    pub fn new(source_id: u32, timestamp: u64, value: f64, priority: Priority) -> Self {
        Self {
            source_id,
            timestamp,
            value,
            priority,
            deferred_available: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::P1Critical < Priority::P2Important);
        assert!(Priority::P2Important < Priority::P3Normal);
        assert!(Priority::P3Normal < Priority::P4Deferred);
        assert!(Priority::P4Deferred < Priority::P5Disposable);
    }

    #[test]
    fn test_priority_should_transmit() {
        assert!(Priority::P1Critical.should_transmit());
        assert!(Priority::P2Important.should_transmit());
        assert!(Priority::P3Normal.should_transmit());
        assert!(!Priority::P4Deferred.should_transmit());
        assert!(!Priority::P5Disposable.should_transmit());
    }

    #[test]
    fn test_header_byte_roundtrip() {
        let header = MessageHeader {
            version: 1,
            message_type: MessageType::Data,
            priority: Priority::P2Important,
            sequence: 0,
            timestamp: 0,
            context_version: 0,
        };

        let byte = header.encode_header_byte();
        let (version, msg_type, priority) = MessageHeader::decode_header_byte(byte);

        assert_eq!(version, 1);
        assert_eq!(msg_type, Some(MessageType::Data));
        assert_eq!(priority, Some(Priority::P2Important));
    }

    #[test]
    fn test_header_serialization() {
        let header = MessageHeader {
            version: 1,
            message_type: MessageType::Sync,
            priority: Priority::P1Critical,
            sequence: 12345,
            timestamp: 67890,
            context_version: 42,
        };

        let bytes = header.to_bytes();
        let restored = MessageHeader::from_bytes(&bytes).unwrap();

        assert_eq!(header.version, restored.version);
        assert_eq!(header.message_type, restored.message_type);
        assert_eq!(header.priority, restored.priority);
        assert_eq!(header.sequence, restored.sequence);
        assert_eq!(header.timestamp, restored.timestamp);
        assert_eq!(header.context_version, restored.context_version);
    }

    #[test]
    fn test_message_serialization() {
        let message = EncodedMessage {
            header: MessageHeader::default(),
            payload: vec![0x00, 0x10, 0x42],
        };

        let bytes = message.to_bytes();
        let restored = EncodedMessage::from_bytes(&bytes).unwrap();

        assert_eq!(message.header.message_type, restored.header.message_type);
        assert_eq!(message.payload, restored.payload);
    }

    #[test]
    fn test_raw_data() {
        let data = RawData::new(42.5, 12345);
        assert_eq!(data.source_id, 0);
        assert_eq!(data.value, 42.5);
        assert_eq!(data.timestamp, 12345);
        assert_eq!(data.raw_size(), 20);
    }

    #[test]
    fn test_checksum_computation() {
        let message = EncodedMessage {
            header: MessageHeader::default(),
            payload: vec![0x00, 0x10, 0x42],
        };

        let checksum1 = message.compute_checksum();
        let checksum2 = message.compute_checksum();

        // Same message should produce same checksum
        assert_eq!(checksum1, checksum2);

        // Different message should produce different checksum
        let message2 = EncodedMessage {
            header: MessageHeader::default(),
            payload: vec![0x00, 0x10, 0x43],
        };
        let checksum3 = message2.compute_checksum();
        assert_ne!(checksum1, checksum3);
    }

    #[test]
    fn test_checksum_roundtrip() {
        let message = EncodedMessage {
            header: MessageHeader {
                version: 1,
                message_type: MessageType::Data,
                priority: Priority::P2Important,
                sequence: 42,
                timestamp: 12345,
                context_version: 7,
            },
            payload: vec![0x00, 0x10, 0x42, 0x55, 0xAA],
        };

        let bytes = message.to_bytes_with_checksum();
        let restored = EncodedMessage::from_bytes_with_checksum(&bytes).unwrap();

        assert_eq!(message.header.sequence, restored.header.sequence);
        assert_eq!(message.header.timestamp, restored.header.timestamp);
        assert_eq!(message.payload, restored.payload);
    }

    #[test]
    fn test_checksum_corruption_detected() {
        let message = EncodedMessage {
            header: MessageHeader::default(),
            payload: vec![0x00, 0x10, 0x42],
        };

        let mut bytes = message.to_bytes_with_checksum();

        // Corrupt a byte in the payload
        bytes[MessageHeader::SIZE] ^= 0xFF;

        let result = EncodedMessage::from_bytes_with_checksum(&bytes);
        assert!(matches!(result, Err(DecodeError::InvalidChecksum { .. })));
    }

    #[test]
    fn test_checksum_buffer_too_short() {
        let short_bytes = vec![0u8; MessageHeader::SIZE]; // No checksum

        let result = EncodedMessage::from_bytes_with_checksum(&short_bytes);
        assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
    }
}
