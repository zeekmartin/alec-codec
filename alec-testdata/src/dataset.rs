// ALEC Testdata - Dataset structures
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Dataset structures and I/O operations.
//!
//! Provides the `Dataset` type for storing and exporting generated data.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use thiserror::Error;

/// Dataset error types.
#[derive(Debug, Error)]
pub enum DatasetError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV parse error at line {line}: {message}")]
    CsvParse { line: usize, message: String },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Missing column: {0}")]
    MissingColumn(String),

    #[error("Empty dataset")]
    Empty,
}

/// A single row of dataset values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetRow {
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Sensor values keyed by sensor ID.
    pub values: HashMap<String, Option<f64>>,
}

impl DatasetRow {
    /// Create a new row.
    pub fn new(timestamp_ms: u64) -> Self {
        Self {
            timestamp_ms,
            values: HashMap::new(),
        }
    }

    /// Add a sensor value.
    pub fn with_value(mut self, sensor_id: &str, value: Option<f64>) -> Self {
        self.values.insert(sensor_id.to_string(), value);
        self
    }

    /// Get a sensor value.
    pub fn get(&self, sensor_id: &str) -> Option<f64> {
        self.values.get(sensor_id).copied().flatten()
    }

    /// Iterate over sensor values.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Option<f64>)> {
        self.values.iter().map(|(k, v)| (k.as_str(), *v))
    }
}

/// A dataset containing time series sensor data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    /// Ordered list of sensor IDs (column order).
    pub sensor_ids: Vec<String>,
    /// Data rows.
    pub rows: Vec<DatasetRow>,
    /// Metadata.
    #[serde(default)]
    pub metadata: DatasetMetadata,
}

/// Dataset metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatasetMetadata {
    /// Dataset name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Industry/domain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub industry: Option<String>,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Generation seed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Sample interval in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_interval_ms: Option<u64>,
}

impl Dataset {
    /// Create an empty dataset with given sensor IDs.
    pub fn new(sensor_ids: Vec<String>) -> Self {
        Self {
            sensor_ids,
            rows: Vec::new(),
            metadata: DatasetMetadata::default(),
        }
    }

    /// Add a row from a vector of values (timestamp first, then sensors in order).
    pub fn add_row_vec(&mut self, values: Vec<f64>) {
        if values.is_empty() {
            return;
        }

        let timestamp_ms = values[0] as u64;
        let mut row = DatasetRow::new(timestamp_ms);

        for (i, value) in values.iter().skip(1).enumerate() {
            if i < self.sensor_ids.len() {
                let value = if value.is_nan() { None } else { Some(*value) };
                row.values.insert(self.sensor_ids[i].clone(), value);
            }
        }

        self.rows.push(row);
    }

    /// Add a DatasetRow.
    pub fn add_row(&mut self, row: DatasetRow) {
        self.rows.push(row);
    }

    /// Get sensor IDs.
    pub fn sensor_ids(&self) -> &[String] {
        &self.sensor_ids
    }

    /// Get all rows.
    pub fn rows(&self) -> &[DatasetRow] {
        &self.rows
    }

    /// Get number of samples.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Get duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        if self.rows.len() < 2 {
            return 0;
        }
        self.rows.last().unwrap().timestamp_ms - self.rows.first().unwrap().timestamp_ms
    }

    /// Get a column as a vector of values.
    pub fn column(&self, sensor_id: &str) -> Vec<Option<f64>> {
        self.rows.iter().map(|r| r.get(sensor_id)).collect()
    }

    /// Get timestamps as a vector.
    pub fn timestamps(&self) -> Vec<u64> {
        self.rows.iter().map(|r| r.timestamp_ms).collect()
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: DatasetMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set name.
    pub fn with_name(mut self, name: &str) -> Self {
        self.metadata.name = Some(name.to_string());
        self
    }

    /// Set industry.
    pub fn with_industry(mut self, industry: &str) -> Self {
        self.metadata.industry = Some(industry.to_string());
        self
    }

    /// Set description.
    pub fn with_description(mut self, description: &str) -> Self {
        self.metadata.description = Some(description.to_string());
        self
    }

    /// Export to CSV file.
    pub fn to_csv(&self, path: impl AsRef<Path>) -> Result<(), DatasetError> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Header
        write!(writer, "timestamp_ms")?;
        for sensor_id in &self.sensor_ids {
            write!(writer, ",{}", sensor_id)?;
        }
        writeln!(writer)?;

        // Data rows
        for row in &self.rows {
            write!(writer, "{}", row.timestamp_ms)?;
            for sensor_id in &self.sensor_ids {
                match row.get(sensor_id) {
                    Some(v) => write!(writer, ",{:.6}", v)?,
                    None => write!(writer, ",")?,
                }
            }
            writeln!(writer)?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Import from CSV file.
    pub fn from_csv(path: impl AsRef<Path>) -> Result<Self, DatasetError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Parse header
        let header = lines.next().ok_or(DatasetError::Empty)??;
        let columns: Vec<&str> = header.split(',').collect();

        if columns.is_empty() || columns[0] != "timestamp_ms" {
            return Err(DatasetError::MissingColumn("timestamp_ms".to_string()));
        }

        let sensor_ids: Vec<String> = columns[1..].iter().map(|s| s.to_string()).collect();
        let mut dataset = Dataset::new(sensor_ids.clone());

        // Parse data rows
        for (line_num, line_result) in lines.enumerate() {
            let line = line_result?;
            let values: Vec<&str> = line.split(',').collect();

            if values.is_empty() {
                continue;
            }

            let timestamp_ms: u64 = values[0].parse().map_err(|_| DatasetError::CsvParse {
                line: line_num + 2,
                message: "Invalid timestamp".to_string(),
            })?;

            let mut row = DatasetRow::new(timestamp_ms);

            for (i, sensor_id) in sensor_ids.iter().enumerate() {
                let value = if i + 1 < values.len() {
                    let s = values[i + 1].trim();
                    if s.is_empty() {
                        None
                    } else {
                        Some(s.parse().map_err(|_| DatasetError::CsvParse {
                            line: line_num + 2,
                            message: format!("Invalid value for {}", sensor_id),
                        })?)
                    }
                } else {
                    None
                };
                row.values.insert(sensor_id.clone(), value);
            }

            dataset.rows.push(row);
        }

        Ok(dataset)
    }

    /// Export to JSON file.
    pub fn to_json(&self, path: impl AsRef<Path>) -> Result<(), DatasetError> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self)?;
        Ok(())
    }

    /// Import from JSON file.
    pub fn from_json(path: impl AsRef<Path>) -> Result<Self, DatasetError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let dataset = serde_json::from_reader(reader)?;
        Ok(dataset)
    }

    /// Calculate basic statistics for a sensor.
    pub fn stats(&self, sensor_id: &str) -> Option<SensorStats> {
        let values: Vec<f64> = self.column(sensor_id).into_iter().flatten().collect();

        if values.is_empty() {
            return None;
        }

        let count = values.len();
        let sum: f64 = values.iter().sum();
        let mean = sum / count as f64;

        let variance: f64 = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / count as f64;
        let std_dev = variance.sqrt();

        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        Some(SensorStats {
            count,
            mean,
            std_dev,
            min,
            max,
        })
    }
}

