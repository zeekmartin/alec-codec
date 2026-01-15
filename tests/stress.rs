//! Stress tests for ALEC
//!
//! Run with: cargo test --release stress -- --ignored

use alec::*;
use std::time::Instant;

#[test]
#[ignore] // Run manually with --ignored
fn stress_test_encoding() {
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();

    let iterations = 1_000_000;
    let start = Instant::now();

    for i in 0..iterations {
        let data = RawData::new(20.0 + (i as f64 * 0.001).sin(), i as u64);
        let classification = classifier.classify(&data, &context);
        let _message = encoder.encode(&data, &classification, &context);
        context.observe(&data);
    }

    let elapsed = start.elapsed();
    let rate = iterations as f64 / elapsed.as_secs_f64();

    println!("Encoded {} messages in {:?}", iterations, elapsed);
    println!("Rate: {:.0} messages/second", rate);

    assert!(
        rate > 100_000.0,
        "Should encode at least 100k msg/s, got {:.0}",
        rate
    );
}

#[test]
#[ignore]
fn stress_test_roundtrip() {
    let mut encoder = Encoder::new();
    let mut decoder = Decoder::new();
    let classifier = Classifier::default();
    let mut ctx_enc = Context::new();
    let mut ctx_dec = Context::new();

    let iterations = 100_000;
    let start = Instant::now();

    for i in 0..iterations {
        let data = RawData::new(20.0 + (i as f64 * 0.01).sin() * 5.0, i as u64);
        let classification = classifier.classify(&data, &ctx_enc);
        let message = encoder.encode(&data, &classification, &ctx_enc);
        let decoded = decoder.decode(&message, &ctx_dec).unwrap();

        assert!(
            (decoded.value - data.value).abs() < 0.1,
            "Roundtrip failed at iteration {}: {} vs {}",
            i,
            data.value,
            decoded.value
        );

        ctx_enc.observe(&data);
        ctx_dec.observe(&data);
    }

    let elapsed = start.elapsed();
    let rate = iterations as f64 / elapsed.as_secs_f64();

    println!("Roundtrip {} messages in {:?}", iterations, elapsed);
    println!("Rate: {:.0} messages/second", rate);

    assert!(
        rate > 50_000.0,
        "Should roundtrip at least 50k msg/s, got {:.0}",
        rate
    );
}

#[test]
#[ignore]
fn stress_test_fleet() {
    let mut fleet = FleetManager::new();
    let classifier = Classifier::default();

    let num_emitters = 100;
    let messages_per_emitter = 1000;

    let mut encoders: Vec<_> = (0..num_emitters)
        .map(|_| (Encoder::new(), Context::new()))
        .collect();

    let start = Instant::now();

    for t in 0..messages_per_emitter {
        for (emitter_id, (encoder, context)) in encoders.iter_mut().enumerate() {
            let temp = 20.0 + (emitter_id as f64 * 0.1) + (t as f64 * 0.01).sin();
            let data = RawData::new(temp, t as u64);
            let classification = classifier.classify(&data, context);
            let message = encoder.encode(&data, &classification, context);

            fleet
                .process_message(emitter_id as u32, &message, t as u64)
                .unwrap();
            context.observe(&data);
        }
    }

    let elapsed = start.elapsed();
    let total_messages = num_emitters * messages_per_emitter;
    let rate = total_messages as f64 / elapsed.as_secs_f64();

    println!("Fleet processed {} messages in {:?}", total_messages, elapsed);
    println!("Rate: {:.0} messages/second", rate);

    assert!(
        rate > 10_000.0,
        "Should process at least 10k msg/s, got {:.0}",
        rate
    );
}

#[test]
#[ignore]
fn stress_test_context_evolution() {
    use alec::context::EvolutionConfig;

    let mut context = Context::with_evolution(EvolutionConfig {
        min_frequency: 2,
        max_age: 10000,
        evolution_interval: 100,
        promotion_threshold: 5,
        enabled: true,
    });

    let iterations = 100_000;
    let start = Instant::now();

    for i in 0..iterations {
        // Generate various patterns
        let value = match i % 10 {
            0 => 0.0,                          // Constant
            1 => i as f64 * 0.1,               // Linear increase
            2 => (i as f64 * 0.1).sin() * 100.0, // Sine wave
            3 => if i % 2 == 0 { 10.0 } else { 20.0 }, // Alternating
            _ => 50.0 + (i % 100) as f64,      // Semi-random
        };

        let data = RawData::new(value, i as u64);
        context.observe(&data);

        // Trigger evolution every 1000 iterations
        if i % 1000 == 0 {
            context.evolve();
        }
    }

    let elapsed = start.elapsed();
    let rate = iterations as f64 / elapsed.as_secs_f64();

    println!(
        "Context evolution processed {} observations in {:?}",
        iterations, elapsed
    );
    println!("Rate: {:.0} observations/second", rate);
    println!("Final pattern count: {}", context.pattern_count());

    assert!(
        rate > 100_000.0,
        "Should process at least 100k observations/s, got {:.0}",
        rate
    );
}

#[test]
#[ignore]
fn stress_test_classifier() {
    let classifier = Classifier::default();
    let context = Context::new();

    let iterations = 1_000_000;
    let start = Instant::now();

    for i in 0..iterations {
        let value = match i % 5 {
            0 => -100.0, // Critical low
            1 => 150.0,  // Critical high
            2 => (i as f64 * 0.001).sin() * 10.0, // Normal variation
            3 => 50.0,   // Constant
            _ => i as f64 % 100.0, // Various
        };
        let data = RawData::new(value, i as u64);
        let _classification = classifier.classify(&data, &context);
    }

    let elapsed = start.elapsed();
    let rate = iterations as f64 / elapsed.as_secs_f64();

    println!(
        "Classified {} messages in {:?}",
        iterations, elapsed
    );
    println!("Rate: {:.0} classifications/second", rate);

    assert!(
        rate > 1_000_000.0,
        "Should classify at least 1M msg/s, got {:.0}",
        rate
    );
}

