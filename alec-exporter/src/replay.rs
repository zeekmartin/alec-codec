// ALEC Exporter - Dataset replay engine
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Dataset replay engine for simulating ALEC metrics from CSV datasets.
//!
//! This module provides functionality to replay pre-generated datasets
//! through the ALEC metrics pipeline, updating Prometheus metrics in real-time.

use crate::metrics::{
    increment_samples_processed, record_anomaly_event, update_baseline_metrics,
    update_channel_criticality, update_channel_entropy, update_core_metrics, update_delta_metrics,
    update_replay_metrics, update_zscore_metrics, Severity,
};
use alec_complexity::{
    ChannelEntropy as ComplexityChannelEntropy, ComplexityConfig, ComplexityEngine,
    ComplexitySnapshot, EventSeverity, InputSnapshot,
};
use alec_gateway::{ChannelConfig, Gateway, GatewayConfig, MetricsConfig, MetricsEngine};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Configuration for dataset replay.
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    /// Path to CSV dataset file.
    pub csv_path: String,
    /// Replay speed multiplier (1.0 = real-time, 10.0 = 10x faster).
    pub speed: f64,
    /// Whether to loop the dataset.
    pub loop_replay: bool,
    /// Sample interval in milliseconds (used if dataset doesn't specify).
    pub default_sample_interval_ms: u64,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            csv_path: String::new(),
            speed: 1.0,
            loop_replay: true,
            default_sample_interval_ms: 60_000, // 1 minute
        }
    }
}

/// State of the replay engine.
#[derive(Debug)]
pub struct ReplayState {
    /// Current position in the dataset (sample index).
    pub position: AtomicUsize,
    /// Total samples in the dataset.
    pub total_samples: AtomicUsize,
    /// Whether replay is running.
    pub running: AtomicBool,
    /// Whether replay is paused.
    pub paused: AtomicBool,
}

impl Default for ReplayState {
    fn default() -> Self {
        Self {
            position: AtomicUsize::new(0),
            total_samples: AtomicUsize::new(0),
            running: AtomicBool::new(false),
            paused: AtomicBool::new(false),
        }
    }
}

/// Dataset row for replay.
#[derive(Debug, Clone)]
struct DataRow {
    timestamp_ms: u64,
    values: HashMap<String, Option<f64>>,
}

/// Replay engine that feeds datasets through ALEC and updates Prometheus metrics.
pub struct ReplayEngine {
    config: ReplayConfig,
    state: Arc<ReplayState>,
    gateway: Arc<RwLock<Gateway>>,
    metrics_engine: Arc<RwLock<MetricsEngine>>,
    complexity_engine: Arc<RwLock<ComplexityEngine>>,
    sensor_ids: Vec<String>,
    rows: Vec<DataRow>,
}

impl ReplayEngine {
    /// Create a new replay engine from a CSV file.
    pub fn from_csv(config: ReplayConfig) -> Result<Self, ReplayError> {
        let path = Path::new(&config.csv_path);
        if !path.exists() {
            return Err(ReplayError::FileNotFound(config.csv_path.clone()));
        }

        // Parse CSV
        let (sensor_ids, rows) = Self::parse_csv(path)?;

        if rows.is_empty() {
            return Err(ReplayError::EmptyDataset);
        }

        // Calculate sample interval from dataset
        let sample_interval_ms = if rows.len() >= 2 {
            (rows[1].timestamp_ms - rows[0].timestamp_ms).max(1)
        } else {
            config.default_sample_interval_ms
        };

        // Create gateway with channels
        let gateway_config = GatewayConfig {
            max_frame_size: 242,
            ..Default::default()
        };
        let mut gateway = Gateway::with_config(gateway_config);

        for sensor_id in &sensor_ids {
            let channel_config = ChannelConfig::default();
            gateway.add_channel(sensor_id, channel_config).ok();
        }

        // Create metrics engine
        let metrics_config = MetricsConfig {
            enabled: true,
            ..Default::default()
        };
        let metrics_engine = MetricsEngine::new(metrics_config);

        // Create complexity engine
        let complexity_config = ComplexityConfig {
            enabled: true,
            ..Default::default()
        };
        let complexity_engine = ComplexityEngine::new(complexity_config);

        let state = Arc::new(ReplayState::default());
        state.total_samples.store(rows.len(), Ordering::SeqCst);

        info!(
            "Loaded dataset: {} sensors, {} samples, {}ms interval",
            sensor_ids.len(),
            rows.len(),
            sample_interval_ms
        );

        Ok(Self {
            config,
            state,
            gateway: Arc::new(RwLock::new(gateway)),
            metrics_engine: Arc::new(RwLock::new(metrics_engine)),
            complexity_engine: Arc::new(RwLock::new(complexity_engine)),
            sensor_ids,
            rows,
        })
    }

