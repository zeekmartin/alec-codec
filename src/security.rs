// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.


//! Security module for ALEC
//!
//! Provides authentication, encryption, and audit capabilities:
//! - Security configuration
//! - Audit logging with configurable backends
//! - Rate limiting using token bucket algorithm
//! - Certificate validation helpers

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Security configuration
#[derive(Debug, Clone, Default)]
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
    /// Rate limit burst size
    pub rate_burst: Option<u32>,
}

impl SecurityConfig {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a secure configuration with sensible defaults
    pub fn secure() -> Self {
        Self {
            tls_enabled: true,
            mtls_required: true,
            allowed_fingerprints: Vec::new(),
            audit_enabled: true,
            rate_limit: Some(1000),
            rate_burst: Some(100),
        }
    }

    /// Create configuration with rate limiting only
    pub fn with_rate_limit(rate: u32, burst: u32) -> Self {
        Self {
            rate_limit: Some(rate),
            rate_burst: Some(burst),
            ..Default::default()
        }
    }

    /// Create configuration with audit logging only
    pub fn with_audit() -> Self {
        Self {
            audit_enabled: true,
            ..Default::default()
        }
    }

    /// Add an allowed certificate fingerprint
    pub fn allow_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.allowed_fingerprints.push(fingerprint.into());
        self
    }
}

/// Audit event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Context sync operation
    ContextSync,
    /// Error occurred
    Error,
    /// Configuration changed
    ConfigChanged,
    /// Emitter registered
    EmitterRegistered,
    /// Emitter removed
    EmitterRemoved,
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditEventType::ConnectionEstablished => write!(f, "CONNECTION_ESTABLISHED"),
            AuditEventType::ConnectionClosed => write!(f, "CONNECTION_CLOSED"),
            AuditEventType::AuthSuccess => write!(f, "AUTH_SUCCESS"),
            AuditEventType::AuthFailure => write!(f, "AUTH_FAILURE"),
            AuditEventType::MessageReceived => write!(f, "MESSAGE_RECEIVED"),
            AuditEventType::MessageSent => write!(f, "MESSAGE_SENT"),
            AuditEventType::RateLimitExceeded => write!(f, "RATE_LIMIT_EXCEEDED"),
            AuditEventType::AnomalyDetected => write!(f, "ANOMALY_DETECTED"),
            AuditEventType::ContextSync => write!(f, "CONTEXT_SYNC"),
            AuditEventType::Error => write!(f, "ERROR"),
            AuditEventType::ConfigChanged => write!(f, "CONFIG_CHANGED"),
            AuditEventType::EmitterRegistered => write!(f, "EMITTER_REGISTERED"),
            AuditEventType::EmitterRemoved => write!(f, "EMITTER_REMOVED"),
        }
    }
}

/// Severity levels for audit events
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational event
    Info = 1,
    /// Low importance
    Low = 2,
    /// Medium importance
    Medium = 3,
    /// High importance
    High = 4,
    /// Critical event
    Critical = 5,
}

impl From<u8> for Severity {
    fn from(value: u8) -> Self {
        match value {
            1 => Severity::Info,
            2 => Severity::Low,
            3 => Severity::Medium,
            4 => Severity::High,
            5.. => Severity::Critical,
            _ => Severity::Info,
        }
    }
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
    /// Severity level
    pub severity: Severity,
}

impl AuditEvent {
    /// Create a new audit event with current timestamp
    pub fn new(event_type: AuditEventType, details: impl Into<String>) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            event_type,
            emitter_id: None,
            details: details.into(),
            severity: Severity::Info,
        }
    }

    /// Create audit event with specific timestamp
    pub fn with_timestamp(
        event_type: AuditEventType,
        details: impl Into<String>,
        timestamp: u64,
    ) -> Self {
        Self {
            timestamp,
            event_type,
            emitter_id: None,
            details: details.into(),
            severity: Severity::Info,
        }
    }

    /// Set the emitter ID
    pub fn with_emitter(mut self, emitter_id: u32) -> Self {
        self.emitter_id = Some(emitter_id);
        self
    }

    /// Set the severity level
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Set severity from u8 (clamped to 1-5)
    pub fn with_severity_level(mut self, level: u8) -> Self {
        self.severity = Severity::from(level.clamp(1, 5));
        self
    }

    /// Format as a log line
    pub fn to_log_line(&self) -> String {
        let emitter = self
            .emitter_id
            .map(|id| format!(" emitter={}", id))
            .unwrap_or_default();
        format!(
            "[{}] {:?} {}{} - {}",
            self.timestamp, self.severity, self.event_type, emitter, self.details
        )
    }
}

/// Audit logger trait
pub trait AuditLogger: Send + Sync {
    /// Log an audit event
    fn log(&self, event: AuditEvent);

