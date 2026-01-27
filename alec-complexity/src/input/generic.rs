// ALEC Complexity - Standalone complexity monitoring
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Generic JSON input adapter.
//!
//! Accepts a simple JSON format for integration with any metrics source.

use super::{ChannelEntropy, InputAdapter, InputSnapshot};
use serde::{Deserialize, Serialize};

/// Generic JSON input format.
///
/// Example JSON:
/// ```json
/// {
///   "timestamp_ms": 1706000000000,
///   "h_bytes": 6.5,
///   "tc": 2.3,
///   "h_joint": 8.1,
///   "r": 0.45,
///   "channels": [
///     {"id": "temp", "h": 3.2},
///     {"id": "humid", "h": 2.8}
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericInput {
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,

    /// Payload/byte entropy (required).
    pub h_bytes: f64,

    /// Total Correlation (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tc: Option<f64>,

    /// Joint entropy (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h_joint: Option<f64>,

    /// Resilience index (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r: Option<f64>,

    /// Per-channel entropies (optional).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<GenericChannelInput>,
}

/// Per-channel input in generic format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericChannelInput {
    pub id: String,
    pub h: f64,
}

impl GenericInput {
    /// Parse from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Create with minimal required fields.
    pub fn new(timestamp_ms: u64, h_bytes: f64) -> Self {
        Self {
            timestamp_ms,
            h_bytes,
            tc: None,
            h_joint: None,
            r: None,
            channels: Vec::new(),
        }
    }

    /// Builder: add Total Correlation.
    pub fn with_tc(mut self, tc: f64) -> Self {
        self.tc = Some(tc);
        self
    }

    /// Builder: add joint entropy.
    pub fn with_h_joint(mut self, h_joint: f64) -> Self {
        self.h_joint = Some(h_joint);
        self
    }

    /// Builder: add resilience.
    pub fn with_r(mut self, r: f64) -> Self {
        self.r = Some(r);
        self
    }

    /// Builder: add channel.
    pub fn with_channel(mut self, id: &str, h: f64) -> Self {
        self.channels.push(GenericChannelInput {
            id: id.to_string(),
            h,
        });
        self
    }

    /// Finalize builder and convert to InputSnapshot.
    pub fn build(self) -> InputSnapshot {
        self.to_input_snapshot()
    }

    /// Convert to InputSnapshot (alias for to_input_snapshot).
    pub fn to_snapshot(&self) -> InputSnapshot {
        self.to_input_snapshot()
    }
}

impl InputAdapter for GenericInput {
    fn to_input_snapshot(&self) -> InputSnapshot {
        InputSnapshot {
            timestamp_ms: self.timestamp_ms,
            tc: self.tc,
            h_joint: self.h_joint,
            h_bytes: self.h_bytes,
            r: self.r,
            channel_entropies: self
                .channels
                .iter()
                .map(|ch| ChannelEntropy {
                    channel_id: ch.id.clone(),
                    h: ch.h,
                })
                .collect(),
            source: "generic-json".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_input_new() {
        let input = GenericInput::new(1000, 5.5);
        assert_eq!(input.timestamp_ms, 1000);
        assert_eq!(input.h_bytes, 5.5);
        assert!(input.tc.is_none());
        assert!(input.channels.is_empty());
    }

    #[test]
    fn test_generic_input_builder() {
        let input = GenericInput::new(1000, 5.5)
            .with_tc(2.0)
            .with_h_joint(8.0)
            .with_r(0.45)
            .with_channel("temp", 3.0)
            .with_channel("humid", 2.5);

        assert_eq!(input.tc, Some(2.0));
        assert_eq!(input.h_joint, Some(8.0));
        assert_eq!(input.r, Some(0.45));
        assert_eq!(input.channels.len(), 2);
    }

    #[test]
    fn test_generic_input_to_snapshot() {
        let input = GenericInput::new(1000, 5.5)
            .with_tc(2.0)
            .with_channel("ch1", 3.0);

        let snapshot = input.to_input_snapshot();
        assert_eq!(snapshot.timestamp_ms, 1000);
        assert_eq!(snapshot.h_bytes, 5.5);
        assert_eq!(snapshot.tc, Some(2.0));
        assert_eq!(snapshot.channel_entropies.len(), 1);
        assert_eq!(snapshot.source, "generic-json");
    }

    #[test]
    fn test_json_roundtrip() {
        let input = GenericInput::new(1000, 5.5)
            .with_tc(2.0)
            .with_h_joint(8.0)
            .with_channel("temp", 3.0);

        let json = input.to_json().unwrap();
        let parsed = GenericInput::from_json(&json).unwrap();

        assert_eq!(parsed.timestamp_ms, input.timestamp_ms);
        assert_eq!(parsed.h_bytes, input.h_bytes);
        assert_eq!(parsed.tc, input.tc);
        assert_eq!(parsed.channels.len(), input.channels.len());
    }

    #[test]
    fn test_json_parsing() {
        let json = r#"{
            "timestamp_ms": 1706000000000,
            "h_bytes": 6.5,
            "tc": 2.3,
            "h_joint": 8.1,
            "r": 0.45,
            "channels": [
                {"id": "temp", "h": 3.2},
                {"id": "humid", "h": 2.8}
            ]
        }"#;

        let input = GenericInput::from_json(json).unwrap();
        assert_eq!(input.timestamp_ms, 1706000000000);
        assert_eq!(input.h_bytes, 6.5);
        assert_eq!(input.tc, Some(2.3));
        assert_eq!(input.h_joint, Some(8.1));
        assert_eq!(input.r, Some(0.45));
        assert_eq!(input.channels.len(), 2);
    }

    #[test]
    fn test_minimal_json() {
        let json = r#"{"timestamp_ms": 1000, "h_bytes": 5.5}"#;
        let input = GenericInput::from_json(json).unwrap();
        assert_eq!(input.timestamp_ms, 1000);
        assert_eq!(input.h_bytes, 5.5);
        assert!(input.tc.is_none());
        assert!(input.channels.is_empty());
    }
}
