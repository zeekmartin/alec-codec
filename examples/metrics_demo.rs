//! Metrics demonstration example
//!
//! This example shows how to use ALEC's compression metrics to analyze
//! encoding efficiency across different data patterns.
//!
//! Run with: `cargo run --example metrics_demo`

use alec::{Classifier, CompressionMetrics, Context, ContextMetrics, Encoder, RawData};

fn main() {
    println!("=== ALEC Compression Metrics Demo ===\n");

    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();
    let mut metrics = CompressionMetrics::new();

    // Simulate 1000 sensor readings with realistic temperature data
    println!("Simulating 1000 sensor readings...\n");

    for i in 0..1000 {
        // Temperature with slow sinusoidal variation (simulating day/night cycle)
        let temp = 20.0 + (i as f64 * 0.01).sin() * 5.0;

        // Add some noise
        let noise = ((i * 7919) % 100) as f64 / 1000.0 - 0.05;
        let value = temp + noise;

        let data = RawData::new(value, i as u64);
        let classification = classifier.classify(&data, &context);

        // Encode with metrics collection
        let _message = encoder.encode_with_metrics(&data, &classification, &context, &mut metrics);

        // Track prediction accuracy
        if let Some(prediction) = context.predict(0) {
            let error = (prediction.value - value).abs();
            metrics.record_prediction(error < 0.5); // Hit if within 0.5 degrees
        }

        // Update context for next iteration
        context.observe(&data);
    }

    // Print compression metrics report
    println!("{}", metrics.report());

    // Print context metrics
    let ctx_metrics = ContextMetrics::from_context(&context);
    println!("{}", ctx_metrics.report());

    // Additional insights
    println!("=== Additional Insights ===\n");
    println!(
        "Bytes saved: {} bytes",
        metrics.raw_bytes - metrics.encoded_bytes
    );

    if let Some(best_encoding) = metrics.most_used_encoding() {
        println!("Most efficient encoding: {:?}", best_encoding);
    }

    println!(
        "\nEfficiency summary: {:.1}x compression with {:.1}% prediction accuracy",
        metrics.compression_ratio(),
        metrics.prediction_accuracy() * 100.0
    );
}