    /// Flush pending logs
    fn flush(&self);

    /// Get events matching a filter (optional)
    fn query(&self, _filter: &AuditFilter) -> Vec<AuditEvent> {
        Vec::new()
    }
}

/// Filter for querying audit events
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    /// Filter by event type
    pub event_type: Option<AuditEventType>,
    /// Filter by emitter ID
    pub emitter_id: Option<u32>,
    /// Filter by minimum severity
    pub min_severity: Option<Severity>,
    /// Filter by time range (start)
    pub from_timestamp: Option<u64>,
    /// Filter by time range (end)
    pub to_timestamp: Option<u64>,
}

impl AuditFilter {
    /// Check if an event matches this filter
    pub fn matches(&self, event: &AuditEvent) -> bool {
        if let Some(et) = self.event_type {
            if event.event_type != et {
                return false;
            }
        }
        if let Some(eid) = self.emitter_id {
            if event.emitter_id != Some(eid) {
                return false;
            }
        }
        if let Some(min_sev) = self.min_severity {
            if event.severity < min_sev {
                return false;
            }
        }
        if let Some(from) = self.from_timestamp {
            if event.timestamp < from {
                return false;
            }
        }
        if let Some(to) = self.to_timestamp {
            if event.timestamp > to {
                return false;
            }
        }
        true
    }
}

/// Simple in-memory audit logger
#[derive(Debug)]
pub struct MemoryAuditLogger {
    events: Mutex<Vec<AuditEvent>>,
    max_events: usize,
}

impl Default for MemoryAuditLogger {
    fn default() -> Self {
        Self::new(10000)
    }
}

impl MemoryAuditLogger {
    /// Create a new memory logger with specified capacity
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Mutex::new(Vec::with_capacity(max_events.min(1000))),
            max_events,
        }
    }

    /// Get all stored events
    pub fn events(&self) -> Vec<AuditEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Get event count
    pub fn len(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.events.lock().unwrap().is_empty()
    }

    /// Clear all events
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }

    /// Get events by type
    pub fn events_by_type(&self, event_type: AuditEventType) -> Vec<AuditEvent> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect()
    }

    /// Get events for an emitter
    pub fn events_by_emitter(&self, emitter_id: u32) -> Vec<AuditEvent> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.emitter_id == Some(emitter_id))
            .cloned()
            .collect()
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

    fn query(&self, filter: &AuditFilter) -> Vec<AuditEvent> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| filter.matches(e))
            .cloned()
            .collect()
    }
}

/// Rate limiter using token bucket algorithm
#[derive(Debug)]
pub struct RateLimiter {
    /// Tokens per second
    rate: f64,
    /// Maximum burst size
    burst: f64,
    /// Current tokens per emitter
    tokens: HashMap<u32, f64>,
    /// Last update time per emitter (in seconds)
    last_update: HashMap<u32, u64>,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `rate` - Tokens (requests) allowed per second
    /// * `burst` - Maximum tokens that can accumulate
    pub fn new(rate: u32, burst: u32) -> Self {
        Self {
            rate: rate as f64,
            burst: burst as f64,
            tokens: HashMap::new(),
            last_update: HashMap::new(),
        }
    }

    /// Check if a request is allowed for an emitter
    ///
    /// Returns true if allowed (consumes a token), false if rate limited
    pub fn check(&mut self, emitter_id: u32, now_secs: u64) -> bool {
        let tokens = self.tokens.entry(emitter_id).or_insert(self.burst);
        let last = self.last_update.entry(emitter_id).or_insert(now_secs);

        // Refill tokens based on elapsed time
        let elapsed = now_secs.saturating_sub(*last);
        if elapsed > 0 {
            *tokens = (*tokens + elapsed as f64 * self.rate).min(self.burst);
            *last = now_secs;
        }

        // Check and consume
        if *tokens >= 1.0 {
            *tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Check without consuming (peek)
    pub fn would_allow(&self, emitter_id: u32, now_secs: u64) -> bool {
        let tokens = self.tokens.get(&emitter_id).copied().unwrap_or(self.burst);
        let last = self
            .last_update
            .get(&emitter_id)
            .copied()
            .unwrap_or(now_secs);

        let elapsed = now_secs.saturating_sub(last);
        let available = (tokens + elapsed as f64 * self.rate).min(self.burst);

        available >= 1.0
    }

    /// Get remaining tokens for an emitter
    pub fn remaining(&self, emitter_id: u32) -> f64 {
        self.tokens.get(&emitter_id).copied().unwrap_or(self.burst)
    }

    /// Reset rate limiter for an emitter
    pub fn reset(&mut self, emitter_id: u32) {
        self.tokens.remove(&emitter_id);
        self.last_update.remove(&emitter_id);
    }

    /// Reset all emitters
    pub fn reset_all(&mut self) {
        self.tokens.clear();
        self.last_update.clear();
    }

    /// Get number of tracked emitters
    pub fn tracked_count(&self) -> usize {
        self.tokens.len()
    }
}

/// Certificate validation result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertValidation {
    /// Certificate is valid
    Valid,
    /// Certificate has expired
    Expired,
    /// Certificate is not yet valid
    NotYetValid,
    /// Invalid signature
    InvalidSignature,
    /// Unknown certificate issuer
    UnknownIssuer,
    /// Certificate has been revoked
    Revoked,
    /// Fingerprint not in allowed list
    FingerprintMismatch,
    /// Self-signed certificate (may be allowed)
    SelfSigned,
}

impl std::fmt::Display for CertValidation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CertValidation::Valid => write!(f, "Valid"),
            CertValidation::Expired => write!(f, "Certificate expired"),
            CertValidation::NotYetValid => write!(f, "Certificate not yet valid"),
            CertValidation::InvalidSignature => write!(f, "Invalid signature"),
            CertValidation::UnknownIssuer => write!(f, "Unknown issuer"),
            CertValidation::Revoked => write!(f, "Certificate revoked"),
            CertValidation::FingerprintMismatch => write!(f, "Fingerprint mismatch"),
            CertValidation::SelfSigned => write!(f, "Self-signed certificate"),
        }
    }
}

