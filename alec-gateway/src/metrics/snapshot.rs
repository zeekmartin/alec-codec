// ALEC Gateway - Multi-sensor orchestration layer
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! MetricsSnapshot serialization for export.

use super::payload::PayloadMetrics;
use super::resilience::ResilienceMetrics;
use super::signal::SignalMetrics;
use serde::{Deserialize, Serialize};

/// Complete metrics snapshot for export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Schema version.
    pub version: u32,
    /// Snapshot timestamp (UTC epoch ms).
    pub timestamp_ms: u64,
    /// Window information.
    pub window: WindowInfo,
    /// Signal-level metrics (may be invalid).
    pub signal: SignalSnapshot,
    /// Payload-level metrics (always available).
    pub payload: PayloadSnapshot,
    /// Resilience metrics (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resilience: Option<ResilienceSnapshot>,
    /// Computation flags.
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub kind: String,
    pub value: u64,
    pub aligned_samples: usize,
    pub channels_included: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalSnapshot {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid_reason: Option<String>,
    pub log_base: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub h_per_channel: Vec<ChannelEntropyJson>,
    pub sum_h: f64,
    pub h_joint: f64,
    pub total_corr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelEntropyJson {
    pub channel_id: String,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadSnapshot {
    pub frame_size_bytes: usize,
    pub h_bytes: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub histogram: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_channel: Option<Vec<ChannelPayloadJson>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelPayloadJson {
    pub channel_id: String,
    pub size_bytes: usize,
    pub h_bytes: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilienceSnapshot {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub criticality: Option<CriticalitySnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalitySnapshot {
    pub enabled: bool,
    pub ranking: Vec<CriticalityRankingJson>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalityRankingJson {
    pub channel_id: String,
    pub delta_r: f64,
}

impl MetricsSnapshot {
    /// Current schema version.
    pub const VERSION: u32 = 1;

    /// Create a snapshot from computed metrics.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        timestamp_ms: u64,
        window_kind: &str,
        window_value: u64,
        signal: Option<&SignalMetrics>,
        payload: &PayloadMetrics,
        resilience: Option<&ResilienceMetrics>,
        log_base: &str,
        flags: Vec<String>,
    ) -> Self {
        let (signal_snapshot, aligned_samples, channels_included) = if let Some(s) = signal {
            (
                SignalSnapshot {
                    valid: true,
                    invalid_reason: None,
                    log_base: log_base.to_string(),
                    h_per_channel: s
                        .h_per_channel
                        .iter()
                        .map(|c| ChannelEntropyJson {
                            channel_id: c.channel_id.clone(),
                            h: c.h,
                        })
                        .collect(),
                    sum_h: s.sum_h,
                    h_joint: s.h_joint,
                    total_corr: s.total_correlation,
                },
                s.aligned_samples,
                s.channels_included,
            )
        } else {
            (
                SignalSnapshot {
                    valid: false,
                    invalid_reason: Some("insufficient_samples".to_string()),
                    log_base: log_base.to_string(),
                    h_per_channel: vec![],
                    sum_h: 0.0,
                    h_joint: 0.0,
                    total_corr: 0.0,
                },
                0,
                0,
            )
        };

        let payload_snapshot = PayloadSnapshot {
            frame_size_bytes: payload.frame_size_bytes,
            h_bytes: payload.h_bytes,
            histogram: payload.histogram.map(|h| h.to_vec()),
            per_channel: payload.per_channel.as_ref().map(|channels| {
                channels
                    .iter()
                    .map(|c| ChannelPayloadJson {
                        channel_id: c.channel_id.clone(),
                        size_bytes: c.size_bytes,
                        h_bytes: c.h_bytes,
                    })
                    .collect()
            }),
        };

        let resilience_snapshot = resilience.map(|r| ResilienceSnapshot {
            enabled: true,
            r: Some(r.r),
            zone: Some(r.zone.as_str().to_string()),
            criticality: r.criticality.as_ref().map(|crit| CriticalitySnapshot {
                enabled: true,
                ranking: crit
                    .iter()
                    .map(|c| CriticalityRankingJson {
                        channel_id: c.channel_id.clone(),
                        delta_r: c.delta_r,
                    })
                    .collect(),
                note: "delta_r = R_all - R_without_channel (leave-one-out)".to_string(),
            }),
        });

        Self {
            version: Self::VERSION,
            timestamp_ms,
            window: WindowInfo {
                kind: window_kind.to_string(),
                value: window_value,
                aligned_samples,
                channels_included,
            },
            signal: signal_snapshot,
            payload: payload_snapshot,
            resilience: resilience_snapshot,
            flags,
        }
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Serialize to compact JSON string.
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::resilience::ResilienceZone;
    use crate::metrics::signal::ChannelEntropy;

    fn create_test_payload() -> PayloadMetrics {
        PayloadMetrics {
            frame_size_bytes: 100,
            h_bytes: 5.5,
            histogram: None,
            per_channel: None,
        }
    }

    fn create_test_signal() -> SignalMetrics {
        SignalMetrics {
            h_per_channel: vec![
                ChannelEntropy {
                    channel_id: "ch1".to_string(),
                    h: 2.0,
                },
                ChannelEntropy {
                    channel_id: "ch2".to_string(),
                    h: 2.5,
                },
            ],
            sum_h: 4.5,
            h_joint: 3.0,
            total_correlation: 1.5,
            aligned_samples: 50,
            channels_included: 2,
        }
    }

    #[test]
    fn test_snapshot_creation() {
        let payload = create_test_payload();
        let signal = create_test_signal();

        let snapshot = MetricsSnapshot::new(
            1234567890,
            "time_ms",
            60_000,
            Some(&signal),
            &payload,
            None,
            "log2",
            vec!["FLAG1".to_string()],
        );

        assert_eq!(snapshot.version, MetricsSnapshot::VERSION);
        assert_eq!(snapshot.timestamp_ms, 1234567890);
        assert!(snapshot.signal.valid);
        assert_eq!(snapshot.payload.frame_size_bytes, 100);
    }

    #[test]
    fn test_snapshot_without_signal() {
        let payload = create_test_payload();

        let snapshot = MetricsSnapshot::new(
            1234567890,
            "time_ms",
            60_000,
            None, // No signal
            &payload,
            None,
            "log2",
            vec![],
        );

        assert!(!snapshot.signal.valid);
        assert!(snapshot.signal.invalid_reason.is_some());
    }

    #[test]
    fn test_snapshot_with_resilience() {
        let payload = create_test_payload();
        let signal = create_test_signal();
        let resilience = ResilienceMetrics {
            r: 0.33,
            zone: ResilienceZone::Attention,
            criticality: None,
        };

        let snapshot = MetricsSnapshot::new(
            1234567890,
            "time_ms",
            60_000,
            Some(&signal),
            &payload,
            Some(&resilience),
            "log2",
            vec![],
        );

        let res = snapshot.resilience.unwrap();
        assert!(res.enabled);
        assert_eq!(res.r, Some(0.33));
        assert_eq!(res.zone, Some("attention".to_string()));
    }

    #[test]
    fn test_json_serialization() {
        let payload = create_test_payload();
        let signal = create_test_signal();

        let snapshot = MetricsSnapshot::new(
            1234567890,
            "time_ms",
            60_000,
            Some(&signal),
            &payload,
            None,
            "log2",
            vec![],
        );

        let json = snapshot.to_json().unwrap();
        assert!(json.contains("\"version\""));
        assert!(json.contains("\"signal\""));
        assert!(json.contains("\"payload\""));
    }

    #[test]
    fn test_json_roundtrip() {
        let payload = create_test_payload();
        let signal = create_test_signal();

        let snapshot = MetricsSnapshot::new(
            1234567890,
            "time_ms",
            60_000,
            Some(&signal),
            &payload,
            None,
            "log2",
            vec!["TEST_FLAG".to_string()],
        );

        let json = snapshot.to_json().unwrap();
        let parsed = MetricsSnapshot::from_json(&json).unwrap();

        assert_eq!(parsed.version, snapshot.version);
        assert_eq!(parsed.timestamp_ms, snapshot.timestamp_ms);
        assert_eq!(parsed.signal.sum_h, snapshot.signal.sum_h);
        assert_eq!(parsed.flags, snapshot.flags);
    }

    #[test]
    fn test_compact_json() {
        let payload = create_test_payload();

        let snapshot = MetricsSnapshot::new(
            1234567890,
            "time_ms",
            60_000,
            None,
            &payload,
            None,
            "log2",
            vec![],
        );

        let compact = snapshot.to_json_compact().unwrap();
        let pretty = snapshot.to_json().unwrap();

        // Compact should be shorter (no whitespace)
        assert!(compact.len() < pretty.len());
    }

    #[test]
    fn test_optional_fields_skipped() {
        let payload = create_test_payload();

        let snapshot = MetricsSnapshot::new(
            1234567890,
            "time_ms",
            60_000,
            None,
            &payload,
            None,
            "log2",
            vec![],
        );

        let json = snapshot.to_json().unwrap();

        // resilience should not appear if None
        assert!(!json.contains("\"resilience\""));
    }

    #[test]
    fn test_window_info() {
        let payload = create_test_payload();
        let signal = create_test_signal();

        let snapshot = MetricsSnapshot::new(
            1234567890,
            "last_n",
            100,
            Some(&signal),
            &payload,
            None,
            "log2",
            vec![],
        );

        assert_eq!(snapshot.window.kind, "last_n");
        assert_eq!(snapshot.window.value, 100);
        assert_eq!(snapshot.window.aligned_samples, 50);
        assert_eq!(snapshot.window.channels_included, 2);
    }
}
