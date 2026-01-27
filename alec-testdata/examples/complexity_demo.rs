//! Example: Run dataset through ALEC Complexity for anomaly detection.
//!
//! This example demonstrates how to process test data through
//! the ALEC Complexity engine and detect anomalies.
//!
//! Run with: cargo run --example complexity_demo --features complexity

#[cfg(feature = "complexity")]
use alec_complexity::{ComplexityConfig, ComplexityEngine, GenericInput};
#[cfg(feature = "complexity")]
use alec_complexity::config::BaselineConfig;

use alec_testdata::{generate_dataset, GeneratorConfig};
use alec_testdata::industries::agriculture::{create_farm_sensors, AgriculturalScenario};

fn main() {
    println!("ALEC Complexity Demo");
    println!("====================\n");

    // Generate test datasets
    demo_normal_operation();
    demo_drought_detection();
    demo_sensor_failure();
}

fn demo_normal_operation() {
    println!("1. Normal Operation");
    println!("-------------------");

    let config = GeneratorConfig::new()
        .with_sample_interval_secs(60)
        .with_duration_minutes(30.0)
        .with_seed(42);

    let sensors = create_farm_sensors(AgriculturalScenario::Normal);
    let dataset = generate_dataset(&config, &sensors);

    println!("Generated {} samples of normal operation", dataset.len());

    #[cfg(feature = "complexity")]
    {
        let events = run_complexity_analysis(&dataset);
        println!("Events detected: {}", events.len());
        if events.is_empty() {
            println!("  (No anomalies - as expected for normal operation)\n");
        } else {
            for event in &events {
                println!("  {:?}: {}", event.event_type, event.message);
            }
            println!();
        }
    }

    #[cfg(not(feature = "complexity"))]
    {
        println!("  (Complexity feature not enabled)\n");
    }
}

fn demo_drought_detection() {
    println!("2. Drought Event Detection");
    println!("--------------------------");

    let config = GeneratorConfig::new()
        .with_sample_interval_secs(60)
        .with_duration_hours(2.0)
        .with_seed(42);

    let sensors = create_farm_sensors(AgriculturalScenario::Drought);
    let dataset = generate_dataset(&config, &sensors);

    println!("Generated {} samples with drought pattern", dataset.len());

    #[cfg(feature = "complexity")]
    {
        let events = run_complexity_analysis(&dataset);
        println!("Events detected: {}", events.len());
        for event in &events {
            println!(
                "  [{:?}] {:?}: {}",
                event.severity, event.event_type, event.message
            );
        }
        if events.iter().any(|e| {
            matches!(
                e.event_type,
                alec_complexity::EventType::ComplexitySurge
                    | alec_complexity::EventType::StructureBreak
            )
        }) {
            println!("  ✓ Anomaly detected!\n");
        } else {
            println!("  (Note: May need longer duration for detection)\n");
        }
    }

    #[cfg(not(feature = "complexity"))]
    {
        println!("  (Complexity feature not enabled)\n");
    }
}

fn demo_sensor_failure() {
    println!("3. Sensor Failure Detection");
    println!("---------------------------");

    let config = GeneratorConfig::new()
        .with_sample_interval_secs(60)
        .with_duration_hours(2.0)
        .with_seed(42);

    let sensors = create_farm_sensors(AgriculturalScenario::SensorFailure);
    let dataset = generate_dataset(&config, &sensors);

    println!(
        "Generated {} samples with sensor failure at sample 500",
        dataset.len()
    );

    // Show the stuck values
    let moisture_col = dataset.column("soil_moisture");
    if moisture_col.len() > 510 {
        println!(
            "  Soil moisture at 499: {:?}",
            moisture_col[499]
        );
        println!(
            "  Soil moisture at 500 (failure start): {:?}",
            moisture_col[500]
        );
        println!(
            "  Soil moisture at 510 (should be same): {:?}",
            moisture_col[510]
        );
    }

    #[cfg(feature = "complexity")]
    {
        let events = run_complexity_analysis(&dataset);
        println!("\nEvents detected: {}", events.len());
        for event in &events {
            println!(
                "  [{:?}] {:?}: {}",
                event.severity, event.event_type, event.message
            );
        }
        if events.iter().any(|e| {
            matches!(
                e.event_type,
                alec_complexity::EventType::StructureBreak
            )
        }) {
            println!("  ✓ Sensor failure detected!\n");
        } else {
            println!("  (Note: Structure break detection may require tuning)\n");
        }
    }

    #[cfg(not(feature = "complexity"))]
    {
        println!("  (Complexity feature not enabled)\n");
    }
}

#[cfg(feature = "complexity")]
fn run_complexity_analysis(
    dataset: &alec_testdata::Dataset,
) -> Vec<alec_complexity::ComplexityEvent> {
    // Configure complexity engine
    let config = ComplexityConfig {
        enabled: true,
        baseline: BaselineConfig {
            min_valid_snapshots: 20,
            build_time_ms: 0, // Use snapshot count only
            ..Default::default()
        },
        ..Default::default()
    };

    let mut engine = ComplexityEngine::new(config);
    let mut all_events = Vec::new();

    // Process each row
    for row in dataset.rows() {
        // Create input from dataset row
        // We'll compute a simple "entropy" approximation based on value spread
        let values: Vec<f64> = row
            .iter()
            .filter_map(|(_, v)| v)
            .collect();

        if values.is_empty() {
            continue;
        }

        // Simple h_bytes approximation
        let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
        let variance: f64 = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
            / values.len() as f64;
        let h_bytes = (variance.sqrt() + 1.0).log2() * 2.0; // Rough approximation

        let input = GenericInput::new(row.timestamp_ms, h_bytes)
            .with_tc(variance.sqrt() / 10.0) // Rough TC approximation
            .with_r(0.5) // Placeholder
            .build();

        if let Some(snapshot) = engine.process(&input) {
            all_events.extend(snapshot.events.clone());
        }
    }

    all_events
}
