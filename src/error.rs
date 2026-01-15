//! Error types for ALEC
//!
//! This module defines all error types used throughout the library.

use thiserror::Error;

/// Result type alias for ALEC operations
pub type Result<T> = std::result::Result<T, AlecError>;

/// Main error type for ALEC operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum AlecError {
    /// Encoding error
    #[error("Encoding error: {0}")]
    Encode(#[from] EncodeError),

    /// Decoding error
    #[error("Decoding error: {0}")]
    Decode(#[from] DecodeError),

    /// Context error
    #[error("Context error: {0}")]
    Context(#[from] ContextError),

    /// Channel error
    #[error("Channel error: {0}")]
    Channel(#[from] ChannelError),

    /// Protocol error
    #[error("Protocol error: {0}")]
    Protocol(String),
}

/// Errors during encoding
#[derive(Error, Debug, Clone, PartialEq)]
pub enum EncodeError {
    /// Value is not a valid number (NaN, Inf)
    #[error("Invalid value: {0}")]
    InvalidValue(String),

    /// Buffer too small
    #[error("Buffer too small: need {needed} bytes, have {available}")]
    BufferTooSmall { needed: usize, available: usize },

    /// Payload too large
    #[error("Payload too large: {size} bytes exceeds maximum {max}")]
    PayloadTooLarge { size: usize, max: usize },

    /// Context version mismatch
    #[error("Context version mismatch: expected {expected}, got {actual}")]
    ContextMismatch { expected: u32, actual: u32 },
}

/// Errors during decoding
#[derive(Error, Debug, Clone, PartialEq)]
pub enum DecodeError {
    /// Invalid checksum
    #[error("Invalid checksum: expected {expected:08x}, got {actual:08x}")]
    InvalidChecksum { expected: u32, actual: u32 },

    /// Context mismatch (can't decode without correct context)
    #[error("Context mismatch: expected version {expected}, message has {actual}")]
    ContextMismatch { expected: u32, actual: u32 },

    /// Malformed message
    #[error("Malformed message at offset {offset}: {reason}")]
    MalformedMessage { offset: usize, reason: String },

    /// Unknown pattern reference
    #[error("Unknown pattern ID: {pattern_id}")]
    UnknownPattern { pattern_id: u32 },

    /// Unknown encoding type
    #[error("Unknown encoding type: 0x{0:02x}")]
    UnknownEncodingType(u8),

    /// Unknown message type
    #[error("Unknown message type: {0}")]
    UnknownMessageType(u8),

    /// Buffer too short
    #[error("Buffer too short: need at least {needed} bytes, got {available}")]
    BufferTooShort { needed: usize, available: usize },

    /// Invalid header
    #[error("Invalid header")]
    InvalidHeader,
}

/// Errors related to the shared context
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ContextError {
    /// Hash mismatch during sync
    #[error("Hash mismatch: expected {expected:016x}, got {actual:016x}")]
    HashMismatch { expected: u64, actual: u64 },

    /// Version gap too large
    #[error("Version gap too large: from {from} to {to}")]
    VersionGapTooLarge { from: u32, to: u32 },

    /// Dictionary full
    #[error("Dictionary full: maximum {max} patterns reached")]
    DictionaryFull { max: usize },

    /// Pattern too large
    #[error("Pattern too large: {size} bytes exceeds maximum {max}")]
    PatternTooLarge { size: usize, max: usize },

    /// Sync failed
    #[error("Synchronization failed: {reason}")]
    SyncFailed { reason: String },

    /// Memory limit exceeded
    #[error("Memory limit exceeded: {used} bytes exceeds {limit}")]
    MemoryLimitExceeded { used: usize, limit: usize },
}

/// Errors related to the communication channel
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ChannelError {
    /// Connection timeout
    #[error("Connection timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// Disconnected
    #[error("Disconnected: {reason}")]
    Disconnected { reason: String },

    /// Buffer full
    #[error("Send buffer full")]
    BufferFull,

    /// Transmission error
    #[error("Transmission error after {retries} retries")]
    TransmissionError { retries: u8 },

    /// Rate limited
    #[error("Rate limited: retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AlecError::Decode(DecodeError::InvalidChecksum {
            expected: 0x12345678,
            actual: 0xABCDEF00,
        });
        let msg = format!("{}", err);
        assert!(msg.contains("checksum"));
        assert!(msg.contains("12345678"));
    }

    #[test]
    fn test_error_conversion() {
        let encode_err = EncodeError::InvalidValue("NaN".to_string());
        let alec_err: AlecError = encode_err.into();
        assert!(matches!(alec_err, AlecError::Encode(_)));
    }
}