    /// Parse a CSV file into sensor IDs and data rows.
    fn parse_csv(path: &Path) -> Result<(Vec<String>, Vec<DataRow>), ReplayError> {
        let mut reader = csv::Reader::from_path(path)?;

        // Parse headers
        let headers = reader.headers()?.clone();
        let header_strs: Vec<&str> = headers.iter().collect();

        if header_strs.is_empty() || header_strs[0] != "timestamp_ms" {
            return Err(ReplayError::InvalidFormat(
                "First column must be 'timestamp_ms'".to_string(),
            ));
        }

        let sensor_ids: Vec<String> = header_strs[1..].iter().map(|s| s.to_string()).collect();

        // Parse rows
        let mut rows = Vec::new();
        for result in reader.records() {
            let record = result?;
            let values: Vec<&str> = record.iter().collect();

            if values.is_empty() {
                continue;
            }

            let timestamp_ms: u64 = values[0]
                .parse()
                .map_err(|_| ReplayError::InvalidFormat("Invalid timestamp".to_string()))?;

            let mut row_values = HashMap::new();
            for (i, sensor_id) in sensor_ids.iter().enumerate() {
                let value = if i + 1 < values.len() {
                    let s = values[i + 1].trim();
                    if s.is_empty() {
                        None
                    } else {
                        s.parse().ok()
                    }
                } else {
                    None
                };
                row_values.insert(sensor_id.clone(), value);
            }

            rows.push(DataRow {
                timestamp_ms,
                values: row_values,
            });
        }

        Ok((sensor_ids, rows))
    }

    /// Get the replay state.
    pub fn state(&self) -> Arc<ReplayState> {
        Arc::clone(&self.state)
    }

    /// Start the replay loop (runs until stopped).
    pub async fn run(&self) {
        self.state.running.store(true, Ordering::SeqCst);
        info!(
            "Starting replay: speed={}, loop={}",
            self.config.speed, self.config.loop_replay
        );

        loop {
            // Check if we should stop
            if !self.state.running.load(Ordering::SeqCst) {
                break;
            }

            // Check if paused
            if self.state.paused.load(Ordering::SeqCst) {
                sleep(Duration::from_millis(100)).await;
                continue;
            }

            // Get current position
            let position = self.state.position.load(Ordering::SeqCst);

            // Check if we've reached the end
            if position >= self.rows.len() {
                if self.config.loop_replay {
                    info!("Dataset complete, looping...");
                    self.state.position.store(0, Ordering::SeqCst);
                    // Reset gateway and engines for clean loop
                    self.reset_engines().await;
                    continue;
                } else {
                    info!("Dataset complete, stopping");
                    self.state.running.store(false, Ordering::SeqCst);
                    break;
                }
            }

            // Process current row
            let row = &self.rows[position];
            self.process_row(row, position).await;

            // Update position
            self.state.position.fetch_add(1, Ordering::SeqCst);

            // Update replay metrics
            update_replay_metrics(position + 1, self.rows.len(), self.config.speed);

            // Calculate sleep duration
            let base_interval_ms = if position + 1 < self.rows.len() {
                self.rows[position + 1].timestamp_ms - row.timestamp_ms
            } else {
                self.config.default_sample_interval_ms
            };

            let sleep_ms = (base_interval_ms as f64 / self.config.speed) as u64;
            if sleep_ms > 0 {
                sleep(Duration::from_millis(sleep_ms)).await;
            }
        }
    }

