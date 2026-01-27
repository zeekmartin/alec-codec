// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! ALEC Gateway Metrics Module
//!
//! Provides information-theoretic observability for multi-sensor gateways.
//!
//! ## Features
//!
//! - **Signal-level entropy**: H(X_i), H_joint, Total Correlation
//! - **Payload-level entropy**: Shannon entropy over frame bytes
//! - **Resilience index**: Normalized redundancy indicator R
//! - **Criticality ranking**: Per-channel failure impact via leave-one-out
//!
//! ## Design Principles
//!
//! 1. **Passive by default**: Metrics observe; they never alter encoding.
//! 2. **Orthogonal integration**: Failures in metrics never block the main data path.
//! 3. **Opt-in overhead**: Disabled by default; near-zero cost when off.
//!
//! ## Example
//!
//! ```rust,ignore
//! use alec_gateway::Gateway;
//! use alec_gateway::metrics::{MetricsEngine, MetricsConfig, ResilienceConfig};
//!
//! let mut gateway = Gateway::new();
//!
//! // Enable metrics with resilience
//! gateway.enable_metrics(MetricsConfig {
//!     enabled: true,
//!     resilience: ResilienceConfig {
//!         enabled: true,
//!         ..Default::default()
//!     },
//!     ..Default::default()
//! });
//!
//! // Add channels and push samples
//! gateway.add_channel("temp_1", Default::default()).unwrap();
//! gateway.push("temp_1", 22.5, 1000).unwrap();
//!
//! // On flush, metrics are computed
//! let frame = gateway.flush().unwrap();
//!
//! // Get the metrics snapshot
//! if let Some(snapshot) = gateway.last_metrics() {
//!     println!("{}", snapshot.to_json().unwrap());
//! }
//! ```
//!
//! ## Two Perspectives on Entropy
//!
//! | Metric Type | Source | Purpose |
//! |-------------|--------|---------|
//! | **Signal-level** (pre-encoding) | Raw sensor samples | Information content in the world |
//! | **Payload-level** (post-encoding) | Frame bytes | Information in transmitted data |

mod alignment;
mod config;
mod engine;
mod payload;
mod resilience;
mod signal;
mod snapshot;
mod window;

// Re-export public API
pub use config::*;
pub use engine::MetricsEngine;
pub use payload::{ChannelPayloadMetrics, PayloadMetrics};
pub use resilience::{ChannelCriticality, ResilienceMetrics, ResilienceZone};
pub use signal::{ChannelEntropy, SignalMetrics};
pub use snapshot::MetricsSnapshot;
