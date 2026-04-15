// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Protocol definitions for ALEC
//!
//! This module defines the core types used in the ALEC protocol:
//! - Message structure and headers
//! - Priority levels
//! - Encoding types
//! - Raw data representation

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::error::DecodeError;
use core::fmt;

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

/// Input for one channel in a multi-channel encode_multi_adaptive() call
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelInput {
    /// Channel identifier (included in the wire frame as 1 byte)
    pub name_id: u8,
    /// Source identifier for per-channel context isolation
    pub source_id: u32,
    /// The measured value
    pub value: f64,
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
    /// Keep-alive heartbeat (repurposed by the fixed-channel path as
    /// a keyframe signal on the wire via marker byte 0xA2).
    Heartbeat = 6,
    /// Fixed-channel data frame used by the Milesight integration.
    /// The wire format for this variant does NOT use `MessageHeader` —
    /// it uses `CompactHeader` (4 bytes) prefixed by a marker byte
    /// (0xA1 data / 0xA2 keyframe). This enum value only exists so
    /// higher-level abstractions can tag frames by type internally.
    DataFixedChannel = 7,
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
            7 => Some(MessageType::DataFixedChannel),
            _ => None,
        }
    }
}

// ============================================================================
// Compact fixed-channel header (Milesight EM500-CO2 integration)
//
// Wire format:
//
//     byte 0       : marker        (0xA1 data, 0xA2 keyframe)
//     byte 1..=2   : sequence      (u16 big-endian)
//     byte 3..=4   : ctx_version   (u16 big-endian, low bits of ctx u32)
//     byte 5..     : payload (bitmap + channel data — see encoder)
//
// Two dedicated markers are used (instead of stealing bit 15 of
// ctx_version as a keyframe flag) so the full u16 range of the
// context version is available on the wire, and so a JS passthrough
// codec can identify any ALEC fixed-channel frame via `b & 0xFE == 0xA0`.
// ============================================================================

/// Marker byte for a regular fixed-channel data frame.
pub const COMPACT_MARKER_DATA: u8 = 0xA1;

/// Marker byte for a fixed-channel keyframe frame (all channels
/// encoded as Raw32 so the decoder can resync unconditionally).
pub const COMPACT_MARKER_KEYFRAME: u8 = 0xA2;

/// Whether a byte is a fixed-channel ALEC marker.
///
/// Returns `Some(true)` for a keyframe marker, `Some(false)` for a
/// regular data marker, `None` for any other byte.
#[inline]
pub fn classify_compact_marker(byte: u8) -> Option<bool> {
    match byte {
        COMPACT_MARKER_DATA => Some(false),
        COMPACT_MARKER_KEYFRAME => Some(true),
        _ => None,
    }
}

/// 4-byte fixed-channel header — sequence + truncated context version.
///
/// The `context_version` field carries the low 16 bits of the encoder's
/// internal `u32` version. Wraparound is handled with
/// [`ctx_version_compatible`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactHeader {
    /// Frame sequence number (u16 big-endian on the wire).
    pub sequence: u16,
    /// Context version (low 16 bits of the encoder's u32 version),
    /// big-endian on the wire.
    pub context_version: u16,
}

impl CompactHeader {
    /// Size of the compact header on the wire (excludes the marker byte).
    pub const SIZE: usize = 4;

    /// Create a new compact header.
    pub fn new(sequence: u16, context_version: u16) -> Self {
        Self {
            sequence,
            context_version,
        }
    }

    /// Serialize the 4-byte header into `buf[..4]`.
    ///
    /// Returns the number of bytes written (always 4) or
    /// `DecodeError::BufferTooShort` if the buffer is too small.
    pub fn write(&self, buf: &mut [u8]) -> Result<usize, DecodeError> {
        if buf.len() < Self::SIZE {
            return Err(DecodeError::BufferTooShort {
                needed: Self::SIZE,
                available: buf.len(),
            });
        }
        buf[0..2].copy_from_slice(&self.sequence.to_be_bytes());
        buf[2..4].copy_from_slice(&self.context_version.to_be_bytes());
        Ok(Self::SIZE)
    }

    /// Parse a 4-byte header from `buf[..4]`.
    pub fn read(buf: &[u8]) -> Result<Self, DecodeError> {
        if buf.len() < Self::SIZE {
            return Err(DecodeError::BufferTooShort {
                needed: Self::SIZE,
                available: buf.len(),
            });
        }
        let sequence = u16::from_be_bytes([buf[0], buf[1]]);
        let context_version = u16::from_be_bytes([buf[2], buf[3]]);
        Ok(Self {
            sequence,
            context_version,
        })
    }
}

/// Check whether an incoming u16 `context_version` is a "compatible"
/// successor of the locally tracked `last` version.
///
/// The version is a ring in `u16` space. Any forward jump up to
/// `max_forward_jump` — including one that wraps past 65535 — is
/// accepted as compatible. A backward jump (i.e. a decrease larger
/// than `u16::MAX - max_forward_jump`) is rejected as a mismatch.
///
/// The Milesight integration increments `context_version` once per
/// observation, so in steady state successive frames have either
/// equal versions (no new observation yet) or differ by at most a
/// small amount. Callers pick `max_forward_jump` to bound the
/// tolerated gap before flagging a sync error.
///
/// Returns `true` if compatible, `false` if the difference looks like
/// a backwards jump or a forward jump larger than `max_forward_jump`.
///
/// # Example
///
/// ```
/// use alec::protocol::ctx_version_compatible;
///
/// // Normal forward increment:
/// assert!(ctx_version_compatible(11, 10, 32));
/// // Wraparound is fine:
/// assert!(ctx_version_compatible(0, 65535, 32));
/// assert!(ctx_version_compatible(5, 65530, 32));
/// // Large backward jump: not compatible.
/// assert!(!ctx_version_compatible(10, 100, 32));
/// // Forward jump larger than the tolerated threshold: not compatible.
/// assert!(!ctx_version_compatible(1000, 10, 32));
/// ```
#[inline]
pub fn ctx_version_compatible(incoming: u16, last: u16, max_forward_jump: u16) -> bool {
    // u16 wrapping subtraction gives the forward distance (0..=65535).
    // Any value <= max_forward_jump is treated as a forward step.
    let forward = incoming.wrapping_sub(last);
    forward <= max_forward_jump
}