/// Validate a certificate fingerprint against an allowed list
///
/// # Arguments
/// * `fingerprint` - The certificate fingerprint to validate
/// * `allowed` - List of allowed fingerprints (empty = allow all)
///
/// # Returns
/// `CertValidation::Valid` if the fingerprint is in the allowed list or the list is empty
pub fn validate_fingerprint(fingerprint: &str, allowed: &[String]) -> CertValidation {
    if allowed.is_empty() {
        return CertValidation::Valid;
    }

    if allowed.iter().any(|f| f == fingerprint) {
        CertValidation::Valid
    } else {
        CertValidation::FingerprintMismatch
    }
}

/// Security context for a session
pub struct SecurityContext {
    /// Security configuration
    pub config: SecurityConfig,
    /// Rate limiter (if enabled)
    pub rate_limiter: Option<RateLimiter>,
    /// Audit logger (if enabled)
    audit_logger: Option<Box<dyn AuditLogger>>,
}

impl std::fmt::Debug for SecurityContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecurityContext")
            .field("config", &self.config)
            .field("rate_limiter", &self.rate_limiter)
            .field("audit_logger", &self.audit_logger.is_some())
            .finish()
    }
}

impl SecurityContext {
    /// Create a new security context from configuration
    pub fn new(config: SecurityConfig) -> Self {
        let rate_limiter = config
            .rate_limit
            .map(|rate| RateLimiter::new(rate, config.rate_burst.unwrap_or(rate / 10).max(1)));

        Self {
            config,
            rate_limiter,
            audit_logger: None,
        }
    }

    /// Create with a specific audit logger
    pub fn with_audit_logger(mut self, logger: Box<dyn AuditLogger>) -> Self {
        self.audit_logger = Some(logger);
        self
    }

    /// Create with the default memory audit logger
    pub fn with_memory_audit(self, max_events: usize) -> Self {
        self.with_audit_logger(Box::new(MemoryAuditLogger::new(max_events)))
    }

    /// Log an audit event
    pub fn audit(&self, event: AuditEvent) {
        if self.config.audit_enabled {
            if let Some(ref logger) = self.audit_logger {
                logger.log(event);
            }
        }
    }

    /// Check rate limit for an emitter
    pub fn check_rate_limit(&mut self, emitter_id: u32, now_secs: u64) -> bool {
        if let Some(ref mut limiter) = self.rate_limiter {
            limiter.check(emitter_id, now_secs)
        } else {
            true // No rate limiting configured
        }
    }

    /// Validate a certificate fingerprint
    pub fn validate_cert(&self, fingerprint: &str) -> CertValidation {
        if !self.config.mtls_required {
            return CertValidation::Valid;
        }
        validate_fingerprint(fingerprint, &self.config.allowed_fingerprints)
    }

    /// Flush audit logs
    pub fn flush(&self) {
        if let Some(ref logger) = self.audit_logger {
            logger.flush();
        }
    }
}

