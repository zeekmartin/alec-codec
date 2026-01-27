// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! # ALEC Gateway - Multi-sensor orchestration layer
//!
//! This crate provides a higher-level API for managing multiple ALEC
//! encoder instances on IoT gateways.
//!
//! ## Overview
//!
//! ALEC Gateway manages multiple ALEC encoder instances for IoT gateways
//! that aggregate data from many sensors into efficient transmission frames.
//!
//! ## Features
//!
//! - **Multi-channel management**: Handle dozens of sensor channels
//! - **Priority-based aggregation**: Critical sensors get bandwidth first
//! - **Frame packing**: Optimize for LoRaWAN/MQTT payload limits
//! - **Preload support**: Load pre-trained contexts per channel
//!
//! ## Quick Start
//!
//! ```rust
//! use alec_gateway::{Gateway, ChannelConfig, GatewayConfig};
//!
//! // Create gateway with LoRaWAN frame limit
//! let config = GatewayConfig {
//!     max_frame_size: 242,
//!     ..Default::default()
//! };
//! let mut gateway = Gateway::with_config(config);
//!
//! // Add sensor channels
//! gateway.add_channel("temperature", ChannelConfig {
//!     priority: 1,  // High priority
//!     ..Default::default()
//! }).unwrap();
//!
//! gateway.add_channel("humidity", ChannelConfig {
//!     priority: 2,
//!     ..Default::default()
//! }).unwrap();
//!
//! // Collect sensor readings
//! let timestamp = 1234567890;
//! gateway.push("temperature", 22.5, timestamp).unwrap();
//! gateway.push("temperature", 22.6, timestamp + 1000).unwrap();
//! gateway.push("humidity", 65.0, timestamp).unwrap();
//!
//! // Get aggregated frame
//! let frame = gateway.flush().unwrap();
//! println!("Frame size: {} bytes", frame.size());
//!
//! // Send frame.to_bytes() over LoRaWAN, MQTT, etc.
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  IoT Gateway                                                │
//! │  ┌────────────────────────────────────────────────────────┐│
//! │  │  ALEC Gateway                                          ││
//! │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐               ││
//! │  │  │ Channel  │ │ Channel  │ │ Channel  │  ...          ││
//! │  │  │ Temp #1  │ │ Humid #1 │ │ Accel #1 │               ││
//! │  │  │ [Context]│ │ [Context]│ │ [Context]│               ││
//! │  │  └────┬─────┘ └────┬─────┘ └────┬─────┘               ││
//! │  │       │            │            │                      ││
//! │  │       └────────────┼────────────┘                      ││
//! │  │                    ▼                                   ││
//! │  │              ┌───────────┐                             ││
//! │  │              │ Aggregator│                             ││
//! │  │              └─────┬─────┘                             ││
//! │  │                    ▼                                   ││
//! │  │              ┌───────────┐                             ││
//! │  │              │  Frame    │  → LoRaWAN / MQTT / etc.   ││
//! │  │              └───────────┘                             ││
//! │  └────────────────────────────────────────────────────────┘│
//! └─────────────────────────────────────────────────────────────┘
//! ```

mod aggregator;
mod channel_manager;
mod config;
mod error;
mod frame;
mod gateway;

// Metrics module (feature-gated)
#[cfg(feature = "metrics")]
pub mod metrics;

// Public API
pub use aggregator::Aggregator;
pub use channel_manager::{Channel, ChannelId, ChannelManager};
pub use config::{ChannelConfig, GatewayConfig};
pub use error::{GatewayError, Result};
pub use frame::{ChannelData, Frame, FrameBuilder, FrameParseError};
pub use gateway::Gateway;

// Metrics re-exports (feature-gated)
#[cfg(feature = "metrics")]
pub use metrics::{
    MetricsConfig, MetricsEngine, MetricsSnapshot, PayloadMetrics, ResilienceConfig,
    ResilienceMetrics, ResilienceZone, SignalMetrics,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
