# Changelog

All notable changes to ALEC are documented here.

## [1.0.0] - 2024

### Added
- Health monitoring (`HealthMonitor`, `HealthCheckable`)
- Recovery mechanisms (`CircuitBreaker`, `RetryStrategy`)
- Graceful degradation (`DegradationLevel`)
- Stress tests for performance validation
- Security module (`SecurityContext`, `RateLimiter`, `AuditLogger`)
- TLS/DTLS interfaces (`TlsConfig`, `DtlsConfig`)

### Changed
- Improved documentation with examples
- Better error messages

## [0.4.0] - 2024

### Added
- Fleet mode for multi-emitter scenarios
- `FleetManager` with per-emitter context
- Cross-fleet anomaly detection
- `FleetStats` for fleet-wide statistics

## [0.3.0] - 2024

### Added
- Automatic context synchronization
- `Synchronizer` state machine
- `SyncMessage` protocol
- Diff-based incremental sync

## [0.2.0] - 2024

### Added
- Evolving context with pattern pruning
- EMA-based predictions
- Pattern scoring and reordering
- `EvolutionConfig` for customization

## [0.1.0] - 2024

### Added
- Initial release
- Core encoding/decoding
- Priority classification (P1-P5)
- Delta, repeated, and raw encoding
- Checksum support (CRC32)
- Metrics collection
- Channel abstraction
