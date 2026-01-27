// ALEC Complexity - Standalone complexity monitoring
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Complexity event types and definitions.

use crate::structure::StructureBreak;
use serde::{Deserialize, Serialize};

/// Type of complexity event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    /// Baseline is being built.
    BaselineBuilding,
    /// Baseline is locked and ready.
    BaselineLocked,
    /// Payload entropy spike detected.
    PayloadEntropySpike,
    /// Structure break detected (S-lite edges changed).
    StructureBreak,
    /// Redundancy (R) dropped below threshold.
    RedundancyDrop,
    /// Complexity surge (TC or H_joint persists high).
    ComplexitySurge,
    /// Criticality ranking changed significantly.
    CriticalityShift,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::BaselineBuilding => "BASELINE_BUILDING",
            EventType::BaselineLocked => "BASELINE_LOCKED",
            EventType::PayloadEntropySpike => "PAYLOAD_ENTROPY_SPIKE",
            EventType::StructureBreak => "STRUCTURE_BREAK",
            EventType::RedundancyDrop => "REDUNDANCY_DROP",
            EventType::ComplexitySurge => "COMPLEXITY_SURGE",
            EventType::CriticalityShift => "CRITICALITY_SHIFT",
        }
    }
}

/// Severity level of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSeverity {
    Info,
    Warning,
    Critical,
}

impl EventSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventSeverity::Info => "INFO",
            EventSeverity::Warning => "WARN",
            EventSeverity::Critical => "CRIT",
        }
    }
}

/// Additional details for specific event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventDetails {
    /// Baseline building progress.
    BaselineProgress { progress: f64 },
    /// Z-score that triggered the event.
    ZScore { value: f64, threshold: f64 },
    /// Structure break details.
    Structure(StructureBreak),
    /// Criticality shift details.
    CriticalityRanking {
        old_top: Vec<String>,
        new_top: Vec<String>,
    },
    /// No additional details.
    None,
}

/// A complexity event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityEvent {
    /// Event type.
    pub event_type: EventType,
    /// Severity level.
    pub severity: EventSeverity,
    /// Timestamp when event was detected (ms).
    pub timestamp_ms: u64,
    /// Human-readable message.
    pub message: String,
    /// Additional details.
    pub details: EventDetails,
}

impl ComplexityEvent {
    /// Create a new event.
    pub fn new(
        event_type: EventType,
        severity: EventSeverity,
        timestamp_ms: u64,
        message: impl Into<String>,
        details: EventDetails,
    ) -> Self {
        Self {
            event_type,
            severity,
            timestamp_ms,
            message: message.into(),
            details,
        }
    }

    /// Create a baseline building event.
    pub fn baseline_building(timestamp_ms: u64, progress: f64) -> Self {
        Self::new(
            EventType::BaselineBuilding,
            EventSeverity::Info,
            timestamp_ms,
            format!("Baseline building: {:.0}% complete", progress * 100.0),
            EventDetails::BaselineProgress { progress },
        )
    }

    /// Create a baseline locked event.
    pub fn baseline_locked(timestamp_ms: u64) -> Self {
        Self::new(
            EventType::BaselineLocked,
            EventSeverity::Info,
            timestamp_ms,
            "Baseline locked and ready",
            EventDetails::None,
        )
    }

    /// Create a payload entropy spike event.
    pub fn payload_entropy_spike(
        timestamp_ms: u64,
        severity: EventSeverity,
        z_score: f64,
        threshold: f64,
    ) -> Self {
        Self::new(
            EventType::PayloadEntropySpike,
            severity,
            timestamp_ms,
            format!(
                "Payload entropy spike: z={:.2} (threshold: {:.2})",
                z_score, threshold
            ),
            EventDetails::ZScore {
                value: z_score,
                threshold,
            },
        )
    }

    /// Create a structure break event.
    pub fn structure_break(timestamp_ms: u64, break_info: StructureBreak) -> Self {
        let edge_count = break_info.changed_edges.len();
        Self::new(
            EventType::StructureBreak,
            EventSeverity::Warning,
            timestamp_ms,
            format!("Structure break detected: {} edges changed", edge_count),
            EventDetails::Structure(break_info),
        )
    }

