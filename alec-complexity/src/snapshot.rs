// ALEC Complexity - Standalone complexity monitoring
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! ComplexitySnapshot - output structure with full state.

use crate::baseline::{Baseline, BaselineState};
use crate::delta::{Deltas, ZScores};
use crate::event::ComplexityEvent;
use crate::structure::SLite;
use serde::{Deserialize, Serialize};

/// Version of the snapshot format.
pub const SNAPSHOT_VERSION: &str = "0.1.0";

/// Complete complexity snapshot for export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexitySnapshot {
    /// Format version.
    pub version: String,
    /// Timestamp of this snapshot.
    pub timestamp_ms: u64,
    /// Baseline state summary.
    pub baseline: BaselineSummary,
    /// Current deltas from baseline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deltas: Option<Deltas>,
    /// Current z-scores.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z_scores: Option<ZScores>,
    /// Structure summary (S-lite).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s_lite: Option<SLite>,
    /// Events emitted this cycle.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub events: Vec<ComplexityEvent>,
    /// Additional flags.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub flags: Vec<String>,
}

/// Summary of baseline state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineSummary {
    /// Current state (building or locked).
    pub state: String,
    /// Number of samples collected.
    pub sample_count: usize,
    /// Progress towards lock (0.0 to 1.0).
    pub progress: f64,
    /// Baseline statistics if locked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<BaselineStats>,
}

/// Baseline statistics summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineStats {
    /// Mean TC value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tc_mean: Option<f64>,
    /// Std TC value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tc_std: Option<f64>,
    /// Mean H_joint value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h_joint_mean: Option<f64>,
    /// Std H_joint value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h_joint_std: Option<f64>,
    /// Mean H_bytes value.
    pub h_bytes_mean: f64,
    /// Std H_bytes value.
    pub h_bytes_std: f64,
    /// Mean R value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_mean: Option<f64>,
    /// Std R value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_std: Option<f64>,
}

impl ComplexitySnapshot {
    /// Create a new snapshot.
    pub fn new(
        timestamp_ms: u64,
        baseline: &Baseline,
        deltas: Option<Deltas>,
        z_scores: Option<ZScores>,
        s_lite: Option<SLite>,
        events: Vec<ComplexityEvent>,
        flags: Vec<String>,
    ) -> Self {
        Self {
            version: SNAPSHOT_VERSION.to_string(),
            timestamp_ms,
            baseline: BaselineSummary::from_baseline(baseline),
            deltas,
            z_scores,
            s_lite,
            events,
            flags,
        }
    }

