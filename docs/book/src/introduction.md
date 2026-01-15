# Introduction

**ALEC** (Adaptive Lazy Evolving Compression) is a high-performance compression codec designed specifically for IoT and embedded systems.

## What is ALEC?

ALEC is a Rust library that provides:

- **Adaptive compression** - Automatically selects the best encoding strategy
- **Lazy evaluation** - Only compresses when beneficial
- **Evolving context** - Learns patterns over time for better compression
- **Priority classification** - Smart message prioritization (P1-P5)

## Why ALEC?

Traditional compression algorithms (gzip, lz4, etc.) are designed for bulk data and don't work well with small sensor messages. ALEC is specifically designed for:

| Feature | Traditional | ALEC |
|---------|-------------|------|
| Message size | Large files | Small messages (8-64 bytes) |
| Compression | Batch | Per-message |
| Learning | None | Continuous |
| Priority | None | Built-in (P1-P5) |
| Memory | High | Low (~64KB default) |

## Key Features

### Compression Strategies

ALEC automatically selects from multiple encoding strategies:

- **Delta encoding**: For values close to predictions (1-4 bytes)
- **Repeated encoding**: For unchanged values (1 byte)
- **Pattern encoding**: For recurring patterns
- **Raw encoding**: Fallback for unpredictable data

### Priority Classification

Messages are classified into 5 priority levels:

| Priority | Name | Description |
|----------|------|-------------|
| P1 | Critical | Immediate transmission, ACK required |
| P2 | Important | Immediate transmission |
| P3 | Normal | Standard transmission |
| P4 | Deferred | Stored locally, sent on request |
| P5 | Disposable | Never sent spontaneously |

### Fleet Mode

Manage multiple emitters with:

- Per-emitter context tracking
- Cross-fleet anomaly detection
- Automatic stale emitter cleanup

### Security

Production-ready security features:

- Rate limiting (token bucket algorithm)
- Audit logging
- TLS/DTLS support (optional)

## Performance

ALEC is designed for high performance:

- **Encoding**: >100k messages/second
- **Roundtrip**: >50k messages/second
- **Fleet mode**: >10k messages/second
- **Memory**: <64KB per context

## Getting Started

Ready to start using ALEC? Continue to [Installation](./getting-started/installation.md).
