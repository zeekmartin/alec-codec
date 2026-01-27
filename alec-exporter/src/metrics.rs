// ALEC Exporter - Prometheus metrics definitions
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Prometheus metrics for ALEC monitoring.
//!
//! This module defines all Prometheus metrics exposed by the exporter
//! and provides functions to update them from ALEC snapshots.

use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge, register_gauge_vec, CounterVec, Encoder, Gauge, GaugeVec,
    TextEncoder,
};

lazy_static! {
    // ============================================================
    // Core ALEC Metrics (from MetricsSnapshot)
    // ============================================================

    /// Resilience Index (R) - measures system redundancy.
    /// Range: 0.0 to 1.0 (higher = more resilient)
    pub static ref RESILIENCE_INDEX: Gauge = register_gauge!(
        "alec_resilience_index",
        "ALEC Resilience Index (R) measuring system redundancy (0-1)"
    ).unwrap();

    /// Resilience Zone - categorical health indicator.
    /// Values: 0 = Healthy, 1 = Warning, 2 = Critical
    pub static ref RESILIENCE_ZONE: Gauge = register_gauge!(
        "alec_resilience_zone",
        "ALEC Resilience Zone (0=Healthy, 1=Warning, 2=Critical)"
    ).unwrap();

    /// Total Correlation (TC) - measures inter-channel dependencies.
    /// Higher values indicate more structure/correlation.
    pub static ref TOTAL_CORRELATION_BITS: Gauge = register_gauge!(
        "alec_total_correlation_bits",
        "ALEC Total Correlation in bits"
    ).unwrap();

    /// Joint Entropy (H_joint) - combined entropy of all channels.
    pub static ref JOINT_ENTROPY_BITS: Gauge = register_gauge!(
        "alec_joint_entropy_bits",
        "ALEC Joint Entropy in bits"
    ).unwrap();

    /// Payload Entropy (H_bytes) - entropy of raw payload bytes.
    pub static ref PAYLOAD_ENTROPY_BITS: Gauge = register_gauge!(
        "alec_payload_entropy_bits",
        "ALEC Payload Entropy (H_bytes) in bits"
    ).unwrap();

    /// Sum of individual channel entropies.
    pub static ref SUM_ENTROPY_BITS: Gauge = register_gauge!(
        "alec_sum_entropy_bits",
        "Sum of individual channel entropies in bits"
    ).unwrap();

    /// Per-channel entropy (labeled by channel ID).
    pub static ref CHANNEL_ENTROPY_BITS: GaugeVec = register_gauge_vec!(
        "alec_channel_entropy_bits",
        "Per-channel entropy in bits",
        &["channel"]
    ).unwrap();

    /// Per-channel criticality ranking (labeled by channel ID).
    /// Lower rank = more critical (rank 1 is most critical).
    pub static ref CHANNEL_CRITICALITY: GaugeVec = register_gauge_vec!(
        "alec_channel_criticality",
        "Per-channel criticality ranking (lower = more critical)",
        &["channel"]
    ).unwrap();

    // ============================================================
    // Complexity Metrics (from ComplexitySnapshot)
    // ============================================================

    /// Baseline learning progress (0.0 to 1.0).
    pub static ref BASELINE_PROGRESS: Gauge = register_gauge!(
        "alec_baseline_progress",
        "ALEC baseline learning progress (0-1)"
    ).unwrap();

    /// Whether baseline is locked (1 = locked, 0 = learning).
    pub static ref BASELINE_LOCKED: Gauge = register_gauge!(
        "alec_baseline_locked",
        "ALEC baseline locked state (1=locked, 0=learning)"
    ).unwrap();

    /// Z-score for Total Correlation deviation from baseline.
    pub static ref ZSCORE_TC: Gauge = register_gauge!(
        "alec_zscore_tc",
        "Z-score for Total Correlation deviation"
    ).unwrap();

    /// Z-score for Joint Entropy deviation from baseline.
    pub static ref ZSCORE_H_JOINT: Gauge = register_gauge!(
        "alec_zscore_h_joint",
        "Z-score for Joint Entropy deviation"
    ).unwrap();

    /// Z-score for Payload Entropy deviation from baseline.
    pub static ref ZSCORE_H_BYTES: Gauge = register_gauge!(
        "alec_zscore_h_bytes",
        "Z-score for Payload Entropy deviation"
    ).unwrap();

    /// Z-score for Resilience Index deviation from baseline.
    pub static ref ZSCORE_R: Gauge = register_gauge!(
        "alec_zscore_r",
        "Z-score for Resilience Index deviation"
    ).unwrap();

    /// Delta from baseline for Total Correlation.
    pub static ref DELTA_TC: Gauge = register_gauge!(
        "alec_delta_tc",
        "Delta from baseline for Total Correlation"
    ).unwrap();

    /// Delta from baseline for Joint Entropy.
    pub static ref DELTA_H_JOINT: Gauge = register_gauge!(
        "alec_delta_h_joint",
        "Delta from baseline for Joint Entropy"
    ).unwrap();

    /// Delta from baseline for Payload Entropy.
    pub static ref DELTA_H_BYTES: Gauge = register_gauge!(
        "alec_delta_h_bytes",
        "Delta from baseline for Payload Entropy"
    ).unwrap();

    /// Delta from baseline for Resilience Index.
    pub static ref DELTA_R: Gauge = register_gauge!(
        "alec_delta_r",
        "Delta from baseline for Resilience Index"
    ).unwrap();

    // ============================================================
    // Event Counters
    // ============================================================

    /// Total anomaly events detected (labeled by event type and severity).
    pub static ref ANOMALY_EVENTS_TOTAL: CounterVec = register_counter_vec!(
        "alec_anomaly_events_total",
        "Total anomaly events detected",
        &["event_type", "severity"]
    ).unwrap();

    // ============================================================
    // Exporter Metrics
    // ============================================================

    /// Total samples processed by the exporter.
    pub static ref SAMPLES_PROCESSED_TOTAL: Gauge = register_gauge!(
        "alec_exporter_samples_total",
        "Total samples processed by the exporter"
    ).unwrap();

    /// Current replay position (sample index).
    pub static ref REPLAY_POSITION: Gauge = register_gauge!(
        "alec_exporter_replay_position",
        "Current replay position (sample index)"
    ).unwrap();

    /// Total samples in the replay dataset.
    pub static ref REPLAY_TOTAL_SAMPLES: Gauge = register_gauge!(
        "alec_exporter_replay_total_samples",
        "Total samples in the replay dataset"
    ).unwrap();

    /// Replay speed multiplier.
    pub static ref REPLAY_SPEED: Gauge = register_gauge!(
        "alec_exporter_replay_speed",
        "Replay speed multiplier"
    ).unwrap();
}