#[test]
#[ignore]
fn stress_test_multi_value_encoding() {
    use alec::protocol::Priority;

    let mut encoder = Encoder::new();
    let mut decoder = Decoder::new();
    let mut ctx = Context::new();

    let iterations = 50_000;
    let values_per_message = 5;
    let start = Instant::now();

    for i in 0..iterations {
        let base_timestamp = (i * values_per_message) as u64;
        let mut values: Vec<(u16, f64)> = Vec::with_capacity(values_per_message);

        for j in 0..values_per_message {
            let value = 20.0 + (((i * values_per_message + j) as f64) * 0.01).sin() * 5.0;
            values.push((j as u16, value));
        }

        // Encode multi-value
        let message = encoder.encode_multi(&values, i as u32, base_timestamp, Priority::P3Normal, &ctx);

        // Decode and verify
        let decoded = decoder.decode_multi(&message, &ctx).unwrap();
        assert_eq!(decoded.len(), values.len());

        // Observe for context
        for (name_id, value) in &values {
            ctx.observe(&RawData::new(*value, base_timestamp + *name_id as u64));
        }
    }

    let elapsed = start.elapsed();
    let total_values = iterations * values_per_message;
    let rate = total_values as f64 / elapsed.as_secs_f64();

    println!(
        "Multi-value encoded {} values in {:?}",
        total_values, elapsed
    );
    println!("Rate: {:.0} values/second", rate);

    assert!(
        rate > 100_000.0,
        "Should encode at least 100k values/s, got {:.0}",
        rate
    );
}

#[test]
#[ignore]
fn stress_test_sync_messages() {
    use alec::context::Pattern;
    use alec::sync::{SyncAnnounce, SyncDiff, SyncMessage, SyncRequest};

    let iterations = 100_000;
    let start = Instant::now();

    for i in 0..iterations {
        // Create and serialize various sync messages
        let msg = match i % 4 {
            0 => SyncMessage::Announce(SyncAnnounce {
                version: i as u32,
                hash: (i as u64) * 12345,
                pattern_count: (i % 100) as u16,
            }),
            1 => SyncMessage::Request(SyncRequest {
                from_version: i as u32,
                to_version: Some((i + 10) as u32),
            }),
            2 => SyncMessage::Diff(SyncDiff {
                base_version: i as u32,
                new_version: (i + 1) as u32,
                added: vec![(i as u32, Pattern::new(vec![1, 2, 3]))],
                removed: vec![],
                hash: (i as u64) * 54321,
            }),
            _ => SyncMessage::ReqDetail(i as u32),
        };

        let bytes = msg.to_bytes();
        let _decoded = SyncMessage::from_bytes(&bytes).unwrap();
    }

    let elapsed = start.elapsed();
    let rate = iterations as f64 / elapsed.as_secs_f64();

    println!(
        "Sync message roundtrip {} messages in {:?}",
        iterations, elapsed
    );
    println!("Rate: {:.0} messages/second", rate);

    assert!(
        rate > 50_000.0,
        "Should process at least 50k sync messages/s, got {:.0}",
        rate
    );
}

#[test]
#[ignore]
fn stress_test_memory_usage() {
    let mut context = Context::new();
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();

    // Measure initial memory
    let initial_patterns = context.pattern_count();

    // Process many messages
    for i in 0..10_000 {
        let data = RawData::new(20.0 + (i as f64 * 0.1).sin() * 10.0, i as u64);
        let classification = classifier.classify(&data, &context);
        let _msg = encoder.encode(&data, &classification, &context);
        context.observe(&data);
    }

    let final_patterns = context.pattern_count();
    let estimated_memory = context.estimated_memory();

    println!("Initial patterns: {}", initial_patterns);
    println!("Final patterns: {}", final_patterns);
    println!("Estimated memory: {} bytes", estimated_memory);

    // Memory should be bounded
    assert!(
        estimated_memory < 1_000_000,
        "Memory usage should be under 1MB, got {} bytes",
        estimated_memory
    );
}

#[test]
#[ignore]
fn stress_test_concurrent_fleet() {
    use std::sync::{Arc, Mutex};
    use std::thread;

    let fleet = Arc::new(Mutex::new(FleetManager::new()));
    let num_threads = 4;
    let messages_per_thread = 10_000;

    let start = Instant::now();

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let fleet = Arc::clone(&fleet);
            thread::spawn(move || {
                let mut encoder = Encoder::new();
                let classifier = Classifier::default();
                let mut context = Context::new();

                for i in 0..messages_per_thread {
                    let data = RawData::new(
                        20.0 + (thread_id as f64) + (i as f64 * 0.01).sin(),
                        i as u64,
                    );
                    let classification = classifier.classify(&data, &context);
                    let message = encoder.encode(&data, &classification, &context);

                    let emitter_id = (thread_id * 100 + (i % 10)) as u32;
                    fleet
                        .lock()
                        .unwrap()
                        .process_message(emitter_id, &message, i as u64)
                        .unwrap();

                    context.observe(&data);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_messages = num_threads * messages_per_thread;
    let rate = total_messages as f64 / elapsed.as_secs_f64();

    println!(
        "Concurrent fleet processed {} messages in {:?}",
        total_messages, elapsed
    );
    println!("Rate: {:.0} messages/second", rate);

    // Concurrent access will be slower, so lower threshold
    assert!(
        rate > 5_000.0,
        "Should process at least 5k msg/s concurrently, got {:.0}",
        rate
    );
}
