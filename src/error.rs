// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Error types for ALEC
//!
//! This module defines all error types used throughout the library.

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg(feature = "std")]
use thiserror::Error;

/// Result type alias for ALEC operations
pub type Result<T> = core::result::Result<T, AlecError>;

/// Main error type for ALEC operations
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Error))]
pub enum AlecError {
    /// Encoding error
    #[cfg_attr(feature = "std", error("Encoding error: {0}"))]
    Encode(#[cfg_attr(feature = "std", from)] EncodeError),

    /// Decoding error
    #[cfg_attr(feature = "std", error("Decoding error: {0}"))]
    Decode(#[cfg_attr(feature = "std", from)] DecodeError),

    /// Context error
    #[cfg_attr(feature = "std", error("Context error: {0}"))]
    Context(#[cfg_attr(feature = "std", from)] ContextError),

    /// Channel error
    #[cfg_attr(feature = "std", error("Channel error: {0}"))]
    Channel(#[cfg_attr(feature = "std", from)] ChannelError),

    /// Protocol error
    #[cfg_attr(feature = "std", error("Protocol error: {0}"))]
    Protocol(String),
}

#[cfg(not(feature = "std"))]
impl core::fmt::Display for AlecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AlecError::Encode(e) => write!(f, "Encoding error: {}", e),
            AlecError::Decode(e) => write!(f, "Decoding error: {}", e),
            AlecError::Context(e) => write!(f, "Context error: {}", e),
            AlecError::Channel(e) => write!(f, "Channel error: {}", e),
            AlecError::Protocol(s) => write!(f, "Protocol error: {}", s),
        }
    }
}

#[cfg(not(feature = "std"))]
impl From<EncodeError> for AlecError {
    fn from(e: EncodeError) -> Self {
        AlecError::Encode(e)
    }
}

#[cfg(not(feature = "std"))]
impl From<DecodeError> for AlecError {
    fn from(e: DecodeError) -> Self {
        AlecError::Decode(e)
    }
}

#[cfg(not(feature = "std"))]
impl From<ContextError> for AlecError {
    fn from(e: ContextError) -> Self {
        AlecError::Context(e)
    }
}

#[cfg(not(feature = "std"))]
impl From<ChannelError> for AlecError {
    fn from(e: ChannelError) -> Self {
        AlecError::Channel(e)
    }
}

/// Errors during encoding
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Error))]
pub enum EncodeError {
    /// Value is not a valid number (NaN, Inf)
    #[cfg_attr(feature = "std", error("Invalid value: {0}"))]
    InvalidValue(String),

    /// Buffer too small
    #[cfg_attr(
        feature = "std",
        error("Buffer too small: need {needed} bytes, have {available}")
    )]
    BufferTooSmall { needed: usize, available: usize },

    /// Payload too large
    #[cfg_attr(
        feature = "std",
        error("Payload too large: {size} bytes exceeds maximum {max}")
    )]
    PayloadTooLarge { size: usize, max: usize },

    /// Context version mismatch
    #[cfg_attr(
        feature = "std",
        error("Context version mismatch: expected {expected}, got {actual}")
    )]
    ContextMismatch { expected: u32, actual: u32 },
}

#[cfg(not(feature = "std"))]
impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EncodeError::InvalidValue(s) => write!(f, "Invalid value: {}", s),
            EncodeError::BufferTooSmall { needed, available } => {
                write!(
                    f,
                    "Buffer too small: need {} bytes, have {}",
                    needed, available
                )
            }
            EncodeError::PayloadTooLarge { size, max } => {
                write!(
                    f,
                    "Payload too large: {} bytes exceeds maximum {}",
                    size, max
                )
            }
            EncodeError::ContextMismatch { expected, actual } => {
                write!(
                    f,
                    "Context version mismatch: expected {}, got {}",
                    expected, actual
                )
            }
        }
    }
}

/// Errors during decoding
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Error))]
pub enum DecodeError {
    /// Invalid checksum
    #[cfg_attr(
        feature = "std",
        error("Invalid checksum: expected {expected:08x}, got {actual:08x}")
    )]
    InvalidChecksum { expected: u32, actual: u32 },

    /// Context mismatch (can't decode without correct context)
    #[cfg_attr(
        feature = "std",
        error("Context mismatch: expected version {expected}, message has {actual}")
    )]
    ContextMismatch { expected: u32, actual: u32 },

    /// Malformed message
    #[cfg_attr(
        feature = "std",
        error("Malformed message at offset {offset}: {reason}")
    )]
    MalformedMessage { offset: usize, reason: String },

    /// Unknown pattern reference
    #[cfg_attr(feature = "std", error("Unknown pattern ID: {pattern_id}"))]
    UnknownPattern { pattern_id: u32 },

    /// Unknown encoding type
    #[cfg_attr(feature = "std", error("Unknown encoding type: 0x{0:02x}"))]
    UnknownEncodingType(u8),

    /// Unknown message type
    #[cfg_attr(feature = "std", error("Unknown message type: {0}"))]
    UnknownMessageType(u8),

    /// Buffer too short
    #[cfg_attr(
        feature = "std",
        error("Buffer too short: need at least {needed} bytes, got {available}")
    )]
    BufferTooShort { needed: usize, available: usize },

    /// Invalid header
    #[cfg_attr(feature = "std", error("Invalid header"))]
    InvalidHeader,
}

