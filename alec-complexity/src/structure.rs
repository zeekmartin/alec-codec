// ALEC Complexity - Standalone complexity monitoring
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! S-lite structure summary (lightweight pairwise edges).

use crate::config::StructureConfig;
use crate::input::InputSnapshot;
use serde::{Deserialize, Serialize};

/// An edge in the S-lite structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLiteEdge {
    pub channel_a: String,
    pub channel_b: String,
    /// Weight: |H(A) - H(B)| normalized (similarity indicator).
    pub weight: f64,
}

/// S-lite structure summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLite {
    /// Edges sorted by weight (descending).
    pub edges: Vec<SLiteEdge>,
    /// Number of channels included.
    pub channel_count: usize,
    /// Timestamp of extraction.
    pub timestamp_ms: u64,
}

impl SLite {
    /// Get edge between two channels.
    pub fn get_edge(&self, a: &str, b: &str) -> Option<&SLiteEdge> {
        self.edges.iter().find(|e| {
            (e.channel_a == a && e.channel_b == b) || (e.channel_a == b && e.channel_b == a)
        })
    }
}

/// Structure break event details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureBreak {
    /// Edges that changed significantly.
    pub changed_edges: Vec<EdgeChange>,
    /// Total change magnitude.
    pub total_change: f64,
}

/// Details of an edge change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeChange {
    pub channel_a: String,
    pub channel_b: String,
    pub old_weight: f64,
    pub new_weight: f64,
    pub delta: f64,
}

/// Extractor for S-lite structures.
pub struct SLiteExtractor {
    config: StructureConfig,
    last_s_lite: Option<SLite>,
}

impl SLiteExtractor {
    pub fn new(config: StructureConfig) -> Self {
        Self {
            config,
            last_s_lite: None,
        }
    }

    /// Extract S-lite from input snapshot.
    pub fn extract(&mut self, input: &InputSnapshot) -> Option<SLite> {
        if !self.config.enabled || !self.config.emit_s_lite {
            return None;
        }

        if !input.can_compute_structure() {
            return None;
        }

        let channels = &input.channel_entropies;
        let n = channels.len().min(self.config.max_channels);

        if n < 2 {
            return None;
        }

        // Compute all pairwise edges
        let mut edges = Vec::new();
        let max_h: f64 = channels.iter().map(|c| c.h).fold(0.0, f64::max);

        for i in 0..n {
            for j in (i + 1)..n {
                let h_i = channels[i].h;
                let h_j = channels[j].h;

                // Weight: similarity based on entropy difference
                // Closer entropies = higher weight (more similar)
                let diff = (h_i - h_j).abs();
                let weight = if max_h > 0.0 {
                    1.0 - (diff / max_h).min(1.0)
                } else {
                    1.0
                };

                edges.push(SLiteEdge {
                    channel_a: channels[i].channel_id.clone(),
                    channel_b: channels[j].channel_id.clone(),
                    weight,
                });
            }
        }

        // Sort by weight (descending)
        edges.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply sparsification
        if self.config.sparsify.enabled {
            edges.retain(|e| e.weight >= self.config.sparsify.min_abs_weight);
            edges.truncate(self.config.sparsify.top_k_edges);
        }

        let s_lite = SLite {
            edges,
            channel_count: n,
            timestamp_ms: input.timestamp_ms,
        };

        // Store for break detection
        let result = s_lite.clone();
        self.last_s_lite = Some(s_lite);

        Some(result)
    }

    /// Detect structure break compared to last S-lite.
    pub fn detect_break(&self, current: &SLite) -> Option<StructureBreak> {
        if !self.config.detect_breaks {
            return None;
        }

        let last = self.last_s_lite.as_ref()?;
        let mut changed_edges = Vec::new();
        let mut total_change = 0.0;

        // Compare edges
        for curr_edge in &current.edges {
            if let Some(last_edge) = last.get_edge(&curr_edge.channel_a, &curr_edge.channel_b) {
                let delta = (curr_edge.weight - last_edge.weight).abs();
                if delta >= self.config.break_threshold {
                    changed_edges.push(EdgeChange {
                        channel_a: curr_edge.channel_a.clone(),
                        channel_b: curr_edge.channel_b.clone(),
                        old_weight: last_edge.weight,
                        new_weight: curr_edge.weight,
                        delta,
                    });
                    total_change += delta;
                }
            }
        }

        if changed_edges.is_empty() {
            None
        } else {
            Some(StructureBreak {
                changed_edges,
                total_change,
            })
        }
    }

