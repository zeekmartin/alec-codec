// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Configuration types for ALEC Gateway

/// Gateway-level configuration
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Maximum frame size in bytes (default: 242 for LoRaWAN DR0)
    pub max_frame_size: usize,

    /// Maximum number of channels
    pub max_channels: usize,

    /// Enable checksums on all channels by default
    pub enable_checksums: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            max_frame_size: 242, // LoRaWAN DR0
            max_channels: 32,
            enable_checksums: true,
        }
    }
}

impl GatewayConfig {
    /// Create a new configuration with custom max frame size
    pub fn with_max_frame_size(max_frame_size: usize) -> Self {
        Self {
            max_frame_size,
            ..Default::default()
        }
    }

    /// Create a configuration for LoRaWAN with specific data rate
    pub fn lorawan(data_rate: u8) -> Self {
        let max_frame_size = match data_rate {
            0 => 51,  // DR0: SF12/125kHz
            1 => 51,  // DR1: SF11/125kHz
            2 => 51,  // DR2: SF10/125kHz
            3 => 115, // DR3: SF9/125kHz
            4 => 242, // DR4: SF8/125kHz
            5 => 242, // DR5: SF7/125kHz
            _ => 242, // Default to max
        };
        Self {
            max_frame_size,
            ..Default::default()
        }
    }
}

/// Per-channel configuration
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    /// Buffer size for pending values
    pub buffer_size: usize,

    /// Preload file path (optional)
    pub preload_path: Option<String>,

    /// Priority (0 = highest)
    pub priority: u8,

    /// Enable checksum for this channel
    pub enable_checksum: bool,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            buffer_size: 64,
            preload_path: None,
            priority: 128,
            enable_checksum: true,
        }
    }
}

impl ChannelConfig {
    /// Create a configuration with specific priority
    pub fn with_priority(priority: u8) -> Self {
        Self {
            priority,
            ..Default::default()
        }
    }

    /// Create a configuration with a preload file
    pub fn with_preload(path: impl Into<String>) -> Self {
        Self {
            preload_path: Some(path.into()),
            ..Default::default()
        }
    }

    /// Create a configuration with custom buffer size
    pub fn with_buffer_size(buffer_size: usize) -> Self {
        Self {
            buffer_size,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gateway_config_default() {
        let config = GatewayConfig::default();
        assert_eq!(config.max_frame_size, 242);
        assert_eq!(config.max_channels, 32);
        assert!(config.enable_checksums);
    }

    #[test]
    fn test_gateway_config_lorawan() {
        let dr0 = GatewayConfig::lorawan(0);
        assert_eq!(dr0.max_frame_size, 51);

        let dr4 = GatewayConfig::lorawan(4);
        assert_eq!(dr4.max_frame_size, 242);
    }

    #[test]
    fn test_channel_config_default() {
        let config = ChannelConfig::default();
        assert_eq!(config.buffer_size, 64);
        assert!(config.preload_path.is_none());
        assert_eq!(config.priority, 128);
        assert!(config.enable_checksum);
    }

    #[test]
    fn test_channel_config_with_priority() {
        let config = ChannelConfig::with_priority(1);
        assert_eq!(config.priority, 1);
    }

    #[test]
    fn test_channel_config_with_preload() {
        let config = ChannelConfig::with_preload("test.alec-context");
        assert_eq!(config.preload_path, Some("test.alec-context".to_string()));
    }
}