    /// Create a redundancy drop event.
    pub fn redundancy_drop(
        timestamp_ms: u64,
        severity: EventSeverity,
        z_score: f64,
        threshold: f64,
    ) -> Self {
        Self::new(
            EventType::RedundancyDrop,
            severity,
            timestamp_ms,
            format!(
                "Redundancy (R) dropped: z={:.2} (threshold: {:.2})",
                z_score, threshold
            ),
            EventDetails::ZScore {
                value: z_score,
                threshold,
            },
        )
    }

    /// Create a complexity surge event.
    pub fn complexity_surge(
        timestamp_ms: u64,
        severity: EventSeverity,
        z_score: f64,
        threshold: f64,
    ) -> Self {
        Self::new(
            EventType::ComplexitySurge,
            severity,
            timestamp_ms,
            format!(
                "Complexity surge detected: z={:.2} (threshold: {:.2})",
                z_score, threshold
            ),
            EventDetails::ZScore {
                value: z_score,
                threshold,
            },
        )
    }

    /// Create a criticality shift event.
    pub fn criticality_shift(
        timestamp_ms: u64,
        old_top: Vec<String>,
        new_top: Vec<String>,
    ) -> Self {
        Self::new(
            EventType::CriticalityShift,
            EventSeverity::Warning,
            timestamp_ms,
            "Channel criticality ranking changed",
            EventDetails::CriticalityRanking { old_top, new_top },
        )
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serialize to pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_as_str() {
        assert_eq!(EventType::BaselineBuilding.as_str(), "BASELINE_BUILDING");
        assert_eq!(
            EventType::PayloadEntropySpike.as_str(),
            "PAYLOAD_ENTROPY_SPIKE"
        );
    }

    #[test]
    fn test_severity_as_str() {
        assert_eq!(EventSeverity::Info.as_str(), "INFO");
        assert_eq!(EventSeverity::Warning.as_str(), "WARN");
        assert_eq!(EventSeverity::Critical.as_str(), "CRIT");
    }

    #[test]
    fn test_baseline_building_event() {
        let event = ComplexityEvent::baseline_building(1000, 0.5);
        assert_eq!(event.event_type, EventType::BaselineBuilding);
        assert_eq!(event.severity, EventSeverity::Info);
        assert!(event.message.contains("50%"));
    }

    #[test]
    fn test_baseline_locked_event() {
        let event = ComplexityEvent::baseline_locked(1000);
        assert_eq!(event.event_type, EventType::BaselineLocked);
    }

    #[test]
    fn test_payload_entropy_spike_event() {
        let event = ComplexityEvent::payload_entropy_spike(1000, EventSeverity::Warning, 2.5, 2.0);
        assert_eq!(event.event_type, EventType::PayloadEntropySpike);
        match event.details {
            EventDetails::ZScore { value, threshold } => {
                assert!((value - 2.5).abs() < 0.001);
                assert!((threshold - 2.0).abs() < 0.001);
            }
            _ => panic!("Wrong details type"),
        }
    }

    #[test]
    fn test_event_json_serialization() {
        let event = ComplexityEvent::baseline_locked(1000);
        let json = event.to_json().unwrap();
        // Serde serializes enum as "BaselineLocked" (Pascal case)
        assert!(json.contains("BaselineLocked"));
        assert!(json.contains("1000"));
    }

    #[test]
    fn test_redundancy_drop_event() {
        let event = ComplexityEvent::redundancy_drop(1000, EventSeverity::Critical, -3.0, -2.0);
        assert_eq!(event.event_type, EventType::RedundancyDrop);
        assert_eq!(event.severity, EventSeverity::Critical);
    }

    #[test]
    fn test_criticality_shift_event() {
        let event = ComplexityEvent::criticality_shift(
            1000,
            vec!["ch1".to_string(), "ch2".to_string()],
            vec!["ch3".to_string(), "ch1".to_string()],
        );
        assert_eq!(event.event_type, EventType::CriticalityShift);
        match event.details {
            EventDetails::CriticalityRanking { old_top, new_top } => {
                assert_eq!(old_top.len(), 2);
                assert_eq!(new_top.len(), 2);
            }
            _ => panic!("Wrong details type"),
        }
    }
}