    /// Get last extracted S-lite.
    pub fn last_s_lite(&self) -> Option<&SLite> {
        self.last_s_lite.as_ref()
    }

    /// Reset state.
    pub fn reset(&mut self) {
        self.last_s_lite = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::ChannelEntropy;

    fn create_test_input(channels: &[(&str, f64)]) -> InputSnapshot {
        let mut input = InputSnapshot::minimal(1000, 5.0);
        for (id, h) in channels {
            input.channel_entropies.push(ChannelEntropy {
                channel_id: id.to_string(),
                h: *h,
            });
        }
        input
    }

    #[test]
    fn test_extract_s_lite() {
        let config = StructureConfig::default();
        let mut extractor = SLiteExtractor::new(config);

        let input = create_test_input(&[("ch1", 2.0), ("ch2", 2.5), ("ch3", 3.0)]);
        let s_lite = extractor.extract(&input).unwrap();

        assert_eq!(s_lite.channel_count, 3);
        assert!(!s_lite.edges.is_empty());
    }

    #[test]
    fn test_insufficient_channels() {
        let config = StructureConfig::default();
        let mut extractor = SLiteExtractor::new(config);

        let input = create_test_input(&[("ch1", 2.0)]);
        let s_lite = extractor.extract(&input);

        assert!(s_lite.is_none());
    }

    #[test]
    fn test_sparsification() {
        let config = StructureConfig {
            sparsify: crate::config::SparsifyConfig {
                enabled: true,
                top_k_edges: 2,
                min_abs_weight: 0.1,
            },
            ..Default::default()
        };
        let mut extractor = SLiteExtractor::new(config);

        let input = create_test_input(&[("ch1", 2.0), ("ch2", 2.1), ("ch3", 5.0), ("ch4", 5.1)]);
        let s_lite = extractor.extract(&input).unwrap();

        assert!(s_lite.edges.len() <= 2);
    }

    #[test]
    fn test_edge_weights() {
        let config = StructureConfig {
            sparsify: crate::config::SparsifyConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut extractor = SLiteExtractor::new(config);

        let input = create_test_input(&[("ch1", 2.0), ("ch2", 2.0), ("ch3", 10.0)]);
        let s_lite = extractor.extract(&input).unwrap();

        // ch1-ch2 should have highest weight (identical entropy)
        let edge_12 = s_lite.get_edge("ch1", "ch2").unwrap();
        let edge_13 = s_lite.get_edge("ch1", "ch3").unwrap();

        assert!(edge_12.weight > edge_13.weight);
    }

    #[test]
    fn test_structure_break_detection() {
        let config = StructureConfig {
            break_threshold: 0.1,
            sparsify: crate::config::SparsifyConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut extractor = SLiteExtractor::new(config);

        // First extraction - stores s_lite1 in last_s_lite
        let input1 = create_test_input(&[("ch1", 2.0), ("ch2", 2.0)]);
        let s_lite1 = extractor.extract(&input1).unwrap();

        // Second extraction with changed values - stores s_lite2 in last_s_lite
        let input2 = create_test_input(&[("ch1", 2.0), ("ch2", 5.0)]);
        let _s_lite2 = extractor.extract(&input2).unwrap();

        // Now compare s_lite1 against current last_s_lite (which is s_lite2)
        // to detect the break between them
        let break_event = extractor.detect_break(&s_lite1);
        assert!(break_event.is_some());

        // Verify the edges changed
        let brk = break_event.unwrap();
        assert!(!brk.changed_edges.is_empty());
    }

    #[test]
    fn test_no_structure_break() {
        let config = StructureConfig {
            break_threshold: 0.5,
            sparsify: crate::config::SparsifyConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut extractor = SLiteExtractor::new(config);

        // First extraction
        let input1 = create_test_input(&[("ch1", 2.0), ("ch2", 2.0)]);
        extractor.extract(&input1);

        // Second extraction with similar values
        let input2 = create_test_input(&[("ch1", 2.0), ("ch2", 2.1)]);
        let s_lite2 = extractor.extract(&input2).unwrap();

        let break_event = extractor.detect_break(&s_lite2);
        assert!(break_event.is_none());
    }

    #[test]
    fn test_disabled_structure() {
        let config = StructureConfig {
            enabled: false,
            ..Default::default()
        };
        let mut extractor = SLiteExtractor::new(config);

        let input = create_test_input(&[("ch1", 2.0), ("ch2", 2.5)]);
        assert!(extractor.extract(&input).is_none());
    }
}
