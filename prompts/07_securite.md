# Prompt 07 — Sécurité (v1.0.0)

## Contexte

Pour une version production (v1.0.0), ALEC doit être sécurisé :
- Protection des données en transit
- Authentification des émetteurs
- Audit des opérations

## Objectif

Implémenter les mécanismes de sécurité :
1. Support TLS/DTLS
2. Authentification mTLS
3. Audit logging
4. Rate limiting

## Étapes

### 1. Créer `src/security.rs`

```rust
//! Security module for ALEC
//!
//! Provides authentication, encryption, and audit capabilities.

use std::time::{SystemTime, UNIX_EPOCH};

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Enable TLS/DTLS
    pub tls_enabled: bool,
    /// Require client certificates (mTLS)
    pub mtls_required: bool,
    /// Allowed certificate fingerprints (if mTLS)
    pub allowed_fingerprints: Vec<String>,
    /// Enable audit logging
    pub audit_enabled: bool,
    /// Rate limit (messages per second per emitter)
    pub rate_limit: Option<u32>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            tls_enabled: false,
            mtls_required: false,
            allowed_fingerprints: Vec::new(),
            audit_enabled: false,
            rate_limit: None,
        }
    }
}

impl SecurityConfig {
    /// Create a secure configuration
    pub fn secure() -> Self {
        Self {
            tls_enabled: true,
            mtls_required: true,
            allowed_fingerprints: Vec::new(),
            audit_enabled: true,
            rate_limit: Some(1000),
        }
    }
}

/// Audit event types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditEventType {
    /// New connection established
    ConnectionEstablished,
    /// Connection closed
    ConnectionClosed,
    /// Authentication succeeded
    AuthSuccess,
    /// Authentication failed
    AuthFailure,
    /// Message received
    MessageReceived,
    /// Message sent
    MessageSent,
    /// Rate limit exceeded
    RateLimitExceeded,
    /// Anomaly detected
    AnomalyDetected,
    /// Context sync
    ContextSync,
    /// Error occurred
    Error,
}

/// Audit event
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// Timestamp (Unix epoch seconds)
    pub timestamp: u64,
    /// Event type
    pub event_type: AuditEventType,
    /// Emitter ID (if applicable)
    pub emitter_id: Option<u32>,
    /// Additional details
    pub details: String,
    /// Severity (1-5, 1=info, 5=critical)
    pub severity: u8,
}

impl AuditEvent {
    pub fn new(event_type: AuditEventType, details: impl Into<String>) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            event_type,
            emitter_id: None,
            details: details.into(),
            severity: 1,
        }
    }
    
    pub fn with_emitter(mut self, emitter_id: u32) -> Self {
        self.emitter_id = Some(emitter_id);
        self
    }
    
    pub fn with_severity(mut self, severity: u8) -> Self {
        self.severity = severity.min(5).max(1);
        self
    }
}

/// Audit logger trait
pub trait AuditLogger: Send + Sync {
    /// Log an audit event
    fn log(&self, event: AuditEvent);
    
    /// Flush pending logs
    fn flush(&self);
}

/// Simple in-memory audit logger
#[derive(Debug, Default)]
pub struct MemoryAuditLogger {
    events: std::sync::Mutex<Vec<AuditEvent>>,
    max_events: usize,
}

impl MemoryAuditLogger {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
            max_events,
        }
    }
    
    pub fn events(&self) -> Vec<AuditEvent> {
        self.events.lock().unwrap().clone()
    }
    
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl AuditLogger for MemoryAuditLogger {
    fn log(&self, event: AuditEvent) {
        let mut events = self.events.lock().unwrap();
        if events.len() >= self.max_events {
            events.remove(0);
        }
        events.push(event);
    }
    
    fn flush(&self) {
        // No-op for memory logger
    }
}

/// Rate limiter using token bucket algorithm
#[derive(Debug)]
pub struct RateLimiter {
    /// Tokens per second
    rate: u32,
    /// Maximum burst size
    burst: u32,
    /// Current tokens per emitter
    tokens: std::collections::HashMap<u32, f64>,
    /// Last update time per emitter
    last_update: std::collections::HashMap<u32, u64>,
}

impl RateLimiter {
    pub fn new(rate: u32, burst: u32) -> Self {
        Self {
            rate,
            burst,
            tokens: std::collections::HashMap::new(),
            last_update: std::collections::HashMap::new(),
        }
    }
    
    /// Check if request is allowed, consumes a token if so
    pub fn check(&mut self, emitter_id: u32, now: u64) -> bool {
        let tokens = self.tokens.entry(emitter_id).or_insert(self.burst as f64);
        let last = self.last_update.entry(emitter_id).or_insert(now);
        
        // Refill tokens
        let elapsed = now.saturating_sub(*last);
        *tokens = (*tokens + elapsed as f64 * self.rate as f64).min(self.burst as f64);
        *last = now;
        
        // Check and consume
        if *tokens >= 1.0 {
            *tokens -= 1.0;
            true
        } else {
            false
        }
    }
    
    /// Reset limiter for an emitter
    pub fn reset(&mut self, emitter_id: u32) {
        self.tokens.remove(&emitter_id);
        self.last_update.remove(&emitter_id);
    }
}

/// Certificate validation result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertValidation {
    Valid,
    Expired,
    NotYetValid,
    InvalidSignature,
    UnknownIssuer,
    Revoked,
    FingerprintMismatch,
}

/// Validate a certificate fingerprint against allowed list
pub fn validate_fingerprint(fingerprint: &str, allowed: &[String]) -> CertValidation {
    if allowed.is_empty() {
        return CertValidation::Valid;
    }
    
    if allowed.contains(&fingerprint.to_string()) {
        CertValidation::Valid
    } else {
        CertValidation::FingerprintMismatch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rate_limiter() {
        let mut limiter = RateLimiter::new(10, 5);
        
        // First 5 should pass (burst)
        for _ in 0..5 {
            assert!(limiter.check(1, 0));
        }
        
        // 6th should fail
        assert!(!limiter.check(1, 0));
        
        // After 1 second, should have 10 new tokens
        assert!(limiter.check(1, 1));
    }
    
    #[test]
    fn test_audit_logger() {
        let logger = MemoryAuditLogger::new(100);
        
        logger.log(AuditEvent::new(
            AuditEventType::ConnectionEstablished,
            "New connection"
        ).with_emitter(42));
        
        let events = logger.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].emitter_id, Some(42));
    }
    
    #[test]
    fn test_fingerprint_validation() {
        let allowed = vec!["abc123".to_string(), "def456".to_string()];
        
        assert_eq!(
            validate_fingerprint("abc123", &allowed),
            CertValidation::Valid
        );
        
        assert_eq!(
            validate_fingerprint("unknown", &allowed),
            CertValidation::FingerprintMismatch
        );
    }
}
```