    /// Process a single data row.
    async fn process_row(&self, row: &DataRow, position: usize) {
        debug!(
            "Processing sample {} at timestamp {}",
            position, row.timestamp_ms
        );

        // Push values to gateway
        {
            let mut gateway = self.gateway.write().await;
            for (sensor_id, value) in &row.values {
                if let Some(v) = value {
                    if let Err(e) = gateway.push(sensor_id, *v, row.timestamp_ms) {
                        warn!("Failed to push to channel {}: {}", sensor_id, e);
                    }
                }
            }
        }

        // Observe samples in metrics engine
        {
            let mut metrics_engine = self.metrics_engine.write().await;
            for (sensor_id, value) in &row.values {
                if let Some(v) = value {
                    metrics_engine.observe_sample(sensor_id, *v, row.timestamp_ms);
                }
            }
        }

        // Create a synthetic frame for metrics computation
        // (in real usage, this would come from gateway.flush())
        let frame_bytes: Vec<u8> = row
            .values
            .values()
            .filter_map(|v| v.map(|x| (x * 100.0) as u8))
            .collect();

        // Get metrics snapshot
        let metrics_snapshot = {
            let mut metrics_engine = self.metrics_engine.write().await;
            metrics_engine.observe_frame(&frame_bytes, row.timestamp_ms)
        };

        // Update Prometheus metrics from snapshot
        if let Some(ref snapshot) = metrics_snapshot {
            // Update core metrics from signal
            let tc = if snapshot.signal.valid {
                snapshot.signal.total_corr
            } else {
                0.0
            };
            let h_joint = if snapshot.signal.valid {
                snapshot.signal.h_joint
            } else {
                0.0
            };
            let sum_h = if snapshot.signal.valid {
                snapshot.signal.sum_h
            } else {
                0.0
            };

            // Get resilience values
            let (r, zone_str) = if let Some(ref resilience) = snapshot.resilience {
                (
                    resilience.r.unwrap_or(0.0),
                    resilience.zone.as_deref().unwrap_or("healthy"),
                )
            } else {
                (0.0, "healthy")
            };

            update_core_metrics(r, zone_str, tc, h_joint, snapshot.payload.h_bytes, sum_h);

            // Update per-channel metrics
            if snapshot.signal.valid {
                for channel in &snapshot.signal.h_per_channel {
                    update_channel_entropy(&channel.channel_id, channel.h);
                }
            }

            // Update criticality rankings
            if let Some(ref resilience) = snapshot.resilience {
                if let Some(ref criticality) = resilience.criticality {
                    for (rank, item) in criticality.ranking.iter().enumerate() {
                        update_channel_criticality(&item.channel_id, rank as u32 + 1);
                    }
                }
            }
        }

        // Process through complexity engine
        if let Some(ref metrics) = metrics_snapshot {
            let input = self.create_input_snapshot(metrics, row.timestamp_ms);
            let complexity_snapshot = {
                let mut complexity_engine = self.complexity_engine.write().await;
                complexity_engine.process(&input)
            };

            if let Some(ref snapshot) = complexity_snapshot {
                self.update_prometheus_from_complexity(snapshot);
            }
        }

        increment_samples_processed();
    }

    /// Update Prometheus metrics from a ComplexitySnapshot.
    fn update_prometheus_from_complexity(&self, snapshot: &ComplexitySnapshot) {
        // Update baseline metrics
        update_baseline_metrics(
            snapshot.baseline.progress,
            snapshot.baseline.state == "locked",
        );

        // Update z-scores
        if let Some(ref z) = snapshot.z_scores {
            update_zscore_metrics(
                z.tc.unwrap_or(0.0),
                z.h_joint.unwrap_or(0.0),
                z.h_bytes,
                z.r.unwrap_or(0.0),
            );
        }

        // Update deltas
        if let Some(ref d) = snapshot.deltas {
            update_delta_metrics(
                d.tc.unwrap_or(0.0),
                d.h_joint.unwrap_or(0.0),
                d.h_bytes,
                d.r.unwrap_or(0.0),
            );
        }

        // Record anomaly events
        for event in &snapshot.events {
            let severity = match event.severity {
                EventSeverity::Info => Severity::Info,
                EventSeverity::Warning => Severity::Warning,
                EventSeverity::Critical => Severity::Critical,
            };
            record_anomaly_event(event.event_type.as_str(), severity);
        }
    }

