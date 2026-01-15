# API Documentation

Full API documentation is available via rustdoc:

```bash
cargo doc --no-deps --open
```

## Core Types

| Type | Description |
|------|-------------|
| `Encoder` | Encodes data into messages |
| `Decoder` | Decodes messages back to data |
| `Context` | Shared state for compression |
| `Classifier` | Determines message priority |
| `RawData` | Raw sensor measurement |
| `EncodedMessage` | Compressed message |

## Fleet Types

| Type | Description |
|------|-------------|
| `FleetManager` | Manages multiple emitters |
| `FleetConfig` | Fleet configuration |
| `FleetStats` | Fleet-wide statistics |

## Security Types

| Type | Description |
|------|-------------|
| `SecurityConfig` | Security settings |
| `SecurityContext` | Runtime security state |
| `RateLimiter` | Token bucket rate limiter |
| `AuditLogger` | Audit logging trait |
| `MemoryAuditLogger` | In-memory audit log |

## Health Types

| Type | Description |
|------|-------------|
| `HealthMonitor` | System health aggregator |
| `HealthCheck` | Single health check result |
| `HealthStatus` | Health status enum |
| `HealthCheckable` | Trait for checkable components |

## Recovery Types

| Type | Description |
|------|-------------|
| `CircuitBreaker` | Circuit breaker pattern |
| `RetryStrategy` | Retry configuration |
| `DegradationLevel` | Graceful degradation levels |

## Sync Types

| Type | Description |
|------|-------------|
| `Synchronizer` | Context synchronization |
| `SyncMessage` | Sync protocol messages |
| `SyncDiff` | Incremental context diff |
