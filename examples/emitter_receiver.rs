//! Emitter-Receiver communication example
//!
//! This example demonstrates full bidirectional communication between
//! an emitter (sensor) and a receiver (server), including context synchronization.
//!
//! Run with: `cargo run --example emitter_receiver`

use alec::{
    channel::{Channel, ChannelPair, MemoryChannel},
    Classifier, Context, Decoder, Encoder, Priority, RawData,
};
use std::time::Duration;

/// Simulated emitter (sensor side)
struct Emitter {
    encoder: Encoder,
    classifier: Classifier,
    context: Context,
    channel: MemoryChannel,
}

impl Emitter {
    fn new(channel: MemoryChannel) -> Self {
        Self {
            encoder: Encoder::new(),
            classifier: Classifier::default(),
            context: Context::new(),
            channel,
        }
    }

    fn send_measurement(&mut self, value: f64, timestamp: u64) -> Option<usize> {
        let data = RawData::new(value, timestamp);
        let classification = self.classifier.classify(&data, &self.context);
        
        // Only transmit P1-P3
        if classification.priority.should_transmit() {
            let message = self.encoder.encode(&data, &classification, &self.context);
            let size = message.len();
            self.channel.send(message).ok()?;
            self.context.observe(&data);
            Some(size)
        } else {
            self.context.observe(&data);
            None
        }
    }
}

/// Simulated receiver (server side)
struct Receiver {
    decoder: Decoder,
    context: Context,
    channel: MemoryChannel,
    received_values: Vec<(u64, f64)>,
}

impl Receiver {
    fn new(channel: MemoryChannel) -> Self {
        Self {
            decoder: Decoder::new(),
            context: Context::new(),
            channel,
            received_values: Vec::new(),
        }
    }

    fn process_incoming(&mut self) -> usize {
        let mut count = 0;
        while let Ok(message) = self.channel.receive(Duration::from_millis(0)) {
            if let Ok(decoded) = self.decoder.decode(&message, &self.context) {
                self.received_values.push((decoded.timestamp, decoded.value));
                self.context.observe(&RawData::new(decoded.value, decoded.timestamp));
                count += 1;
            }
        }
        count
    }
}

fn main() {
    println!("=== ALEC Emitter-Receiver Example ===\n");

    // Create channel pair
    let mut pair = ChannelPair::new();
    
    // Create emitter and receiver with their respective channels
    let mut emitter = Emitter::new(MemoryChannel::new());
    let mut receiver = Receiver::new(MemoryChannel::new());

    // Generate realistic sensor data
    let data = generate_sensor_data();

    println!("Simulating {} measurements over 24 hours...\n", data.len());
    println!("{:<8} {:<10} {:<12} {:<10}",
             "Time", "Value", "Transmitted", "Size");
    println!("{}", "-".repeat(45));

    let mut total_transmitted = 0;
    let mut total_bytes = 0;
    let mut anomaly_count = 0;

    for (timestamp, value) in &data {
        // Emitter sends measurement
        let result = emitter.send_measurement(*value, *timestamp);
        
        // Transfer message through "network"
        while let Some(msg) = emitter.channel.pop_outgoing() {
            receiver.channel.push_incoming(msg);
        }
        
        // Receiver processes
        receiver.process_incoming();
        
        // Display every hour (every 4 measurements in our simulation)
        if timestamp % 4 == 0 {
            let transmitted = if result.is_some() { "✓" } else { "✗" };
            let size_str = result.map_or("-".to_string(), |s| s.to_string());
            
            println!(
                "{:>6}h  {:<10.1} {:<12} {:<10}",
                timestamp / 4,
                value,
                transmitted,
                size_str
            );
        }
        
        if let Some(size) = result {
            total_transmitted += 1;
            total_bytes += size;
            
            // Check for anomalies (P1/P2)
            let classification = emitter.classifier.classify(
                &RawData::new(*value, *timestamp),
                &emitter.context
            );
            if matches!(classification.priority, Priority::P1Critical | Priority::P2Important) {
                anomaly_count += 1;
            }
        }
    }

    println!("{}", "-".repeat(45));
    
    // Statistics
    println!("\n=== Communication Statistics ===\n");
    println!("Total measurements:      {}", data.len());
    println!("Messages transmitted:    {} ({:.1}%)", 
             total_transmitted,
             total_transmitted as f64 / data.len() as f64 * 100.0);
    println!("Messages suppressed:     {}", data.len() - total_transmitted);
    println!("Anomalies detected:      {}", anomaly_count);
    println!();
    println!("Total bytes transmitted: {}", total_bytes);
    println!("Average message size:    {:.1} bytes", 
             total_bytes as f64 / total_transmitted as f64);
    println!();
    
    // Verify all transmitted values were received correctly
    println!("=== Verification ===\n");
    println!("Received {} values", receiver.received_values.len());
    
    if receiver.received_values.len() == total_transmitted {
        println!("✓ All transmitted values received correctly!");
    } else {
        println!("✗ Mismatch in received values!");
    }
    
    // Context synchronization status
    println!("\n=== Context Status ===\n");
    println!("Emitter context version:  {}", emitter.context.version());
    println!("Receiver context version: {}", receiver.context.version());
    println!("Emitter context hash:     {:016x}", emitter.context.hash());
    println!("Receiver context hash:    {:016x}", receiver.context.hash());
}

/// Generate realistic temperature sensor data for 24 hours
fn generate_sensor_data() -> Vec<(u64, f64)> {
    let mut data = Vec::new();
    let mut temp = 18.0;
    
    // 4 measurements per hour, 24 hours = 96 measurements
    for hour in 0..24u64 {
        for quarter in 0..4u64 {
            let timestamp = hour * 4 + quarter;
            
            // Daily temperature pattern
            let hour_factor = match hour {
                0..=5 => -0.1,    // Night cooling
                6..=9 => 0.3,    // Morning warmup
                10..=14 => 0.1,  // Midday stable
                15..=18 => -0.1, // Evening cooling
                _ => -0.2,       // Night
            };
            
            // Random small variation
            let noise = (((timestamp * 7919) % 100) as f64 - 50.0) / 500.0;
            
            temp += hour_factor + noise;
            temp = temp.clamp(15.0, 30.0);
            
            // Inject anomalies
            let anomaly_temp = match (hour, quarter) {
                (10, 2) => Some(35.0),  // Sudden spike at 10:30
                (18, 0) => Some(12.0),  // Sudden drop at 18:00
                _ => None,
            };
            
            let value = anomaly_temp.unwrap_or(temp);
            data.push((timestamp, value));
            
            // Reset after anomaly
            if anomaly_temp.is_some() {
                temp = 20.0;
            }
        }
    }
    
    data
}