#[cfg(not(feature = "std"))]
impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DecodeError::InvalidChecksum { expected, actual } => {
                write!(
                    f,
                    "Invalid checksum: expected {:08x}, got {:08x}",
                    expected, actual
                )
            }
            DecodeError::ContextMismatch { expected, actual } => {
                write!(
                    f,
                    "Context mismatch: expected version {}, message has {}",
                    expected, actual
                )
            }
            DecodeError::MalformedMessage { offset, reason } => {
                write!(f, "Malformed message at offset {}: {}", offset, reason)
            }
            DecodeError::UnknownPattern { pattern_id } => {
                write!(f, "Unknown pattern ID: {}", pattern_id)
            }
            DecodeError::UnknownEncodingType(t) => {
                write!(f, "Unknown encoding type: 0x{:02x}", t)
            }
            DecodeError::UnknownMessageType(t) => {
                write!(f, "Unknown message type: {}", t)
            }
            DecodeError::BufferTooShort { needed, available } => {
                write!(
                    f,
                    "Buffer too short: need at least {} bytes, got {}",
                    needed, available
                )
            }
            DecodeError::InvalidHeader => write!(f, "Invalid header"),
        }
    }
}

/// Errors related to the shared context
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Error))]
pub enum ContextError {
    /// Hash mismatch during sync
    #[cfg_attr(
        feature = "std",
        error("Hash mismatch: expected {expected:016x}, got {actual:016x}")
    )]
    HashMismatch { expected: u64, actual: u64 },

    /// Version gap too large
    #[cfg_attr(feature = "std", error("Version gap too large: from {from} to {to}"))]
    VersionGapTooLarge { from: u32, to: u32 },

    /// Dictionary full
    #[cfg_attr(
        feature = "std",
        error("Dictionary full: maximum {max} patterns reached")
    )]
    DictionaryFull { max: usize },

    /// Pattern too large
    #[cfg_attr(
        feature = "std",
        error("Pattern too large: {size} bytes exceeds maximum {max}")
    )]
    PatternTooLarge { size: usize, max: usize },

    /// Sync failed
    #[cfg_attr(feature = "std", error("Synchronization failed: {reason}"))]
    SyncFailed { reason: String },

    /// Memory limit exceeded
    #[cfg_attr(
        feature = "std",
        error("Memory limit exceeded: {used} bytes exceeds {limit}")
    )]
    MemoryLimitExceeded { used: usize, limit: usize },
}

#[cfg(not(feature = "std"))]
impl core::fmt::Display for ContextError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ContextError::HashMismatch { expected, actual } => {
                write!(
                    f,
                    "Hash mismatch: expected {:016x}, got {:016x}",
                    expected, actual
                )
            }
            ContextError::VersionGapTooLarge { from, to } => {
                write!(f, "Version gap too large: from {} to {}", from, to)
            }
            ContextError::DictionaryFull { max } => {
                write!(f, "Dictionary full: maximum {} patterns reached", max)
            }
            ContextError::PatternTooLarge { size, max } => {
                write!(
                    f,
                    "Pattern too large: {} bytes exceeds maximum {}",
                    size, max
                )
            }
            ContextError::SyncFailed { reason } => {
                write!(f, "Synchronization failed: {}", reason)
            }
            ContextError::MemoryLimitExceeded { used, limit } => {
                write!(f, "Memory limit exceeded: {} bytes exceeds {}", used, limit)
            }
        }
    }
}

/// Errors related to the communication channel
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Error))]
pub enum ChannelError {
    /// Connection timeout
    #[cfg_attr(feature = "std", error("Connection timeout after {timeout_ms}ms"))]
    Timeout { timeout_ms: u64 },

    /// Disconnected
    #[cfg_attr(feature = "std", error("Disconnected: {reason}"))]
    Disconnected { reason: String },

    /// Buffer full
    #[cfg_attr(feature = "std", error("Send buffer full"))]
    BufferFull,

    /// Transmission error
    #[cfg_attr(feature = "std", error("Transmission error after {retries} retries"))]
    TransmissionError { retries: u8 },

    /// Rate limited
    #[cfg_attr(feature = "std", error("Rate limited: retry after {retry_after_ms}ms"))]
    RateLimited { retry_after_ms: u64 },
}

#[cfg(not(feature = "std"))]
impl core::fmt::Display for ChannelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ChannelError::Timeout { timeout_ms } => {
                write!(f, "Connection timeout after {}ms", timeout_ms)
            }
            ChannelError::Disconnected { reason } => {
                write!(f, "Disconnected: {}", reason)
            }
            ChannelError::BufferFull => write!(f, "Send buffer full"),
            ChannelError::TransmissionError { retries } => {
                write!(f, "Transmission error after {} retries", retries)
            }
            ChannelError::RateLimited { retry_after_ms } => {
                write!(f, "Rate limited: retry after {}ms", retry_after_ms)
            }
        }
    }
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
