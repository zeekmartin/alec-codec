//! Example: Run dataset through ALEC Gateway with metrics.
//!
//! This example demonstrates how to process test data through
//! the ALEC Gateway and collect metrics.
//!
//! Run with: cargo run --example run_through_gateway --features gateway

#[cfg(feature = "gateway")]
use alec_gateway::{ChannelConfig, Gateway, GatewayConfig};
#[cfg(feature = "gateway")]
use alec_gateway::metrics::MetricsConfig;

use alec_testdata::{generate_dataset, GeneratorConfig};
use alec_testdata::industries::agriculture::{create_farm_sensors, AgriculturalScenario};

fn main() {
    println!("ALEC Gateway Integration Example");
    println!("=================================\n");

    // Generate a test dataset
    let config = GeneratorConfig::new()
        .with_sample_interval_secs(60)
        .with_duration_hours(1.0)
        .with_seed(42);

    let sensors = create_farm_sensors(AgriculturalScenario::Normal);
    let dataset = generate_dataset(&config, &sensors);

    println!("Generated dataset:");
    println!("  Samples: {}", dataset.len());
    println!("  Duration: {} ms", dataset.duration_ms());
    println!("  Sensors: {:?}\n", dataset.sensor_ids());

    #[cfg(feature = "gateway")]
    {
        run_through_gateway(&dataset);
    }

    #[cfg(not(feature = "gateway"))]
    {
        println!("Gateway feature not enabled.");
        println!("Run with: cargo run --example run_through_gateway --features gateway");
    }
}

#[cfg(feature = "gateway")]
fn run_through_gateway(dataset: &alec_testdata::Dataset) {
    // Create gateway with metrics enabled
    let gateway_config = GatewayConfig {
        max_frame_size: 242,
        ..Default::default()
    };

    let mut gateway = Gateway::with_config(gateway_config);

    // Enable metrics
    gateway.enable_metrics(MetricsConfig {
        enabled: true,
        ..Default::default()
    });

    // Add channels for each sensor
    for sensor_id in dataset.sensor_ids() {
        gateway
            .add_channel(sensor_id, ChannelConfig::default())
            .expect("Failed to add channel");
    }

    println!("Processing dataset through gateway...\n");

    let mut total_frames = 0;
    let mut total_bytes = 0;
    let mut metrics_snapshots = Vec::new();

    for (i, row) in dataset.rows().iter().enumerate() {
        // Push sensor values
        for (sensor_id, value) in row.iter() {
            if let Some(v) = value {
                if let Err(e) = gateway.push(sensor_id, v, row.timestamp_ms) {
                    eprintln!("Warning: Failed to push {}: {}", sensor_id, e);
                }
            }
        }

        // Flush every 10 samples (simulating transmission window)
        if (i + 1) % 10 == 0 {
            match gateway.flush() {
                Ok(frame) => {
                    total_frames += 1;
                    total_bytes += frame.size();

                    // Collect metrics
                    if let Some(snapshot) = gateway.last_metrics() {
                        metrics_snapshots.push(snapshot.clone());
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Flush failed: {}", e);
                }
            }
        }
    }

    // Final flush
    if let Ok(frame) = gateway.flush() {
        total_frames += 1;
        total_bytes += frame.size();
        if let Some(snapshot) = gateway.last_metrics() {
            metrics_snapshots.push(snapshot.clone());
        }
    }

    // Report results
    println!("Gateway Results:");
    println!("  Total frames: {}", total_frames);
    println!("  Total bytes: {}", total_bytes);
    println!("  Metrics snapshots: {}", metrics_snapshots.len());

    if !metrics_snapshots.is_empty() {
        // Compute average metrics
        let avg_tc: f64 = metrics_snapshots
            .iter()
            .map(|s| s.signal.total_corr)
            .sum::<f64>()
            / metrics_snapshots.len() as f64;

        let avg_h_bytes: f64 = metrics_snapshots
            .iter()
            .map(|s| s.payload.h_bytes)
            .sum::<f64>()
            / metrics_snapshots.len() as f64;

        println!("\nMetrics Summary:");
        println!("  Avg Total Correlation: {:.3}", avg_tc);
        println!("  Avg H_bytes: {:.3}", avg_h_bytes);

        // Print last snapshot as JSON
        if let Some(last) = metrics_snapshots.last() {
            if let Ok(json) = last.to_json() {
                println!("\nLast metrics snapshot:");
                println!("{}", json);
            }
        }
    }
}
