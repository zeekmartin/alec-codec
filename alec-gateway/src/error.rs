// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Error types for ALEC Gateway

use thiserror::Error;

/// Main error type for Gateway operations
#[derive(Error, Debug)]
pub enum GatewayError {
    /// Channel not found
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    /// Channel already exists
    #[error("Channel already exists: {0}")]
    ChannelAlreadyExists(String),

    /// Encoding error from ALEC
    #[error("Encoding error: {0}")]
    EncodingError(#[from] alec::AlecError),

    /// Buffer full for channel
    #[error("Buffer full for channel: {0}")]
    BufferFull(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Frame too large
    #[error("Frame too large: {size} bytes (max: {max})")]
    FrameTooLarge { size: usize, max: usize },

    /// Maximum channels reached
    #[error("Maximum channels ({max}) reached")]
    MaxChannelsReached { max: usize },
}

/// Result type alias for Gateway operations
pub type Result<T> = std::result::Result<T, GatewayError>;
