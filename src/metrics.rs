//! Metrics collection for ALEC compression analysis
//!
//! This module provides statistics about compression efficiency,
//! encoding distribution, and prediction accuracy.

use crate::protocol::EncodingType;
use std::collections::HashMap;

/// Compression statistics collector
#[derive(Debug, Clone, Default)]
pub struct CompressionMetrics {
    /// Total raw bytes (before compression)
    pub raw_bytes: u64,
    /// Total encoded bytes (after compression)
    pub encoded_bytes: u64,
    /// Number of messages processed
    pub message_count: u64,
    /// Encoding type distribution
    pub encoding_distribution: HashMap<EncodingType, u64>,
    /// Prediction hits (value matched prediction)
    pub prediction_hits: u64,
    /// Prediction misses
    pub prediction_misses: u64,
}

impl CompressionMetrics {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an encoding operation
    pub fn record_encode(&mut self, raw_size: usize, encoded_size: usize, encoding: EncodingType) {
        self.raw_bytes += raw_size as u64;
        self.encoded_bytes += encoded_size as u64;
        self.message_count += 1;
        *self.encoding_distribution.entry(encoding).or_insert(0) += 1;
    }

    /// Record a prediction result
    pub fn record_prediction(&mut self, hit: bool) {
        if hit {
            self.prediction_hits += 1;
        } else {
            self.prediction_misses += 1;
        }
    }

    /// Calculate compression ratio (higher = better)
    /// Returns raw_size / encoded_size
    pub fn compression_ratio(&self) -> f64 {
        if self.encoded_bytes == 0 {
            return 1.0;
        }
        self.raw_bytes as f64 / self.encoded_bytes as f64
    }

    /// Calculate space savings percentage
    /// Returns (1 - encoded/raw) * 100
    pub fn space_savings_percent(&self) -> f64 {
        if self.raw_bytes == 0 {
            return 0.0;
        }
        (1.0 - (self.encoded_bytes as f64 / self.raw_bytes as f64)) * 100.0
    }

    /// Calculate prediction accuracy (0.0 - 1.0)
    pub fn prediction_accuracy(&self) -> f64 {
        let total = self.prediction_hits + self.prediction_misses;
        if total == 0 {
            return 0.0;
        }
        self.prediction_hits as f64 / total as f64
    }

    /// Get most used encoding type
    pub fn most_used_encoding(&self) -> Option<EncodingType> {
        self.encoding_distribution
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(encoding, _)| *encoding)
    }

    /// Get average message size in bytes
    pub fn average_message_size(&self) -> f64 {
        if self.message_count == 0 {
            return 0.0;
        }
        self.encoded_bytes as f64 / self.message_count as f64
    }

    /// Reset all metrics
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Generate a human-readable report
    pub fn report(&self) -> String {
        let mut report = String::new();

        report.push_str("=== ALEC Compression Metrics ===\n\n");

        report.push_str(&format!("Messages processed: {}\n", self.message_count));
        report.push_str(&format!("Raw bytes: {} bytes\n", self.raw_bytes));
        report.push_str(&format!("Encoded bytes: {} bytes\n", self.encoded_bytes));
        report.push_str(&format!(
            "Compression ratio: {:.2}x\n",
            self.compression_ratio()
        ));
        report.push_str(&format!(
            "Space savings: {:.1}%\n",
            self.space_savings_percent()
        ));
        report.push_str(&format!(
            "Average message size: {:.1} bytes\n\n",
            self.average_message_size()
        ));

        report.push_str("Encoding distribution:\n");
        let mut encodings: Vec<_> = self.encoding_distribution.iter().collect();
        encodings.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
        for (encoding, count) in encodings {
            let percent = if self.message_count > 0 {
                (*count as f64 / self.message_count as f64) * 100.0
            } else {
                0.0
            };
            report.push_str(&format!("  {:?}: {} ({:.1}%)\n", encoding, count, percent));
        }

        let total_predictions = self.prediction_hits + self.prediction_misses;
        if total_predictions > 0 {
            report.push_str(&format!(
                "\nPrediction accuracy: {:.1}% ({}/{})\n",
                self.prediction_accuracy() * 100.0,
                self.prediction_hits,
                total_predictions
            ));
        }

        report
    }
}

/// Context statistics
#[derive(Debug, Clone, Default)]
pub struct ContextMetrics {
    /// Number of patterns in dictionary
    pub pattern_count: usize,
    /// Total memory used by context (estimated)
    pub memory_bytes: usize,
    /// Number of sources tracked
    pub source_count: usize,
    /// Context version
    pub version: u32,
}

impl ContextMetrics {
    /// Create metrics from a Context instance
    pub fn from_context(context: &crate::context::Context) -> Self {
        Self {
            pattern_count: context.pattern_count(),
            memory_bytes: context.estimated_memory(),
            source_count: context.source_count(),
            version: context.version(),
        }
    }

    /// Generate a human-readable report
    pub fn report(&self) -> String {
        let mut report = String::new();

        report.push_str("=== Context Metrics ===\n\n");
        report.push_str(&format!("Version: {}\n", self.version));
        report.push_str(&format!("Patterns: {}\n", self.pattern_count));
        report.push_str(&format!("Sources tracked: {}\n", self.source_count));
        report.push_str(&format!(
            "Estimated memory: {} bytes ({:.1} KB)\n",
            self.memory_bytes,
            self.memory_bytes as f64 / 1024.0
        ));

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_ratio() {
        let mut metrics = CompressionMetrics::new();
        metrics.record_encode(100, 25, EncodingType::Delta8);

        assert!((metrics.compression_ratio() - 4.0).abs() < 0.01);
        assert!((metrics.space_savings_percent() - 75.0).abs() < 0.1);
    }

    #[test]
    fn test_prediction_accuracy() {
        let mut metrics = CompressionMetrics::new();
        metrics.record_prediction(true);
        metrics.record_prediction(true);
        metrics.record_prediction(false);

        assert!((metrics.prediction_accuracy() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_encoding_distribution() {
        let mut metrics = CompressionMetrics::new();
        metrics.record_encode(100, 10, EncodingType::Delta8);
        metrics.record_encode(100, 10, EncodingType::Delta8);
        metrics.record_encode(100, 1, EncodingType::Repeated);

        assert_eq!(metrics.most_used_encoding(), Some(EncodingType::Delta8));
        assert_eq!(metrics.message_count, 3);
    }

    #[test]
    fn test_report_generation() {
        let mut metrics = CompressionMetrics::new();
        metrics.record_encode(1000, 250, EncodingType::Delta8);

        let report = metrics.report();
        assert!(report.contains("Compression ratio"));
        assert!(report.contains("Delta8"));
    }

    #[test]
    fn test_empty_metrics() {
        let metrics = CompressionMetrics::new();

        assert_eq!(metrics.compression_ratio(), 1.0);
        assert_eq!(metrics.space_savings_percent(), 0.0);
        assert_eq!(metrics.prediction_accuracy(), 0.0);
        assert_eq!(metrics.most_used_encoding(), None);
    }

    #[test]
    fn test_reset() {
        let mut metrics = CompressionMetrics::new();
        metrics.record_encode(100, 25, EncodingType::Delta8);
        metrics.record_prediction(true);

        metrics.reset();

        assert_eq!(metrics.message_count, 0);
        assert_eq!(metrics.raw_bytes, 0);
        assert_eq!(metrics.prediction_hits, 0);
    }
}
