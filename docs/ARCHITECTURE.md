# ALEC Architecture

## Overview

ALEC (Adaptive Lazy Evolving Compression) is a suite of tools for IoT data compression and observability:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ALEC Ecosystem                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────┐    ┌─────────────────────────────────────────────────┐    │
│  │             │    │                ALEC Gateway                      │    │
│  │  ALEC Codec │    │  ┌─────────────────────────────────────────┐   │    │
│  │             │    │  │            Metrics Module               │   │    │
│  │  - Encoder  │    │  │  - Signal entropy (H_i, TC, H_joint)   │   │    │
│  │  - Decoder  │    │  │  - Payload entropy                      │   │    │
│  │  - Context  │    │  │  - Resilience index R                   │   │    │
│  │  - Preloads │    │  │  - Criticality ranking                  │   │    │
│  │             │    │  └─────────────────────────────────────────┘   │    │
│  └─────────────┘    │                                                 │    │
│         │           │  - Channel management                           │    │
│         │           │  - Frame aggregation                            │    │
│         │           │  - Priority scheduling                          │    │
│         ▼           └─────────────────────────────────────────────────┘    │
│  ┌─────────────┐                          │                                 │
│  │  ALEC FFI   │                          │ MetricsSnapshot                 │
│  │  (C/C++)    │                          ▼                                 │
│  └─────────────┘    ┌─────────────────────────────────────────────────┐    │
│                     │              ALEC Complexity                     │    │
│                     │  - Baseline learning                             │    │
│                     │  - Delta / Z-score computation                   │    │
│                     │  - S-lite structure analysis                     │    │
│                     │  - Anomaly event detection                       │    │
│                     └─────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Design Principles

### 1. Separation of Concerns

| Crate | Responsibility | Can be used standalone |
|-------|---------------|------------------------|
| `alec` | Compression/decompression | Yes |
| `alec-gateway` | Multi-sensor orchestration | Yes |
| `alec-complexity` | Temporal analysis | Yes |
| `alec-ffi` | C/C++ bindings | Yes |

### 2. Feature Flags

```toml
# alec-gateway
[features]
metrics = ["nalgebra", "serde", "serde_json"]

# alec-complexity
[features]
gateway = ["alec-gateway/metrics"]
```

### 3. Non-Blocking Observability

Metrics and Complexity modules are **passive observers**:
- Never block the main data path
- Failures in metrics don't affect compression
- All monitoring is opt-in via configuration

### 4. Graceful Degradation

```
Full data → Full metrics
├── Missing channels → Reduced TC/H_joint (still valid)
├── Single channel → Only H_i, no correlation
├── Metrics disabled → Zero overhead
└── Complexity disabled → No baseline/events
```

## Architectural Decision Records (ADRs)

### ADR-001: Metrics as Gateway Feature Flag

**Decision:** Metrics is a feature of `alec-gateway`, not a separate crate.

**Rationale:**
- Metrics requires tight integration with frame processing
- No valid use case for Metrics without Gateway
- Simplifies dependency management

**Consequences:**
- Gateway crate is larger when metrics enabled
- Clear compile-time boundary via feature flag

### ADR-002: Complexity as Standalone Crate

**Decision:** Complexity is a separate crate (`alec-complexity`).

**Rationale:**
- Complexity can consume metrics from multiple sources
- Enables independent licensing and sales
- Allows use with non-ALEC data sources

**Consequences:**
- Additional crate to maintain
- Requires input adapter pattern

### ADR-003: InputSnapshot as Unified Input

**Decision:** Complexity uses a unified `InputSnapshot` type, not `MetricsSnapshot` directly.

**Rationale:**
- Decouples Complexity from Gateway internals
- Enables generic JSON input
- Future: Prometheus, InfluxDB, etc.

### ADR-004: Baseline Frozen by Default

**Decision:** Baseline defaults to `Frozen` mode after lock.

**Rationale:**
- Deterministic behavior for testing
- Prevents drift in long-running systems
- EMA/Rolling available as options

## Data Flow

See [diagrams/data-flow.mermaid](diagrams/data-flow.mermaid) for the complete data flow diagram.

```
Sensor Values → Gateway.push() → Channel Buffer
                                      │
                                      ▼
                             Gateway.flush()
                                      │
                          ┌───────────┴───────────┐
                          │                       │
                          ▼                       ▼
                    Frame Builder          Metrics Engine
                          │                       │
                          ▼                       ▼
                   ALEC Encoder            MetricsSnapshot
                          │                       │
                          ▼                       ▼
                  Compressed Frame        Complexity Engine
                                                  │
                                                  ▼
                                         ComplexitySnapshot
```

## Module Dependencies

```
┌──────────────────┐
│   alec (core)    │
└────────┬─────────┘
         │ optional
         ▼
┌──────────────────┐
│  alec-gateway    │
│  ┌────────────┐  │
│  │  metrics   │  │ ← feature flag
│  └────────────┘  │
└────────┬─────────┘
         │ optional (feature: gateway)
         ▼
┌──────────────────┐
│ alec-complexity  │
└──────────────────┘
```

## Crate Overview

### alec (Core Codec)

The foundation crate providing compression and decompression:

- **Encoder**: Adaptive compression with context learning
- **Decoder**: Decompression with context reconstruction
- **Context**: Shared dictionary evolving over time
- **Preloads**: Pre-trained contexts for faster startup

### alec-gateway

Multi-sensor orchestration layer:

- **Channel Management**: Add/remove sensors dynamically
- **Priority Scheduling**: P1-P5 priority levels
- **Frame Aggregation**: Combine channels into single transmission
- **Metrics** (feature-gated): Entropy and resilience computation

### alec-complexity

Temporal analysis and anomaly detection:

- **Baseline Learning**: Statistical summary of nominal operation
- **Delta/Z-Score Computation**: Deviation from baseline
- **S-lite Structure Analysis**: Pairwise sensor dependencies
- **Anomaly Event Detection**: Notifications with persistence/cooldown

### alec-ffi

C/C++ foreign function interface for embedded systems.

## Performance Characteristics

| Operation | Typical Latency | Memory |
|-----------|----------------|--------|
| Single value encode | < 1ms | ~2KB per channel |
| Flush 10 channels | < 5ms | ~20KB |
| Metrics computation | < 10ms | ~100KB window |
| Complexity snapshot | < 2ms | ~50KB baseline |

## Security Considerations

- No network operations in core codec
- Metrics/Complexity are passive (read-only)
- Preloads are local files only
- See [SECURITY.md](../SECURITY.md) for full policy