impl Default for SecurityContext {
    fn default() -> Self {
        Self::new(SecurityConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert!(!config.tls_enabled);
        assert!(!config.mtls_required);
        assert!(!config.audit_enabled);
        assert!(config.rate_limit.is_none());
    }

    #[test]
    fn test_security_config_secure() {
        let config = SecurityConfig::secure();
        assert!(config.tls_enabled);
        assert!(config.mtls_required);
        assert!(config.audit_enabled);
        assert!(config.rate_limit.is_some());
    }

    #[test]
    fn test_rate_limiter_burst() {
        let mut limiter = RateLimiter::new(10, 5);

        // First 5 should pass (burst)
        for i in 0..5 {
            assert!(limiter.check(1, 0), "Request {} should pass", i);
        }

        // 6th should fail (burst exhausted)
        assert!(!limiter.check(1, 0), "6th request should fail");
    }

    #[test]
    fn test_rate_limiter_refill() {
        let mut limiter = RateLimiter::new(10, 5);

        // Exhaust burst
        for _ in 0..5 {
            limiter.check(1, 0);
        }
        assert!(!limiter.check(1, 0));

        // After 1 second, should have 5 new tokens (rate=10, but capped at burst=5)
        assert!(limiter.check(1, 1));
    }

    #[test]
    fn test_rate_limiter_multiple_emitters() {
        let mut limiter = RateLimiter::new(10, 3);

        // Emitter 1
        assert!(limiter.check(1, 0));
        assert!(limiter.check(1, 0));
        assert!(limiter.check(1, 0));
        assert!(!limiter.check(1, 0)); // Exhausted

        // Emitter 2 should still have tokens
        assert!(limiter.check(2, 0));
    }

    #[test]
    fn test_audit_logger() {
        let logger = MemoryAuditLogger::new(100);

        logger.log(
            AuditEvent::new(AuditEventType::ConnectionEstablished, "New connection")
                .with_emitter(42),
        );

        let events = logger.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].emitter_id, Some(42));
        assert_eq!(events[0].event_type, AuditEventType::ConnectionEstablished);
    }

    #[test]
    fn test_audit_logger_max_events() {
        let logger = MemoryAuditLogger::new(3);

        for i in 0..5 {
            logger.log(AuditEvent::new(
                AuditEventType::MessageReceived,
                format!("Message {}", i),
            ));
        }

        // Should only have last 3 events
        let events = logger.events();
        assert_eq!(events.len(), 3);
        assert!(events[0].details.contains("Message 2"));
        assert!(events[2].details.contains("Message 4"));
    }

    #[test]
    fn test_audit_filter() {
        let logger = MemoryAuditLogger::new(100);

        logger.log(
            AuditEvent::new(AuditEventType::MessageReceived, "msg1")
                .with_emitter(1)
                .with_severity(Severity::Info),
        );
        logger.log(
            AuditEvent::new(AuditEventType::AnomalyDetected, "anomaly")
                .with_emitter(2)
                .with_severity(Severity::High),
        );
        logger.log(
            AuditEvent::new(AuditEventType::MessageReceived, "msg2")
                .with_emitter(1)
                .with_severity(Severity::Info),
        );

        // Filter by type
        let filter = AuditFilter {
            event_type: Some(AuditEventType::MessageReceived),
            ..Default::default()
        };
        let results = logger.query(&filter);
        assert_eq!(results.len(), 2);

        // Filter by emitter
        let filter = AuditFilter {
            emitter_id: Some(2),
            ..Default::default()
        };
        let results = logger.query(&filter);
        assert_eq!(results.len(), 1);

        // Filter by severity
        let filter = AuditFilter {
            min_severity: Some(Severity::High),
            ..Default::default()
        };
        let results = logger.query(&filter);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_fingerprint_validation() {
        let allowed = vec!["abc123".to_string(), "def456".to_string()];

        assert_eq!(
            validate_fingerprint("abc123", &allowed),
            CertValidation::Valid
        );

        assert_eq!(
            validate_fingerprint("def456", &allowed),
            CertValidation::Valid
        );

        assert_eq!(
            validate_fingerprint("unknown", &allowed),
            CertValidation::FingerprintMismatch
        );

        // Empty allowed list = allow all
        assert_eq!(validate_fingerprint("anything", &[]), CertValidation::Valid);
    }

    #[test]
    fn test_security_context() {
        let config = SecurityConfig::with_rate_limit(10, 5);
        let mut ctx = SecurityContext::new(config);

        // Rate limiting should work
        for _ in 0..5 {
            assert!(ctx.check_rate_limit(1, 0));
        }
        assert!(!ctx.check_rate_limit(1, 0));
    }

    #[test]
    fn test_audit_event_log_line() {
        let event = AuditEvent::new(AuditEventType::AuthFailure, "Invalid credentials")
            .with_emitter(123)
            .with_severity(Severity::High);

        let line = event.to_log_line();
        assert!(line.contains("AUTH_FAILURE"));
        assert!(line.contains("emitter=123"));
        assert!(line.contains("Invalid credentials"));
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Low);
        assert!(Severity::Low < Severity::Medium);
        assert!(Severity::Medium < Severity::High);
        assert!(Severity::High < Severity::Critical);
    }
}
