//! # ALEC - Adaptive Lazy Evolving Compression
//!
//! A smart compression codec designed for constrained environments where every bit counts.
//!
//! ## Key Features
//!
//! - **Lazy Compression**: Transmit decisions before data
//! - **Evolving Context**: Shared dictionary that improves over time
//! - **Asymmetric Design**: Light encoder, heavy decoder (or vice versa)
//! - **Priority Classification**: P1 (critical) to P5 (disposable)
//!
//! ## Quick Start
//!
//! ```rust
//! use alec::{Encoder, Decoder, Context, Classifier, RawData};
//!
//! // Create components
//! let mut encoder = Encoder::new();
//! let mut decoder = Decoder::new();
//! let classifier = Classifier::default();
//! let mut context = Context::new();
//!
//! // Encode a value
//! let data = RawData::new(22.5, 0);
//! let classification = classifier.classify(&data, &context);
//! let message = encoder.encode(&data, &classification, &context);
//!
//! // Decode
//! let decoded = decoder.decode(&message, &context).unwrap();
//! assert!((decoded.value - data.value).abs() < 0.01);
//!
//! // Update context
//! context.observe(&data);
//! ```
//!
//! ## Modules
//!
//! - [`protocol`]: Message types, priorities, and wire format
//! - [`encoder`]: Data encoding
//! - [`decoder`]: Message decoding
//! - [`classifier`]: Priority classification
//! - [`context`]: Shared context (dictionary + prediction model)
//! - [`channel`]: Communication channel abstraction
//! - [`metrics`]: Compression statistics and analysis

// Modules
pub mod channel;
pub mod classifier;
pub mod context;
pub mod decoder;
pub mod encoder;
pub mod error;
pub mod fleet;
pub mod health;
pub mod metrics;
pub mod protocol;
pub mod recovery;
pub mod security;
pub mod sync;
pub mod tls;

// Re-exports for convenient access
pub use channel::Channel;
pub use classifier::{Classification, ClassificationReason, Classifier};
pub use context::Context;
pub use decoder::Decoder;
pub use encoder::Encoder;
pub use error::{AlecError, Result};
pub use fleet::{EmitterId, EmitterState, FleetConfig, FleetManager, FleetStats, ProcessedMessage};
pub use health::{HealthCheck, HealthCheckable, HealthConfig, HealthMonitor, HealthStatus};
pub use metrics::{CompressionMetrics, ContextMetrics};
pub use protocol::{EncodedMessage, EncodingType, MessageHeader, MessageType, Priority, RawData};
pub use recovery::{
    with_retry, with_retry_metrics, CircuitBreaker, CircuitConfig, CircuitState, DegradationLevel,
    RetryResult, RetryStrategy,
};
pub use security::{
    AuditEvent, AuditEventType, AuditFilter, AuditLogger, CertValidation, MemoryAuditLogger,
    RateLimiter, SecurityConfig, SecurityContext, Severity,
};
pub use sync::{
    SyncAnnounce, SyncConfig, SyncDiff, SyncMessage, SyncRequest, SyncState, Synchronizer,
};
pub use tls::{DtlsConfig, TlsConfig, TlsState};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Protocol version
pub const PROTOCOL_VERSION: u8 = 1;

/// Maximum payload size in bytes
pub const MAX_PAYLOAD_SIZE: usize = 65535;

/// Default scale factor for delta encoding (100 = 2 decimal places)
pub const DEFAULT_SCALE_FACTOR: u32 = 100;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_basic_roundtrip() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        let classifier = Classifier::default();
        let mut context = Context::new();

        let data = RawData::new(42.0, 0);
        let classification = classifier.classify(&data, &context);
        let message = encoder.encode(&data, &classification, &context);
        let decoded = decoder.decode(&message, &context).unwrap();

        assert!((decoded.value - data.value).abs() < 0.001);

        context.observe(&data);
    }
}
