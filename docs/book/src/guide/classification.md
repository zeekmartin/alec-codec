# Classification

ALEC's classifier determines message priority based on data characteristics.

## Priority Levels

| Priority | Name | Description |
|----------|------|-------------|
| P1 | Critical | Immediate transmission, ACK required |
| P2 | Important | Immediate transmission |
| P3 | Normal | Standard transmission |
| P4 | Deferred | Stored locally, sent on request |
| P5 | Disposable | Never sent spontaneously |

## Basic Classification

```rust
use alec::{Classifier, Context, RawData};

let classifier = Classifier::default();
let context = Context::new();

let data = RawData::new(22.5, 0);
let classification = classifier.classify(&data, &context);

println!("Priority: {:?}", classification.priority);
println!("Reason: {:?}", classification.reason);
```

## Custom Thresholds

Configure thresholds for your application:

```rust
use alec::classifier::ClassifierConfig;

let config = ClassifierConfig {
    critical_low: -50.0,    // Below this is critical
    critical_high: 100.0,   // Above this is critical
    anomaly_threshold: 3.0, // Standard deviations
    ..Default::default()
};

let classifier = Classifier::with_config(config);
```

## Classification Reasons

The classifier provides a reason for each classification:

- `Normal`: Value within expected range
- `ThresholdViolation`: Value outside critical range
- `Anomaly`: Value significantly different from prediction
- `NoPrediction`: No history available (first message)

## Using Classification

Priority affects transmission decisions:

```rust
use alec::protocol::Priority;

if classification.priority.should_transmit() {
    // P1, P2, P3 - send immediately
    transmit(&message);
} else {
    // P4, P5 - store for later
    store(&message);
}
```