    /// Create an InputSnapshot from a MetricsSnapshot.
    fn create_input_snapshot(
        &self,
        metrics: &alec_gateway::MetricsSnapshot,
        timestamp_ms: u64,
    ) -> InputSnapshot {
        let mut input = InputSnapshot::minimal(timestamp_ms, metrics.payload.h_bytes);
        input.source = "replay".to_string();

        if metrics.signal.valid {
            input.tc = Some(metrics.signal.total_corr);
            input.h_joint = Some(metrics.signal.h_joint);

            for channel in &metrics.signal.h_per_channel {
                input.channel_entropies.push(ComplexityChannelEntropy {
                    channel_id: channel.channel_id.clone(),
                    h: channel.h,
                });
            }
        }

        if let Some(ref resilience) = metrics.resilience {
            input.r = resilience.r;
        }

        input
    }

    /// Reset engines for a clean loop.
    async fn reset_engines(&self) {
        // Reset gateway
        {
            let mut gateway = self.gateway.write().await;
            *gateway = Gateway::with_config(GatewayConfig {
                max_frame_size: 242,
                ..Default::default()
            });

            for sensor_id in &self.sensor_ids {
                let channel_config = ChannelConfig::default();
                gateway.add_channel(sensor_id, channel_config).ok();
            }
        }

        // Reset metrics engine
        {
            let mut metrics_engine = self.metrics_engine.write().await;
            *metrics_engine = MetricsEngine::new(MetricsConfig {
                enabled: true,
                ..Default::default()
            });
        }

        // Reset complexity engine
        {
            let mut complexity_engine = self.complexity_engine.write().await;
            *complexity_engine = ComplexityEngine::new(ComplexityConfig {
                enabled: true,
                ..Default::default()
            });
        }
    }

    /// Stop the replay.
    #[allow(dead_code)]
    pub fn stop(&self) {
        self.state.running.store(false, Ordering::SeqCst);
    }

    /// Pause/resume the replay.
    #[allow(dead_code)]
    pub fn set_paused(&self, paused: bool) {
        self.state.paused.store(paused, Ordering::SeqCst);
    }

    /// Seek to a specific position.
    #[allow(dead_code)]
    pub fn seek(&self, position: usize) {
        let clamped = position.min(self.rows.len().saturating_sub(1));
        self.state.position.store(clamped, Ordering::SeqCst);
    }

    /// Get dataset info.
    pub fn dataset_info(&self) -> DatasetInfo {
        let duration_ms = if self.rows.len() >= 2 {
            self.rows.last().unwrap().timestamp_ms - self.rows.first().unwrap().timestamp_ms
        } else {
            0
        };

        DatasetInfo {
            sensor_count: self.sensor_ids.len(),
            sample_count: self.rows.len(),
            duration_ms,
            sensor_ids: self.sensor_ids.clone(),
        }
    }
}

/// Dataset information.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DatasetInfo {
    pub sensor_count: usize,
    pub sample_count: usize,
    pub duration_ms: u64,
    pub sensor_ids: Vec<String>,
}

/// Replay errors.
#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Empty dataset")]
    EmptyDataset,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_csv() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "timestamp_ms,temp,humidity").unwrap();
        writeln!(file, "1000,25.0,60.0").unwrap();
        writeln!(file, "2000,25.5,61.0").unwrap();
        writeln!(file, "3000,26.0,62.0").unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_parse_csv() {
        let file = create_test_csv();
        let (sensor_ids, rows) = ReplayEngine::parse_csv(file.path()).expect("Failed to parse CSV");

        assert_eq!(sensor_ids, vec!["temp", "humidity"]);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].timestamp_ms, 1000);
        assert_eq!(rows[0].values.get("temp"), Some(&Some(25.0)));
    }

    #[test]
    fn test_replay_engine_creation() {
        let file = create_test_csv();
        let config = ReplayConfig {
            csv_path: file.path().to_string_lossy().to_string(),
            speed: 1.0,
            loop_replay: false,
            default_sample_interval_ms: 1000,
        };

        let engine = ReplayEngine::from_csv(config).expect("Failed to create engine");
        assert_eq!(engine.sensor_ids.len(), 2);
        assert_eq!(engine.rows.len(), 3);
    }

    #[test]
    fn test_dataset_info() {
        let file = create_test_csv();
        let config = ReplayConfig {
            csv_path: file.path().to_string_lossy().to_string(),
            ..Default::default()
        };

        let engine = ReplayEngine::from_csv(config).unwrap();
        let info = engine.dataset_info();

        assert_eq!(info.sensor_count, 2);
        assert_eq!(info.sample_count, 3);
        assert_eq!(info.duration_ms, 2000);
    }
}