/// Resilience zone categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResilienceZone {
    Healthy = 0,
    Warning = 1,
    Critical = 2,
}

impl From<&str> for ResilienceZone {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "healthy" | "green" => ResilienceZone::Healthy,
            "warning" | "yellow" => ResilienceZone::Warning,
            "critical" | "red" => ResilienceZone::Critical,
            _ => ResilienceZone::Healthy,
        }
    }
}

/// Event severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Critical => "critical",
        }
    }
}

/// Update core metrics from gateway MetricsSnapshot values.
pub fn update_core_metrics(r: f64, zone: &str, tc: f64, h_joint: f64, h_bytes: f64, sum_h: f64) {
    RESILIENCE_INDEX.set(r);
    RESILIENCE_ZONE.set(ResilienceZone::from(zone) as i64 as f64);
    TOTAL_CORRELATION_BITS.set(tc);
    JOINT_ENTROPY_BITS.set(h_joint);
    PAYLOAD_ENTROPY_BITS.set(h_bytes);
    SUM_ENTROPY_BITS.set(sum_h);
}

/// Update per-channel entropy metrics.
pub fn update_channel_entropy(channel_id: &str, entropy: f64) {
    CHANNEL_ENTROPY_BITS
        .with_label_values(&[channel_id])
        .set(entropy);
}

/// Update per-channel criticality ranking.
pub fn update_channel_criticality(channel_id: &str, rank: u32) {
    CHANNEL_CRITICALITY
        .with_label_values(&[channel_id])
        .set(rank as f64);
}

/// Update baseline learning metrics.
pub fn update_baseline_metrics(progress: f64, locked: bool) {
    BASELINE_PROGRESS.set(progress);
    BASELINE_LOCKED.set(if locked { 1.0 } else { 0.0 });
}

/// Update z-score metrics.
pub fn update_zscore_metrics(z_tc: f64, z_h_joint: f64, z_h_bytes: f64, z_r: f64) {
    ZSCORE_TC.set(z_tc);
    ZSCORE_H_JOINT.set(z_h_joint);
    ZSCORE_H_BYTES.set(z_h_bytes);
    ZSCORE_R.set(z_r);
}

/// Update delta metrics.
pub fn update_delta_metrics(d_tc: f64, d_h_joint: f64, d_h_bytes: f64, d_r: f64) {
    DELTA_TC.set(d_tc);
    DELTA_H_JOINT.set(d_h_joint);
    DELTA_H_BYTES.set(d_h_bytes);
    DELTA_R.set(d_r);
}

/// Increment anomaly event counter.
pub fn record_anomaly_event(event_type: &str, severity: Severity) {
    ANOMALY_EVENTS_TOTAL
        .with_label_values(&[event_type, severity.as_str()])
        .inc();
}

/// Update replay position metrics.
pub fn update_replay_metrics(position: usize, total: usize, speed: f64) {
    REPLAY_POSITION.set(position as f64);
    REPLAY_TOTAL_SAMPLES.set(total as f64);
    REPLAY_SPEED.set(speed);
}

/// Increment samples processed counter.
pub fn increment_samples_processed() {
    SAMPLES_PROCESSED_TOTAL.set(SAMPLES_PROCESSED_TOTAL.get() + 1.0);
}

/// Encode all metrics to Prometheus text format.
pub fn encode_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resilience_zone_conversion() {
        assert_eq!(ResilienceZone::from("healthy"), ResilienceZone::Healthy);
        assert_eq!(ResilienceZone::from("warning"), ResilienceZone::Warning);
        assert_eq!(ResilienceZone::from("critical"), ResilienceZone::Critical);
        assert_eq!(ResilienceZone::from("green"), ResilienceZone::Healthy);
        assert_eq!(ResilienceZone::from("yellow"), ResilienceZone::Warning);
        assert_eq!(ResilienceZone::from("red"), ResilienceZone::Critical);
    }

    #[test]
    fn test_severity_as_str() {
        assert_eq!(Severity::Info.as_str(), "info");
        assert_eq!(Severity::Warning.as_str(), "warning");
        assert_eq!(Severity::Critical.as_str(), "critical");
    }

    #[test]
    fn test_encode_metrics() {
        // Update some metrics
        update_core_metrics(0.85, "healthy", 2.5, 10.0, 8.0, 12.5);
        update_baseline_metrics(1.0, true);

        let output = encode_metrics();
        assert!(output.contains("alec_resilience_index"));
        assert!(output.contains("alec_baseline_locked"));
    }
}