### 2. Créer `src/tls.rs` (wrapper TLS)

```rust
//! TLS wrapper for secure channels
//!
//! Provides TLS and DTLS support for ALEC channels.

#[cfg(feature = "tls")]
use rustls::{ClientConfig, ServerConfig};

/// TLS configuration builder
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Path to certificate file
    pub cert_path: Option<String>,
    /// Path to private key file
    pub key_path: Option<String>,
    /// Path to CA certificate for verification
    pub ca_path: Option<String>,
    /// Server name for verification
    pub server_name: Option<String>,
    /// Allow self-signed certificates
    pub allow_self_signed: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_path: None,
            key_path: None,
            ca_path: None,
            server_name: None,
            allow_self_signed: false,
        }
    }
}

impl TlsConfig {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn with_cert(mut self, cert_path: &str, key_path: &str) -> Self {
        self.cert_path = Some(cert_path.to_string());
        self.key_path = Some(key_path.to_string());
        self
    }
    
    pub fn with_ca(mut self, ca_path: &str) -> Self {
        self.ca_path = Some(ca_path.to_string());
        self
    }
    
    pub fn with_server_name(mut self, name: &str) -> Self {
        self.server_name = Some(name.to_string());
        self
    }
}

// Note: Actual TLS implementation would use rustls or native-tls
// This is a placeholder for the interface
```

### 3. Intégrer la sécurité dans FleetManager

```rust
impl FleetManager {
    /// Process message with security checks
    pub fn process_message_secure(
        &mut self,
        emitter_id: EmitterId,
        message: &EncodedMessage,
        timestamp: u64,
        security: &mut SecurityContext,
    ) -> Result<ProcessedMessage> {
        // Rate limiting
        if let Some(ref mut limiter) = security.rate_limiter {
            if !limiter.check(emitter_id, timestamp) {
                security.audit(AuditEvent::new(
                    AuditEventType::RateLimitExceeded,
                    format!("Emitter {} exceeded rate limit", emitter_id)
                ).with_severity(3));
                return Err(AlecError::Channel(ChannelError::RateLimited {
                    retry_after_ms: 1000,
                }));
            }
        }
        
        // Audit message reception
        security.audit(AuditEvent::new(
            AuditEventType::MessageReceived,
            format!("Message from emitter {}", emitter_id)
        ).with_emitter(emitter_id));
        
        // Process normally
        let result = self.process_message(emitter_id, message, timestamp)?;
        
        // Audit anomalies
        if result.is_cross_fleet_anomaly {
            security.audit(AuditEvent::new(
                AuditEventType::AnomalyDetected,
                format!("Cross-fleet anomaly from emitter {}", emitter_id)
            ).with_emitter(emitter_id).with_severity(4));
        }
        
        Ok(result)
    }
}

/// Security context for a session
pub struct SecurityContext {
    pub config: SecurityConfig,
    pub rate_limiter: Option<RateLimiter>,
    pub audit_logger: Option<Box<dyn AuditLogger>>,
}

impl SecurityContext {
    pub fn audit(&self, event: AuditEvent) {
        if let Some(ref logger) = self.audit_logger {
            logger.log(event);
        }
    }
}
```

### 4. Feature flags dans Cargo.toml

```toml
[features]
default = []
tls = ["rustls", "webpki-roots"]
full = ["tls"]

[dependencies]
rustls = { version = "0.21", optional = true }
webpki-roots = { version = "0.25", optional = true }
```

## Livrables

- [ ] `src/security.rs` — Module de sécurité
- [ ] `src/tls.rs` — Wrapper TLS (interface)
- [ ] `SecurityConfig` et `SecurityContext`
- [ ] `AuditLogger` trait + `MemoryAuditLogger`
- [ ] `RateLimiter` avec token bucket
- [ ] Intégration avec `FleetManager`
- [ ] Feature flags pour TLS
- [ ] Tests (au moins 5)

## Critères de succès

```bash
cargo test security
cargo build --features tls  # Doit compiler
```

## Prochaine étape

→ `08_robustesse.md` (v1.0.0 - Robustesse)