/// Basic statistics for a sensor column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorStats {
    pub count: usize,
    pub mean: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_dataset_creation() {
        let dataset = Dataset::new(vec!["temp".to_string(), "humidity".to_string()]);
        assert_eq!(dataset.sensor_ids.len(), 2);
        assert!(dataset.is_empty());
    }

    #[test]
    fn test_add_row_vec() {
        let mut dataset = Dataset::new(vec!["temp".to_string(), "humidity".to_string()]);

        dataset.add_row_vec(vec![1000.0, 25.5, 60.0]);
        dataset.add_row_vec(vec![2000.0, 26.0, 58.5]);

        assert_eq!(dataset.len(), 2);
        assert_eq!(dataset.rows[0].get("temp"), Some(25.5));
        assert_eq!(dataset.rows[1].get("humidity"), Some(58.5));
    }

    #[test]
    fn test_add_row() {
        let mut dataset = Dataset::new(vec!["temp".to_string()]);

        let row = DatasetRow::new(1000).with_value("temp", Some(25.0));
        dataset.add_row(row);

        assert_eq!(dataset.len(), 1);
        assert_eq!(dataset.rows[0].get("temp"), Some(25.0));
    }

    #[test]
    fn test_column() {
        let mut dataset = Dataset::new(vec!["temp".to_string()]);
        dataset.add_row_vec(vec![1000.0, 25.0]);
        dataset.add_row_vec(vec![2000.0, 26.0]);
        dataset.add_row_vec(vec![3000.0, 27.0]);

        let col = dataset.column("temp");
        assert_eq!(col, vec![Some(25.0), Some(26.0), Some(27.0)]);
    }

    #[test]
    fn test_duration() {
        let mut dataset = Dataset::new(vec!["temp".to_string()]);
        dataset.add_row_vec(vec![1000.0, 25.0]);
        dataset.add_row_vec(vec![5000.0, 26.0]);

        assert_eq!(dataset.duration_ms(), 4000);
    }

    #[test]
    fn test_csv_roundtrip() {
        let mut dataset = Dataset::new(vec!["temp".to_string(), "humidity".to_string()]);
        dataset.add_row_vec(vec![1000.0, 25.5, 60.0]);
        dataset.add_row_vec(vec![2000.0, 26.0, 58.5]);

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        dataset.to_csv(path).unwrap();
        let loaded = Dataset::from_csv(path).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.sensor_ids.len(), 2);
        assert_eq!(loaded.rows[0].get("temp"), Some(25.5));
    }

    #[test]
    fn test_csv_with_missing_values() {
        let mut dataset = Dataset::new(vec!["temp".to_string()]);
        let mut row = DatasetRow::new(1000);
        row.values.insert("temp".to_string(), None);
        dataset.add_row(row);

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        dataset.to_csv(path).unwrap();
        let loaded = Dataset::from_csv(path).unwrap();

        assert_eq!(loaded.rows[0].get("temp"), None);
    }

    #[test]
    fn test_json_roundtrip() {
        let mut dataset = Dataset::new(vec!["temp".to_string()])
            .with_name("test")
            .with_industry("testing");
        dataset.add_row_vec(vec![1000.0, 25.0]);

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        dataset.to_json(path).unwrap();
        let loaded = Dataset::from_json(path).unwrap();

        assert_eq!(loaded.metadata.name, Some("test".to_string()));
        assert_eq!(loaded.len(), 1);
    }

    #[test]
    fn test_stats() {
        let mut dataset = Dataset::new(vec!["temp".to_string()]);
        dataset.add_row_vec(vec![1000.0, 10.0]);
        dataset.add_row_vec(vec![2000.0, 20.0]);
        dataset.add_row_vec(vec![3000.0, 30.0]);

        let stats = dataset.stats("temp").unwrap();
        assert_eq!(stats.count, 3);
        assert_eq!(stats.mean, 20.0);
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 30.0);
    }
}
