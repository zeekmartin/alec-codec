// ALEC Complexity - Standalone complexity monitoring
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Input adapters for ComplexityEngine.
//!
//! Complexity can consume data from multiple sources:
//! - ALEC Gateway MetricsSnapshot (with `gateway` feature)
//! - Generic JSON input
//! - Custom adapters via `InputAdapter` trait

mod generic;

#[cfg(feature = "gateway")]
mod gateway;

pub use generic::GenericInput;

#[cfg(feature = "gateway")]
pub use gateway::{GatewayInput, MetricsSnapshotExt};

/// Unified input snapshot for ComplexityEngine.
#[derive(Debug, Clone)]
pub struct InputSnapshot {
    /// Timestamp in milliseconds (UTC epoch).
    pub timestamp_ms: u64,

    /// Total Correlation (optional - requires multi-channel signal analysis).
    pub tc: Option<f64>,

    /// Joint entropy (optional - requires multi-channel signal analysis).
    pub h_joint: Option<f64>,

    /// Payload/byte entropy (required).
    pub h_bytes: f64,

    /// Resilience index R (optional).
    pub r: Option<f64>,

    /// Per-channel entropy values for S-lite structure analysis.
    pub channel_entropies: Vec<ChannelEntropy>,

    /// Source identifier for debugging/logging.
    pub source: String,
}

/// Per-channel entropy information.
#[derive(Debug, Clone)]
pub struct ChannelEntropy {
    pub channel_id: String,
    pub h: f64,
}

impl InputSnapshot {
    /// Create a minimal input with only payload entropy.
    pub fn minimal(timestamp_ms: u64, h_bytes: f64) -> Self {
        Self {
            timestamp_ms,
            tc: None,
            h_joint: None,
            h_bytes,
            r: None,
            channel_entropies: Vec::new(),
            source: "minimal".to_string(),
        }
    }

    /// Check if signal-level metrics are available.
    pub fn has_signal_metrics(&self) -> bool {
        self.tc.is_some() && self.h_joint.is_some()
    }

    /// Check if resilience metrics are available.
    pub fn has_resilience(&self) -> bool {
        self.r.is_some()
    }

    /// Check if structure analysis is possible.
    pub fn can_compute_structure(&self) -> bool {
        self.channel_entropies.len() >= 2
    }
}

/// Trait for input adapters.
pub trait InputAdapter {
    /// Convert source data to unified InputSnapshot.
    fn to_input_snapshot(&self) -> InputSnapshot;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_input() {
        let input = InputSnapshot::minimal(1000, 5.5);
        assert_eq!(input.timestamp_ms, 1000);
        assert_eq!(input.h_bytes, 5.5);
        assert!(!input.has_signal_metrics());
        assert!(!input.has_resilience());
        assert!(!input.can_compute_structure());
    }

    #[test]
    fn test_has_signal_metrics() {
        let mut input = InputSnapshot::minimal(1000, 5.5);
        assert!(!input.has_signal_metrics());

        input.tc = Some(2.0);
        assert!(!input.has_signal_metrics());

        input.h_joint = Some(8.0);
        assert!(input.has_signal_metrics());
    }

    #[test]
    fn test_can_compute_structure() {
        let mut input = InputSnapshot::minimal(1000, 5.5);
        assert!(!input.can_compute_structure());

        input.channel_entropies.push(ChannelEntropy {
            channel_id: "ch1".to_string(),
            h: 2.0,
        });
        assert!(!input.can_compute_structure());

        input.channel_entropies.push(ChannelEntropy {
            channel_id: "ch2".to_string(),
            h: 3.0,
        });
        assert!(input.can_compute_structure());
    }
}
