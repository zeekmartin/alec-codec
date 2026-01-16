// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Demo Preload Generator for ALEC
//!
//! This example generates pre-trained context files (preloads) for common
//! sensor types. These preloads allow ALEC to achieve optimal compression
//! from the very first byte, eliminating the "cold start" problem.
//!
//! # Generated Preloads
//!
//! - `demo_temperature_v1.alec-context` - Indoor temperature sensor (20-25°C)
//! - `demo_humidity_v1.alec-context` - Humidity sensor with daily cycle (40-60%)
//! - `demo_counter_v1.alec-context` - Monotonic counter with occasional resets
//!
//! # Usage
//!
//! ```bash
//! cargo run --example generate_preloads
//! ```
//!
//! # Using Generated Preloads
//!
//! ```rust,no_run
//! use alec::context::Context;
//! use alec::Encoder;
//! use std::path::Path;
//!
//! // Load a pre-trained context
//! let context = Context::load_from_file(
//!     Path::new("contexts/demo/demo_temperature_v1.alec-context")
//! ).expect("Failed to load preload");
//!
//! // Create encoder with pre-trained context - ready for optimal compression!
//! let mut encoder = Encoder::with_context(context);
//! ```

use alec::context::{
    Context, ContextConfig, EvolutionConfig, Pattern, PreloadDictEntry, PreloadFile,
    PreloadPredictionModel, PreloadPredictionType, PreloadStatistics,
};
use alec::protocol::RawData;
use std::path::Path;

/// Configuration for demo preload generation
struct PreloadConfig {
    sensor_type: &'static str,
    min_value: f64,
    max_value: f64,
    typical_value: f64,
    noise_amplitude: f64,
    training_samples: usize,
    prediction_type: PreloadPredictionType,
    prediction_coefficients: Vec<f64>,
    period_samples: u32,
}

fn main() {
    println!("=== ALEC Demo Preload Generator ===\n");

    let output_dir = Path::new("contexts/demo");

    // Ensure output directory exists
    std::fs::create_dir_all(output_dir).expect("Failed to create output directory");

    // Generate all demo preloads
    generate_temperature_preload(output_dir);
    generate_humidity_preload(output_dir);
    generate_counter_preload(output_dir);

    println!("\n=== Verification ===\n");

    // Verify all generated preloads
    verify_preload(output_dir.join("demo_temperature_v1.alec-context"));
    verify_preload(output_dir.join("demo_humidity_v1.alec-context"));
    verify_preload(output_dir.join("demo_counter_v1.alec-context"));

    println!("\n=== Done ===");
    println!("\nPreloads are ready in: {}", output_dir.display());
    println!("\nUsage example:");
    println!("  let ctx = Context::load_from_file(Path::new(\"contexts/demo/demo_temperature_v1.alec-context\"))?;");
}