/// Encoding types for data compression
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
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

/// Message header (10 bytes total)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageHeader {
    /// Protocol version (2 bits in header byte)
    pub version: u8,
    /// Message type (3 bits in header byte)
    pub message_type: MessageType,
    /// Priority level (3 bits in header byte)
    pub priority: Priority,
    /// Sequence number (u16, wraps every 65 536 frames)
    pub sequence: u16,
    /// Timestamp (Unix seconds)
    pub timestamp: u32,
    /// Context version used for encoding (serialized as u24, max 16 777 215)
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
    pub const SIZE: usize = 10;

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
        bytes[1..3].copy_from_slice(&self.sequence.to_be_bytes());
        bytes[3..7].copy_from_slice(&self.timestamp.to_be_bytes());
        let cv = self.context_version & 0x00FFFFFF;
        bytes[7..10].copy_from_slice(&cv.to_be_bytes()[1..]);
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

        let sequence = u16::from_be_bytes([bytes[1], bytes[2]]);
        let timestamp = u32::from_be_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]);
        let context_version = u32::from_be_bytes([0, bytes[7], bytes[8], bytes[9]]);

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

    /// Get the encoding type from the payload (first byte after source_id varint)
    pub fn encoding_type(&self) -> Option<EncodingType> {
        // Payload format: source_id (varint) + encoding_type (1 byte) + value
        // Decode the varint to find where the encoding byte starts.
        let mut pos = 0;
        while pos < self.payload.len() {
            let byte = self.payload[pos];
            pos += 1;
            if byte & 0x80 == 0 {
                // End of varint — next byte is the encoding type
                return self
                    .payload
                    .get(pos)
                    .and_then(|&b| EncodingType::from_u8(b));
            }
        }
        None
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

    // ========================================================================
    // Bloc B1: Compact fixed-channel header
    // ========================================================================

    #[test]
    fn test_compact_header_roundtrip() {
        let h = CompactHeader::new(12345, 6789);
        let mut buf = [0u8; CompactHeader::SIZE];
        assert_eq!(h.write(&mut buf).unwrap(), CompactHeader::SIZE);
        // Big-endian layout check.
        assert_eq!(buf, [0x30, 0x39, 0x1A, 0x85]);
        let back = CompactHeader::read(&buf).unwrap();
        assert_eq!(back, h);
    }

    #[test]
    fn test_compact_header_buffer_too_short() {
        let h = CompactHeader::new(1, 2);
        let mut buf = [0u8; 3];
        assert!(matches!(
            h.write(&mut buf),
            Err(DecodeError::BufferTooShort { .. })
        ));
        assert!(matches!(
            CompactHeader::read(&buf),
            Err(DecodeError::BufferTooShort { .. })
        ));
    }

    #[test]
    fn test_classify_compact_marker() {
        assert_eq!(classify_compact_marker(0xA1), Some(false));
        assert_eq!(classify_compact_marker(0xA2), Some(true));
        assert_eq!(classify_compact_marker(0x00), None);
        assert_eq!(classify_compact_marker(0xFF), None);
        // Legacy TLV first byte can never collide:
        // version 1 | DataFixedChannel=7 | P3=2 → 0b01_111_010 = 0x7A.
        assert_eq!(classify_compact_marker(0x7A), None);
    }

    #[test]
    fn test_message_type_data_fixed_channel() {
        // Variant is placed at the Reserved=7 slot.
        assert_eq!(MessageType::DataFixedChannel as u8, 7);
        assert_eq!(MessageType::from_u8(7), Some(MessageType::DataFixedChannel));
    }

    #[test]
    fn test_ctx_version_compatible_forward_and_wraparound() {
        // Same version (no new observation yet) is compatible.
        assert!(ctx_version_compatible(100, 100, 32));
        // Normal forward increments.
        assert!(ctx_version_compatible(101, 100, 32));
        assert!(ctx_version_compatible(131, 100, 32));
        // Right at the threshold.
        assert!(ctx_version_compatible(132, 100, 32));
        // Past the threshold.
        assert!(!ctx_version_compatible(133, 100, 32));

        // Wraparound across 65535 → 0.
        assert!(ctx_version_compatible(0, 65535, 32));
        // forward distance from 65530 to 5 = (5 - 65530) wrapping = 11, < 32 OK.
        assert!(ctx_version_compatible(5, 65530, 32));
        // forward distance from 65530 to 26 = 32, exactly at threshold.
        assert!(ctx_version_compatible(26, 65530, 32));
        // forward distance from 65530 to 31 = 37, past threshold.
        assert!(!ctx_version_compatible(31, 65530, 32));
        // Wraparound past the threshold.
        assert!(!ctx_version_compatible(1000, 65530, 32));

        // Backward jump (ring direction counts as very large forward).
        assert!(!ctx_version_compatible(10, 100, 32));
    }
}
