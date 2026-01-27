// ALEC Testdata - Realistic test dataset generator
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! # ALEC Testdata
//!
//! Realistic test dataset generator for the ALEC ecosystem.
//!
//! This crate provides generators for realistic IoT sensor data across
//! multiple industries, with support for:
//!
//! - **Signal patterns**: Sine waves, noise, steps, random walks, etc.
//! - **Anomaly injection**: Sensor failures, spikes, drift, correlation breaks
//! - **Industry presets**: Agriculture, Satellite, Manufacturing, Smart City, Logistics, Energy
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use alec_testdata::{GeneratorConfig, generate_dataset};
//! use alec_testdata::industries::agriculture::{AgriculturalScenario, create_farm_sensors};
//!
//! // Generate a 24-hour agricultural dataset
//! let config = GeneratorConfig::new()
//!     .with_duration_hours(24.0)
//!     .with_sample_interval_secs(60)
//!     .with_seed(42);
//!
//! let sensors = create_farm_sensors(AgriculturalScenario::Normal);
//! let dataset = generate_dataset(&config, &sensors);
//!
//! // Export to CSV
//! dataset.to_csv("farm_data.csv").unwrap();
//! ```
//!
//! ## Industry Generators
//!
//! Each industry module provides pre-configured sensor sets:
//!
//! - [`industries::agriculture`]: Soil, weather, and crop sensors
//! - [`industries::satellite`]: Battery, GPS, signal sensors
//! - [`industries::manufacturing`]: Motor, pressure, flow sensors
//! - [`industries::smart_city`]: Traffic, air quality, parking sensors
//! - [`industries::logistics`]: Cold chain, GPS tracking sensors
//! - [`industries::energy`]: Power grid monitoring sensors
//!
//! ## Anomaly Injection
//!
//! Anomalies can be injected to test detection systems:
//!
//! ```rust
//! use alec_testdata::{AnomalyConfig, AnomalyType};
//!
//! let anomaly = AnomalyConfig {
//!     anomaly_type: AnomalyType::Stuck,
//!     start_sample: 500,
//!     duration_samples: Some(200),
//! };
//! ```
//!
//! ## Pre-generated Datasets
//!
//! The `datasets/` directory contains pre-generated CSV files for each industry.
//! Use the manifest JSON files to understand expected metrics and anomalies.

pub mod anomalies;
pub mod dataset;
pub mod generator;
pub mod industries;
pub mod manifest;
pub mod patterns;
pub mod scenario;

// Re-exports for convenience
pub use anomalies::{AnomalyConfig, AnomalyType};
pub use dataset::{Dataset, DatasetRow};
pub use generator::{generate_dataset, GeneratorConfig, SensorConfig};
pub use manifest::{DatasetManifest, SensorManifest};
pub use patterns::SignalPattern;
pub use scenario::{AnomalyScenario, ExpectedEvent, ScenarioValidation};

/// Library version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
