//! Fleet demo - demonstrates multi-emitter scenario with cross-fleet anomaly detection
//!
//! Run with: cargo run --example fleet_demo

use alec::fleet::{EmitterId, FleetConfig, FleetManager};
use alec::{Classifier, Context, Encoder, RawData};

fn main() {
    // Configure fleet
    let config = FleetConfig {
        min_emitters_for_comparison: 3,
        cross_fleet_threshold: 2.5,
        fleet_sync_interval: 100,
        ..Default::default()
    };

    let mut fleet = FleetManager::with_config(config);

    // Simulate 10 emitters
    let num_emitters = 10;
    let mut encoders: Vec<_> = (0..num_emitters)
        .map(|_| (Encoder::new(), Context::new()))
        .collect();

    let classifier = Classifier::default();

    println!("=== ALEC Fleet Demo ===");
    println!(
        "Simulating {} emitters over 1000 time steps\n",
        num_emitters
    );
    println!("Emitter 5 will spike at t=500\n");

    let mut anomaly_events: Vec<(u64, EmitterId)> = Vec::new();

    // Simulate 1000 messages
    for t in 0..1000u64 {
        // Each emitter sends data
        for (emitter_id, (encoder, context)) in encoders.iter_mut().enumerate() {
            // Normal temperature with slight variation per emitter
            let base_temp = 20.0 + (emitter_id as f64 * 0.5);
            let temp = base_temp + (t as f64 * 0.01).sin() * 2.0;

            // Inject anomaly for emitter 5 after t=500
            let temp = if emitter_id == 5 && t > 500 {
                temp + 25.0 // Sudden spike
            } else {
                temp
            };

            let data = RawData::new(temp, t);
            let classification = classifier.classify(&data, context);
            let message = encoder.encode(&data, &classification, context);

            let result = fleet.process_message(emitter_id as EmitterId, &message, t);

            if let Ok(processed) = result {
                if processed.is_cross_fleet_anomaly {
                    anomaly_events.push((t, emitter_id as EmitterId));
                    if anomaly_events.len() <= 5 {
                        println!(
                            "[t={:4}] Cross-fleet anomaly: Emitter {} (value: {:.2})",
                            t, emitter_id, processed.value
                        );
                    }
                }
            }

            context.observe(&data);
        }

        // Periodic status
        if t > 0 && t % 250 == 0 {
            let stats = fleet.stats();
            println!("\n--- Status at t={} ---", t);
            println!("  Total messages: {}", stats.total_messages);
            println!("  Cross-fleet anomalies: {}", stats.cross_fleet_anomalies);
            println!("  Anomalous emitters: {:?}", fleet.anomalous_emitters());
            if let Some(mean) = fleet.fleet_mean() {
                println!("  Fleet mean: {:.2}", mean);
            }
            println!();
        }
    }

    // Final statistics
    let stats = fleet.stats();
    println!("\n=== Final Statistics ===");
    println!("Total emitters: {}", fleet.emitter_count());
    println!("Total messages processed: {}", stats.total_messages);
    println!("Regular anomalies: {}", stats.anomaly_count);
    println!("Cross-fleet anomalies: {}", stats.cross_fleet_anomalies);
    println!("\nPriority distribution:");
    for (priority, count) in &stats.priority_distribution {
        println!("  {:?}: {} messages", priority, count);
    }

    println!("\nAnomalous emitters: {:?}", fleet.anomalous_emitters());

    // Show emitter stats
    println!("\n=== Emitter Statistics ===");
    for emitter_id in 0..num_emitters {
        if let Some(state) = fleet.get_emitter(emitter_id as EmitterId) {
            let status = if state.is_anomalous {
                "ANOMALOUS"
            } else {
                "normal"
            };
            println!(
                "Emitter {}: messages={}, mean={:.2}, status={}",
                emitter_id,
                state.message_count,
                state.mean().unwrap_or(0.0),
                status
            );
        }
    }

    // Fleet context patterns
    println!(
        "\nFleet context patterns: {}",
        fleet.fleet_context().pattern_count()
    );

    println!("\nDemo complete!");
}