    /// Create a minimal snapshot (baseline building phase).
    pub fn building(timestamp_ms: u64, baseline: &Baseline, events: Vec<ComplexityEvent>) -> Self {
        Self {
            version: SNAPSHOT_VERSION.to_string(),
            timestamp_ms,
            baseline: BaselineSummary::from_baseline(baseline),
            deltas: None,
            z_scores: None,
            s_lite: None,
            events,
            flags: vec!["BASELINE_BUILDING".to_string()],
        }
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serialize to pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Check if baseline is locked.
    pub fn is_baseline_locked(&self) -> bool {
        self.baseline.state == "locked"
    }

    /// Check if any events were emitted.
    pub fn has_events(&self) -> bool {
        !self.events.is_empty()
    }

    /// Get events by severity.
    pub fn events_by_severity(&self, severity: &str) -> Vec<&ComplexityEvent> {
        self.events
            .iter()
            .filter(|e| format!("{:?}", e.severity).to_lowercase() == severity.to_lowercase())
            .collect()
    }
}

impl BaselineSummary {
    /// Create summary from baseline.
    pub fn from_baseline(baseline: &Baseline) -> Self {
        let state = match baseline.state {
            BaselineState::Building => "building",
            BaselineState::Locked => "locked",
        };

        let progress = baseline.build_progress;
        let sample_count = baseline.h_bytes.count as usize;

        let stats = if baseline.is_ready() {
            Some(BaselineStats {
                tc_mean: Some(baseline.tc.mean),
                tc_std: Some(baseline.tc.std),
                h_joint_mean: Some(baseline.h_joint.mean),
                h_joint_std: Some(baseline.h_joint.std),
                h_bytes_mean: baseline.h_bytes.mean,
                h_bytes_std: baseline.h_bytes.std,
                r_mean: baseline.r.as_ref().map(|s| s.mean),
                r_std: baseline.r.as_ref().map(|s| s.std),
            })
        } else {
            None
        };

        Self {
            state: state.to_string(),
            sample_count,
            progress,
            stats,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BaselineConfig;

    fn create_locked_baseline() -> Baseline {
        let config = BaselineConfig::default();
        let mut baseline = Baseline::new(true);
        baseline.start(0);
        baseline.add_sample(Some(1.0), Some(2.0), 3.0, Some(0.5), 100, &config);
        baseline.add_sample(Some(1.1), Some(2.1), 3.1, Some(0.6), 200, &config);
        baseline.lock();
        baseline
    }

    #[test]
    fn test_snapshot_creation() {
        let baseline = create_locked_baseline();
        let snapshot = ComplexitySnapshot::new(1000, &baseline, None, None, None, vec![], vec![]);

        assert_eq!(snapshot.version, SNAPSHOT_VERSION);
        assert_eq!(snapshot.timestamp_ms, 1000);
        assert!(snapshot.is_baseline_locked());
    }

    #[test]
    fn test_snapshot_building() {
        let baseline = Baseline::new(true);

        let snapshot = ComplexitySnapshot::building(1000, &baseline, vec![]);

        assert!(!snapshot.is_baseline_locked());
        assert!(snapshot.flags.contains(&"BASELINE_BUILDING".to_string()));
    }

    #[test]
    fn test_snapshot_json_roundtrip() {
        let baseline = create_locked_baseline();
        let deltas = Deltas {
            tc: Some(0.1),
            h_joint: Some(0.2),
            h_bytes: 0.3,
            r: Some(-0.1),
        };
        let z_scores = ZScores {
            tc: Some(1.5),
            h_joint: Some(2.0),
            h_bytes: 2.5,
            r: Some(-1.0),
        };

        let snapshot = ComplexitySnapshot::new(
            1000,
            &baseline,
            Some(deltas),
            Some(z_scores),
            None,
            vec![],
            vec!["TEST_FLAG".to_string()],
        );

        let json = snapshot.to_json().unwrap();
        let restored = ComplexitySnapshot::from_json(&json).unwrap();

        assert_eq!(restored.version, snapshot.version);
        assert_eq!(restored.timestamp_ms, snapshot.timestamp_ms);
        assert!(restored.deltas.is_some());
        assert!(restored.z_scores.is_some());
        assert!(restored.flags.contains(&"TEST_FLAG".to_string()));
    }

    #[test]
    fn test_snapshot_pretty_json() {
        let baseline = create_locked_baseline();
        let snapshot = ComplexitySnapshot::new(1000, &baseline, None, None, None, vec![], vec![]);

        let pretty = snapshot.to_json_pretty().unwrap();
        assert!(pretty.contains('\n'));
        assert!(pretty.contains("  ")); // Indentation
    }

    #[test]
    fn test_baseline_summary() {
        let baseline = create_locked_baseline();
        let summary = BaselineSummary::from_baseline(&baseline);

        assert_eq!(summary.state, "locked");
        assert_eq!(summary.sample_count, 2);
        assert!((summary.progress - 1.0).abs() < 0.01);
        assert!(summary.stats.is_some());

        let stats = summary.stats.unwrap();
        assert!(stats.tc_mean.is_some());
        assert!(stats.h_bytes_mean > 0.0);
    }

    #[test]
    fn test_events_by_severity() {
        use crate::event::{ComplexityEvent, EventSeverity};

        let baseline = create_locked_baseline();
        let events = vec![
            ComplexityEvent::payload_entropy_spike(1000, EventSeverity::Warning, 2.5, 2.0),
            ComplexityEvent::complexity_surge(1000, EventSeverity::Critical, 3.5, 3.0),
            ComplexityEvent::redundancy_drop(1000, EventSeverity::Warning, -2.5, -2.0),
        ];

        let snapshot = ComplexitySnapshot::new(1000, &baseline, None, None, None, events, vec![]);

        assert!(snapshot.has_events());

        let warnings = snapshot.events_by_severity("warning");
        assert_eq!(warnings.len(), 2);

        let criticals = snapshot.events_by_severity("critical");
        assert_eq!(criticals.len(), 1);
    }

    #[test]
    fn test_snapshot_skip_serializing_none() {
        let baseline = create_locked_baseline();
        let snapshot = ComplexitySnapshot::new(1000, &baseline, None, None, None, vec![], vec![]);

        let json = snapshot.to_json().unwrap();

        // These fields should be omitted when None/empty
        assert!(!json.contains("\"deltas\""));
        assert!(!json.contains("\"z_scores\""));
        assert!(!json.contains("\"s_lite\""));
        assert!(!json.contains("\"events\""));
        assert!(!json.contains("\"flags\""));
    }
}
