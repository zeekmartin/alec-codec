# Security

ALEC provides security features for production deployments.

## Rate Limiting

Protect against flooding attacks:

```rust
use alec::RateLimiter;

// 100 messages/second, burst of 50
let mut limiter = RateLimiter::new(100.0, 50);

if limiter.check(emitter_id, timestamp) {
    process_message(&message);
} else {
    log_rate_limit_exceeded(emitter_id);
}
```

## Audit Logging

Track security events:

```rust
use alec::{MemoryAuditLogger, AuditEventType, Severity};

let mut logger = MemoryAuditLogger::new();

// Log events
logger.log(AuditEventType::MessageReceived, Severity::Info,
    Some(emitter_id), "Received message".into());

// Query events
let critical = logger.events_by_severity(Severity::Critical);
```

## Security Context

Combine all security features:

```rust
use alec::{SecurityConfig, SecurityContext};

let config = SecurityConfig {
    rate_limit: Some(100),
    rate_burst: Some(50),
    audit_enabled: true,
    ..Default::default()
};

let mut security = SecurityContext::new(config);

// Check rate limit and audit
if security.check_rate_limit(emitter_id, timestamp) {
    security.audit_info(emitter_id, "Message allowed");
    process_message(&message);
} else {
    security.audit_warning(emitter_id, "Rate limited");
}
```

## TLS Configuration

For encrypted transport (requires `tls` feature):

```rust
use alec::TlsConfig;

let config = TlsConfig {
    cert_path: Some("/path/to/cert.pem".into()),
    key_path: Some("/path/to/key.pem".into()),
    ca_path: Some("/path/to/ca.pem".into()),
    ..Default::default()
};

if config.validate().is_ok() {
    // Configure TLS transport
}
```
