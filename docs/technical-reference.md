# ALEC Technical Reference: Delta Encoding, Min_Value, and Signal Handling

## Overview

This document explains ALEC's core encoding mechanisms and how it handles edge cases like signal loss, value ranges, and anomaly detection.

---

## 1. Delta Encoding Fundamentals

### How Delta Encoding Works

ALEC transmits the **difference** (delta) between consecutive values rather than absolute values.

```
Raw values:     22.5  →  22.5  →  22.6  →  22.5  →  22.4
Deltas:         [22.5]    0.0     +0.1    -0.1    -0.1
                 ↑
            First value sent as absolute
```

### Why Delta Encoding is Effective for IoT

1. **Sensor readings change slowly** - Temperature doesn't jump from 22°C to 50°C instantly
2. **Most deltas are small** - 90%+ of readings have delta < 1.0
3. **Small numbers need fewer bits** - Delta of 0 needs 2 bits, while 22.5 needs 32 bits

### Delta Quantization

ALEC uses configurable precision (default: 0.1 units):

```
delta_quantized = round(actual_delta × quantization_factor)

For temperature (0.1°C precision):
  actual_delta = 22.53 - 22.50 = 0.03
  delta_quantized = round(0.03 × 10) = 0  → Encoded as "same value"

For vibration (0.01 mm/s precision):
  actual_delta = 2.534 - 2.528 = 0.006
  delta_quantized = round(0.006 × 100) = 1 → Encoded as "tiny delta"
```

---

## 2. Encoding Strategies by Delta Size

ALEC selects the most efficient encoding based on delta magnitude:

| Delta Range | Encoding | Bits Used | Example |
|-------------|----------|-----------|---------|
| Δ = 0 | Same value flag | 2 bits | Temperature stable |
| |Δ| < 8 | Tiny delta | 4-6 bits | Normal sensor drift |
| |Δ| < 64 | Small delta | 8-10 bits | Moderate change |
| |Δ| < 512 | Medium delta | 12-14 bits | Significant change |
| |Δ| ≥ 512 | Full value | 32 bits | Anomaly or reset |

### Varint Encoding

ALEC uses variable-length integer encoding (similar to Protocol Buffers):

```
Value 0:        [0]                     → 1 bit
Value 1-7:      [1][xxx]                → 4 bits  
Value 8-63:     [10][xxxxxx]            → 8 bits
Value 64-511:   [110][xxxxxxxxx]        → 12 bits
Value 512-4095: [1110][xxxxxxxxxxxx]    → 16 bits
```

This ensures small deltas use minimal bandwidth.

---

## 3. Min_Value and Value Ranges

### Context-Aware Bounds

ALEC maintains statistics about observed values:

```rust
struct SourceStats {
    min_observed: f64,      // Minimum value seen
    max_observed: f64,      // Maximum value seen
    mean: f64,              // Running average
    std_dev: f64,           // Standard deviation
    last_n_values: [f64; 10], // Recent history
}
```

### Dynamic Range Encoding

When the encoder knows the value range, it can use offset encoding:

```
Observed range: [20.0, 25.0]
New value: 22.5

Without range knowledge:
  → Send full float: 32 bits

With range knowledge:
  offset = (22.5 - 20.0) / (25.0 - 20.0) = 0.5
  → Send normalized offset: 8-10 bits
```

### Out-of-Range Handling

When a value exceeds known bounds:

1. **Soft boundary** (< 2× expected range): Expand bounds, use extended encoding
2. **Hard boundary** (> 2× expected range): Flag as anomaly, send full value + alert

```
Expected range: [20.0, 25.0]
Value received: 28.0

28.0 is 1.2× max → Soft boundary exceeded
  → Extend max to 28.0
  → Encode as medium delta: 12-14 bits
  → Context updated for future

Value received: 85.0

85.0 is 3.4× max → Hard boundary exceeded  
  → Flag as ANOMALY (Priority P1/P2)
  → Send full value: 32 bits + priority flag
  → Alert decoder of potential issue
```

---

## 4. Signal Loss and Recovery

### Detecting Signal Loss

ALEC uses sequence numbers to detect gaps:

```
Message format:
┌─────────┬──────────┬─────────────┬───────────┐
│ Seq# (8)│ Flags (4)│ Delta (var) │ CRC (8)   │
└─────────┴──────────┴─────────────┴───────────┘

Decoder tracking:
  expected_seq = 42
  received_seq = 45
  → GAP DETECTED: 3 messages lost
```

### Recovery Mechanisms

#### Mechanism 1: Context Drift Tolerance

Small gaps (1-3 messages) are tolerated with increased uncertainty:

```
Message 41: temperature = 22.5
Message 42: LOST
Message 43: LOST  
Message 44: delta = +0.3

Decoder logic:
  - Apply delta to last known value: 22.5 + 0.3 = 22.8
  - Increase uncertainty bounds
  - Flag as "potentially stale"
```

#### Mechanism 2: Sync Points

Every N messages (default: 100), ALEC sends a full sync:

```
Sync message:
┌─────────┬─────────────────┬──────────────────┐
│ SYNC    │ Full value (32) │ Context hash (16)│
└─────────┴─────────────────┴──────────────────┘

On receive:
  - Reset decoder value to absolute
  - Verify context hash matches
  - Clear uncertainty flags
```

#### Mechanism 3: Explicit Resync Request

If context divergence is detected, decoder can request resync:

```
Encoder                    Decoder
   │                          │
   │──── delta +0.1 ────────▶│
   │                          │ (detects context mismatch)
   │◀──── RESYNC_REQ ────────│
   │                          │
   │──── SYNC + full ctx ───▶│
   │                          │ (context restored)
   │──── delta +0.0 ────────▶│
```

### Context Hash Verification

Both encoder and decoder maintain a context hash:

```rust
fn context_hash(&self) -> u16 {
    let mut hasher = XxHash::default();
    
    // Hash pattern dictionary
    for (value, index) in &self.patterns {
        hasher.write_f64(*value);
        hasher.write_u8(*index);
    }
    
    // Hash recent values
    for value in &self.recent_values {
        hasher.write_f64(*value);
    }
    
    // Hash statistics
    hasher.write_f64(self.stats.mean);
    hasher.write_f64(self.stats.std_dev);
    
    hasher.finish() as u16
}
```

If hashes diverge, a resync is triggered automatically.

---

## 5. Anomaly Detection via Delta

### Anomaly Classification

Large deltas indicate potential anomalies:

```
Delta Threshold Configuration:
  normal_threshold: 3 × std_dev
  warning_threshold: 6 × std_dev
  critical_threshold: 10 × std_dev

Example (temperature sensor, std_dev = 0.2°C):
  |delta| < 0.6°C  → NORMAL (P3)
  |delta| < 1.2°C  → WARNING (P2)
  |delta| ≥ 2.0°C  → CRITICAL (P1)
```

### Priority-Based Handling

```
P1 CRITICAL:
  - Immediate transmission
  - Full value + context
  - Requires acknowledgment
  
P2 IMPORTANT:
  - Immediate transmission
  - Delta encoding still used
  - No acknowledgment required

P3 NORMAL:
  - Standard transmission
  - Delta encoding
  - Batching allowed

P4 DEFERRED:
  - Transmission on request only
  - May be aggregated

P5 DISPOSABLE:
  - Context update only
  - Not transmitted
```

---

## 6. Pattern Dictionary Management

### Dictionary Building

ALEC maintains a dictionary of frequently occurring values:

```rust
struct PatternDictionary {
    entries: HashMap<QuantizedValue, u8>,  // value → index
    frequency: HashMap<QuantizedValue, u32>, // value → occurrence count
    max_entries: usize,  // Default: 64
}
```

### Dictionary Update Rules

1. **New frequent value**: Add to dictionary if space available
2. **Dictionary full**: Replace least frequent entry if new value is more common
3. **Value decay**: Reduce frequency counts periodically to adapt to changing patterns

```
Initial state:
  [22.5: 150 occurrences]
  [22.6: 120 occurrences]
  [22.4: 80 occurrences]
  ...

After 1000 new samples with 22.7 appearing 200 times:
  [22.7: 200 occurrences]  ← Promoted
  [22.5: 150 occurrences]
  [22.6: 120 occurrences]
  ...
```

### Dictionary Synchronization

On preload, the dictionary is shared between encoder and decoder:

```
Preload file format:
┌──────────────────────────────────────────┐
│ Version (2 bytes)                        │
│ Entry count (2 bytes)                    │
│ Entry 0: value (4 bytes) + index (1)     │
│ Entry 1: value (4 bytes) + index (1)     │
│ ...                                      │
│ Statistics: min, max, mean, std (32)     │
│ Checksum (4 bytes)                       │
└──────────────────────────────────────────┘
```

---

## 7. Implementation Notes

### Thread Safety

- Context is NOT thread-safe by default
- Use `Arc<Mutex<Context>>` for concurrent access
- Or maintain separate contexts per source

### Memory Usage

```
Per-source context:
  Pattern dictionary:  64 entries × 5 bytes  = 320 bytes
  Recent values:       50 values × 8 bytes   = 400 bytes
  Statistics:          6 values × 8 bytes    = 48 bytes
  Overhead:                                  = ~32 bytes
  TOTAL per source:                          ≈ 800 bytes
```

### Precision Recommendations

| Sensor Type | Recommended Precision | Quantization Factor |
|-------------|----------------------|---------------------|
| Temperature | 0.1°C | 10 |
| Humidity | 1% | 1 |
| Voltage | 0.1V | 10 |
| Current | 0.01A | 100 |
| Vibration | 0.01 mm/s | 100 |
| GPS | 0.0001° | 10000 |

---

## Summary

ALEC's delta encoding achieves high compression by:

1. **Exploiting temporal correlation** - Consecutive readings are similar
2. **Variable-length encoding** - Small deltas use fewer bits
3. **Pattern learning** - Frequent values get short codes
4. **Adaptive bounds** - Range knowledge enables offset encoding
5. **Graceful degradation** - Signal loss is detected and recovered

The key insight: **IoT data is not random**. By modeling its structure, ALEC achieves 10-30× compression where generic codecs achieve only 2-5×.
