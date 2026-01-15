# Basic Concepts

This page explains the core concepts you need to understand ALEC.

## Architecture Overview

```
┌─────────────┐                              ┌─────────────┐
│   Emitter   │                              │  Receiver   │
├─────────────┤                              ├─────────────┤
│  RawData    │──►┌────────────┐             │             │
│             │   │ Classifier │             │             │
│  Context ◄──┼──►├────────────┤   Network   │  Context    │
│             │   │  Encoder   │────────────►│  Decoder    │
│             │   └────────────┘             │             │
└─────────────┘                              └─────────────┘
```

## Core Components

### RawData

`RawData` represents a single sensor measurement:

```rust
pub struct RawData {
    pub value: f64,       // The measurement value
    pub timestamp: u64,   // Timestamp (any unit)
    pub source_id: u32,   // Source identifier (default: 0)
}
```

### Encoder

The `Encoder` converts `RawData` into compact `EncodedMessage`:

- Maintains sequence numbers for ordering
- Selects optimal encoding strategy
- Optional checksum generation

### Decoder

The `Decoder` converts `EncodedMessage` back to `RawData`:

- Verifies message integrity
- Tracks sequence numbers for loss detection
- Optional checksum verification

### Context

The `Context` is the shared state between encoder and decoder:

- Pattern dictionary for compression
- Prediction model for delta encoding
- Source statistics for anomaly detection

**Important**: Both sides must have synchronized contexts!

### Classifier

The `Classifier` determines message priority based on:

- Value anomalies (sudden changes)
- Threshold violations (out of range)
- Prediction confidence

## Encoding Strategies

ALEC automatically selects the best encoding:

| Strategy | Size | When Used |
|----------|------|-----------|
| Repeated | 1 byte | Value unchanged from last |
| Delta | 1-4 bytes | Value close to prediction |
| Raw | 8 bytes | Unpredictable values |

### Delta Encoding

Delta encoding transmits the difference from a predicted value:

```
Predicted: 22.0
Actual:    22.1
Delta:     0.1 → encoded as small integer
```

The prediction comes from an Exponential Moving Average (EMA) model.

### Repeated Encoding

When the value hasn't changed:

```
Previous: 22.0
Current:  22.0
Encoding: "Same as before" (1 byte)
```

## Priority Classification

Messages are classified into 5 levels:

| Level | Name | Criteria |
|-------|------|----------|
| P1 | Critical | Threshold violation |
| P2 | Important | Significant anomaly |
| P3 | Normal | Standard data |
| P4 | Deferred | Low-variance data |
| P5 | Disposable | Highly predictable |

Classification is automatic based on:

1. **Threshold check**: Is value in safe range?
2. **Anomaly score**: How far from prediction?
3. **Variance check**: Is source stable?

## Context Synchronization

Both encoder and decoder must have identical contexts:

```rust
// On encoder side
context_encoder.observe(&data);

// On decoder side (after receiving)
context_decoder.observe(&decoded);
```

If contexts diverge, you'll get decode errors. See [Synchronization](../guide/synchronization.md) for automatic sync.

## Memory Model

ALEC is designed for constrained environments:

| Component | Default Memory |
|-----------|----------------|
| Context | 64 KB |
| Encoder | <1 KB |
| Decoder | <1 KB |
| Patterns | Up to 65535 |

Context evolution automatically prunes old patterns to stay within limits.

## Next Steps

- [Encoding Data](../guide/encoding.md) - Deep dive into encoding
- [Context Management](../guide/context.md) - Advanced context usage
- [Synchronization](../guide/synchronization.md) - Keep contexts in sync