/// Generate temperature sensor preload
///
/// Simulates an indoor temperature sensor:
/// - Range: 20.0 - 25.0 °C
/// - Typical: 22.5 °C
/// - Noise: ±0.1 °C
/// - Slow drift over time
fn generate_temperature_preload(output_dir: &Path) {
    println!("Generating: demo_temperature_v1.alec-context");

    let config = PreloadConfig {
        sensor_type: "temperature",
        min_value: 20.0,
        max_value: 25.0,
        typical_value: 22.5,
        noise_amplitude: 0.1,
        training_samples: 10000,
        prediction_type: PreloadPredictionType::MovingAverage,
        prediction_coefficients: vec![3.0], // 3-sample moving average
        period_samples: 0,
    };

    // Create context with evolution disabled for consistent training
    let ctx_config = ContextConfig {
        evolution: EvolutionConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut ctx = Context::with_config(ctx_config);

    // Register common delta patterns for temperature
    // Temperature typically changes by small amounts
    let delta_patterns: Vec<i16> = vec![
        0, // No change (most common)
        1, -1, // ±0.01°C (scaled)
        2, -2, // ±0.02°C
        5, -5, // ±0.05°C
        10, -10, // ±0.1°C
    ];

    for delta in delta_patterns {
        let bytes = delta.to_le_bytes().to_vec();
        let _ = ctx.register_pattern(Pattern::new(bytes));
    }

    // Simulate temperature readings with slow drift and noise
    let mut value = config.typical_value;
    let mut drift_direction = 1.0;

    for i in 0..config.training_samples {
        // Add small noise
        let noise = (simple_random(i as u64) - 0.5) * 2.0 * config.noise_amplitude;

        // Slow drift (changes direction at boundaries)
        if value >= config.max_value - 0.5 {
            drift_direction = -1.0;
        } else if value <= config.min_value + 0.5 {
            drift_direction = 1.0;
        }
        let drift = drift_direction * 0.001;

        value = (value + drift + noise).clamp(config.min_value, config.max_value);

        ctx.observe(&RawData::new(value, i as u64));
    }

    // Build preload with custom statistics
    let preload = build_preload(&ctx, &config);

    // Save preload
    let path = output_dir.join("demo_temperature_v1.alec-context");
    preload
        .save_to_file(&path)
        .expect("Failed to save temperature preload");

    println!("  - Saved: {}", path.display());
    println!("  - Training samples: {}", config.training_samples);
    println!("  - Patterns: {}", ctx.pattern_count());
}

/// Generate humidity sensor preload
///
/// Simulates a humidity sensor with daily cycle:
/// - Range: 40.0 - 60.0 %
/// - Typical: 50.0 %
/// - Daily cycle: ±5%
/// - Noise: ±2%
fn generate_humidity_preload(output_dir: &Path) {
    println!("Generating: demo_humidity_v1.alec-context");

    let config = PreloadConfig {
        sensor_type: "humidity",
        min_value: 40.0,
        max_value: 60.0,
        typical_value: 50.0,
        noise_amplitude: 2.0,
        training_samples: 10000,
        prediction_type: PreloadPredictionType::Periodic,
        prediction_coefficients: vec![],
        period_samples: 86400, // 1 day in seconds
    };

    let ctx_config = ContextConfig {
        evolution: EvolutionConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut ctx = Context::with_config(ctx_config);

    // Register common delta patterns for humidity
    let delta_patterns: Vec<i16> = vec![
        0, // No change
        10, -10, // ±0.1%
        20, -20, // ±0.2%
        50, -50, // ±0.5%
        100, -100, // ±1.0%
    ];

    for delta in delta_patterns {
        let bytes = delta.to_le_bytes().to_vec();
        let _ = ctx.register_pattern(Pattern::new(bytes));
    }

    // Simulate humidity with daily sinusoidal cycle
    let samples_per_day = 86400; // 1 sample per second
    let cycle_amplitude = 5.0;

    for i in 0..config.training_samples {
        // Daily cycle (simplified: use sample index as time)
        let day_progress = (i % samples_per_day) as f64 / samples_per_day as f64;
        let cycle_value = (day_progress * 2.0 * std::f64::consts::PI).sin() * cycle_amplitude;

        // Add noise
        let noise = (simple_random(i as u64) - 0.5) * 2.0 * config.noise_amplitude;

        let value =
            (config.typical_value + cycle_value + noise).clamp(config.min_value, config.max_value);

        ctx.observe(&RawData::new(value, i as u64));
    }

    let preload = build_preload(&ctx, &config);

    let path = output_dir.join("demo_humidity_v1.alec-context");
    preload
        .save_to_file(&path)
        .expect("Failed to save humidity preload");

    println!("  - Saved: {}", path.display());
    println!("  - Training samples: {}", config.training_samples);
    println!("  - Patterns: {}", ctx.pattern_count());
}

/// Generate counter sensor preload
///
/// Simulates a monotonic counter:
/// - Range: 0 - 1,000,000
/// - Behavior: Increment by 1, occasional reset to 0
/// - Prediction: Linear (slope=1)
fn generate_counter_preload(output_dir: &Path) {
    println!("Generating: demo_counter_v1.alec-context");

    let config = PreloadConfig {
        sensor_type: "counter",
        min_value: 0.0,
        max_value: 1_000_000.0,
        typical_value: 0.0,
        noise_amplitude: 0.0,
        training_samples: 10000,
        prediction_type: PreloadPredictionType::Linear,
        prediction_coefficients: vec![1.0, 0.0], // slope=1, intercept=0 (predict next = current + 1)
        period_samples: 0,
    };

    let ctx_config = ContextConfig {
        evolution: EvolutionConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut ctx = Context::with_config(ctx_config);

    // Register common patterns for counter
    // Delta of 1 is most common
    let patterns: Vec<Vec<u8>> = vec![
        1i64.to_le_bytes().to_vec(), // Delta +1 (most common)
        0i64.to_le_bytes().to_vec(), // No change (rare)
        vec![0xFF; 8],               // Reset marker pattern
    ];

    for pattern in patterns {
        let _ = ctx.register_pattern(Pattern::new(pattern));
    }

    // Simulate counter with occasional resets
    let mut counter: i64 = 0;
    let reset_probability = 0.001; // 0.1% chance of reset per sample

    for i in 0..config.training_samples {
        // Occasional reset
        if simple_random(i as u64 + 1000) < reset_probability {
            counter = 0;
        } else {
            counter += 1;
        }

        ctx.observe(&RawData::new(counter as f64, i as u64));
    }

    let preload = build_preload(&ctx, &config);

    let path = output_dir.join("demo_counter_v1.alec-context");
    preload
        .save_to_file(&path)
        .expect("Failed to save counter preload");

    println!("  - Saved: {}", path.display());
    println!("  - Training samples: {}", config.training_samples);
    println!("  - Patterns: {}", ctx.pattern_count());
}

/// Build a PreloadFile from a trained context and configuration
fn build_preload(ctx: &Context, config: &PreloadConfig) -> PreloadFile {
    // Collect dictionary entries from context
    let mut dictionary: Vec<PreloadDictEntry> = Vec::new();
    for (&code, pattern) in ctx.patterns_iter() {
        if code <= u16::MAX as u32 {
            dictionary.push(PreloadDictEntry {
                pattern: pattern.data.clone(),
                code: code as u16,
                frequency: pattern.frequency as u32,
            });
        }
    }
    dictionary.sort_by_key(|e| e.code);

    // Build statistics
    let statistics = PreloadStatistics {
        mean: config.typical_value,
        variance: config.noise_amplitude.powi(2),
        min_observed: config.min_value,
        max_observed: config.max_value,
        min_expected: config.min_value - config.noise_amplitude,
        max_expected: config.max_value + config.noise_amplitude,
        recent_values: vec![config.typical_value; 3],
    };

    // Build prediction model
    let prediction = PreloadPredictionModel {
        model_type: config.prediction_type,
        coefficients: config.prediction_coefficients.clone(),
        period_samples: config.period_samples,
    };

    PreloadFile {
        format_version: 1,
        context_version: ctx.version(),
        sensor_type: config.sensor_type.to_string(),
        created_timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        training_samples: config.training_samples as u64,
        dictionary,
        statistics,
        prediction,
    }
}

/// Simple deterministic pseudo-random number generator (0.0 - 1.0)
fn simple_random(seed: u64) -> f64 {
    // Simple LCG for reproducible results
    let a: u64 = 6364136223846793005;
    let c: u64 = 1442695040888963407;
    let m: u64 = u64::MAX;
    let result = seed.wrapping_mul(a).wrapping_add(c) % m;
    result as f64 / m as f64
}

/// Verify a preload file is valid
fn verify_preload<P: AsRef<Path>>(path: P) {
    let path = path.as_ref();
    print!("Verifying: {} ... ", path.display());

    match PreloadFile::load_from_file(path) {
        Ok(preload) => {
            // Check file is valid
            assert!(
                !preload.sensor_type.is_empty(),
                "Sensor type should not be empty"
            );
            assert!(preload.format_version == 1, "Format version should be 1");
            assert!(preload.training_samples > 0, "Should have training samples");

            // Check statistics are reasonable
            assert!(
                preload.statistics.min_observed <= preload.statistics.max_observed,
                "Min should be <= max"
            );
            assert!(
                preload.statistics.mean >= preload.statistics.min_observed
                    && preload.statistics.mean <= preload.statistics.max_observed,
                "Mean should be within observed range"
            );

            println!("OK");
            println!("    Sensor type: {}", preload.sensor_type);
            println!("    Context version: {}", preload.context_version);
            println!("    Training samples: {}", preload.training_samples);
            println!("    Dictionary entries: {}", preload.dictionary.len());
            println!("    Prediction model: {:?}", preload.prediction.model_type);
        }
        Err(e) => {
            println!("FAILED");
            println!("    Error: {}", e);
        }
    }
}
