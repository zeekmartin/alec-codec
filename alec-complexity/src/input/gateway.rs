// ALEC Complexity - Standalone complexity monitoring
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Gateway MetricsSnapshot adapter.
//!
//! Converts alec-gateway MetricsSnapshot to InputSnapshot.

use super::{ChannelEntropy, InputAdapter, InputSnapshot};
use alec_gateway::metrics::MetricsSnapshot;

/// Adapter for Gateway MetricsSnapshot.
pub struct GatewayInput<'a> {
    snapshot: &'a MetricsSnapshot,
}

impl<'a> GatewayInput<'a> {
    pub fn new(snapshot: &'a MetricsSnapshot) -> Self {
        Self { snapshot }
    }
}

impl InputAdapter for GatewayInput<'_> {
    fn to_input_snapshot(&self) -> InputSnapshot {
        let channel_entropies = if self.snapshot.signal.valid {
            self.snapshot
                .signal
                .h_per_channel
                .iter()
                .map(|ch| ChannelEntropy {
                    channel_id: ch.channel_id.clone(),
                    h: ch.h,
                })
                .collect()
        } else {
            Vec::new()
        };

        let (tc, h_joint) = if self.snapshot.signal.valid {
            (
                Some(self.snapshot.signal.total_corr),
                Some(self.snapshot.signal.h_joint),
            )
        } else {
            (None, None)
        };

        let r = self.snapshot.resilience.as_ref().and_then(|res| res.r);

        InputSnapshot {
            timestamp_ms: self.snapshot.timestamp_ms,
            tc,
            h_joint,
            h_bytes: self.snapshot.payload.h_bytes,
            r,
            channel_entropies,
            source: "alec-gateway".to_string(),
        }
    }
}

/// Extension trait for MetricsSnapshot.
pub trait MetricsSnapshotExt {
    /// Convert to InputSnapshot for ComplexityEngine.
    fn to_complexity_input(&self) -> InputSnapshot;
}

impl MetricsSnapshotExt for MetricsSnapshot {
    fn to_complexity_input(&self) -> InputSnapshot {
        GatewayInput::new(self).to_input_snapshot()
    }
}
