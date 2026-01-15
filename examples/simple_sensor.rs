//! Simple sensor simulation example
//!
//! This example demonstrates basic ALEC usage with a simulated temperature sensor.
//!
//! Run with: `cargo run --example simple_sensor`

use alec::{Classifier, Context, Decoder, Encoder, Priority, RawData};

fn main() {
    println!("=== ALEC Simple Sensor Example ===\n");

    // Create components
    let mut encoder = Encoder::new();
    let mut decoder = Decoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();

    // Simulate temperature readings
    let temperatures = vec![
        20.0, 20.1, 20.2, 20.1, 20.3,  // Normal readings
        20.2, 20.4, 20.3, 20.5, 20.4,  // Slight variations
        20.3, 20.2, 20.1, 20.0, 20.1,  // Stable
        35.0,  // Anomaly!
        20.5, 20.4, 20.3, 20.2,        // Back to normal
    ];

    println!("{:<5} {:<10} {:<15} {:<8} {:<10}",
             "N°", "Temp (°C)", "Priority", "Size", "Transmit?");
    println!("{}", "-".repeat(55));

    let mut total_raw_size = 0;
    let mut total_encoded_size = 0;
    let mut transmitted_count = 0;

    for (i, &temp) in temperatures.iter().enumerate() {
        // Create raw data
        let data = RawData::new(temp, i as u64);
        
        // Classify the data
        let classification = classifier.classify(&data, &context);
        
        // Encode
        let message = encoder.encode(&data, &classification, &context);
        
        // Track sizes
        total_raw_size += data.raw_size();
        let msg_size = message.len();
        
        // Check if we would transmit this
        let should_transmit = classification.priority.should_transmit();
        if should_transmit {
            total_encoded_size += msg_size;
            transmitted_count += 1;
            
            // Decode (verification)
            let decoded = decoder.decode(&message, &context).unwrap();
            assert!((decoded.value - temp).abs() < 0.1, "Decode mismatch!");
        }
        
        // Update context
        context.observe(&data);
        
        // Display
        let transmit_str = if should_transmit { "✓" } else { "✗" };
        println!(
            "{:<5} {:<10.1} {:<15} {:<8} {:<10}",
            i + 1,
            temp,
            format!("{:?}", classification.priority),
            if should_transmit { msg_size.to_string() } else { "-".to_string() },
            transmit_str
        );
    }

    println!("{}", "-".repeat(55));
    println!("\n=== Statistics ===\n");
    println!("Total measurements:     {}", temperatures.len());
    println!("Transmitted:            {}", transmitted_count);
    println!("Suppressed (P4/P5):     {}", temperatures.len() - transmitted_count);
    println!();
    println!("Raw data size:          {} bytes", total_raw_size);
    println!("Encoded size:           {} bytes", total_encoded_size);
    println!("Compression ratio:      {:.1}%", 
             (1.0 - total_encoded_size as f64 / total_raw_size as f64) * 100.0);
    println!();
    println!("Context patterns:       {}", context.pattern_count());
    println!("Context version:        {}", context.version());
    println!("Context memory:         {} bytes", context.memory_usage());
}
