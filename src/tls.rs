// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 Simon Music
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.


//! TLS wrapper for secure channels
//!
//! Provides TLS and DTLS configuration for ALEC channels.
//! This module defines the interface - actual TLS implementation
//! requires the `tls` feature flag.

/// TLS configuration builder
#[derive(Debug, Clone, Default)]
pub struct TlsConfig {
    /// Path to certificate file (PEM format)
    pub cert_path: Option<String>,
    /// Path to private key file (PEM format)
    pub key_path: Option<String>,
    /// Path to CA certificate for verification
    pub ca_path: Option<String>,
    /// Server name for SNI and verification
    pub server_name: Option<String>,
    /// Allow self-signed certificates
    pub allow_self_signed: bool,
    /// Minimum TLS version (e.g., "1.2", "1.3")
    pub min_version: Option<String>,
    /// ALPN protocols to negotiate
    pub alpn_protocols: Vec<String>,
}

impl TlsConfig {
    /// Create a new empty TLS configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure certificate and key paths
    pub fn with_cert(mut self, cert_path: &str, key_path: &str) -> Self {
        self.cert_path = Some(cert_path.to_string());
        self.key_path = Some(key_path.to_string());
        self
    }

    /// Configure CA certificate path for verification
    pub fn with_ca(mut self, ca_path: &str) -> Self {
        self.ca_path = Some(ca_path.to_string());
        self
    }

    /// Configure server name for SNI
    pub fn with_server_name(mut self, name: &str) -> Self {
        self.server_name = Some(name.to_string());
        self
    }

    /// Allow self-signed certificates
    pub fn allow_self_signed(mut self) -> Self {
        self.allow_self_signed = true;
        self
    }

    /// Set minimum TLS version
    pub fn with_min_version(mut self, version: &str) -> Self {
        self.min_version = Some(version.to_string());
        self
    }

    /// Add ALPN protocol
    pub fn with_alpn(mut self, protocol: &str) -> Self {
        self.alpn_protocols.push(protocol.to_string());
        self
    }

    /// Check if this is a valid client configuration
    pub fn is_valid_client(&self) -> bool {
        // Client needs at least server name or CA for verification
        self.server_name.is_some() || self.ca_path.is_some() || self.allow_self_signed
    }

    /// Check if this is a valid server configuration
    pub fn is_valid_server(&self) -> bool {
        // Server needs cert and key
        self.cert_path.is_some() && self.key_path.is_some()
    }
}

/// DTLS (Datagram TLS) configuration for UDP channels
#[derive(Debug, Clone, Default)]
pub struct DtlsConfig {
    /// Base TLS configuration
    pub tls: TlsConfig,
    /// MTU for DTLS records
    pub mtu: Option<u16>,
    /// Enable replay protection
    pub replay_protection: bool,
    /// Retransmission timeout in milliseconds
    pub retransmit_timeout_ms: Option<u32>,
}

impl DtlsConfig {
    /// Create new DTLS configuration
    pub fn new() -> Self {
        Self {
            replay_protection: true,
            ..Default::default()
        }
    }

    /// Configure from TLS config
    pub fn from_tls(tls: TlsConfig) -> Self {
        Self {
            tls,
            replay_protection: true,
            ..Default::default()
        }
    }

    /// Set MTU
    pub fn with_mtu(mut self, mtu: u16) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Set retransmission timeout
    pub fn with_retransmit_timeout(mut self, timeout_ms: u32) -> Self {
        self.retransmit_timeout_ms = Some(timeout_ms);
        self
    }
}

/// Connection state for TLS sessions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TlsState {
    /// Not connected
    #[default]
    Disconnected,
    /// Handshake in progress
    Handshaking,
    /// Connected and ready
    Connected,
    /// Connection closed gracefully
    Closed,
    /// Connection failed
    Error,
}

/// Result of TLS handshake
#[derive(Debug, Clone)]
pub struct HandshakeResult {
    /// Negotiated protocol version
    pub protocol_version: String,
    /// Negotiated cipher suite
    pub cipher_suite: String,
    /// Peer certificate fingerprint (if available)
    pub peer_fingerprint: Option<String>,
    /// Negotiated ALPN protocol
    pub alpn_protocol: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_config_builder() {
        let config = TlsConfig::new()
            .with_cert("/path/to/cert.pem", "/path/to/key.pem")
            .with_ca("/path/to/ca.pem")
            .with_server_name("example.com")
            .with_min_version("1.3")
            .with_alpn("h2");

        assert_eq!(config.cert_path, Some("/path/to/cert.pem".to_string()));
        assert_eq!(config.key_path, Some("/path/to/key.pem".to_string()));
        assert_eq!(config.ca_path, Some("/path/to/ca.pem".to_string()));
        assert_eq!(config.server_name, Some("example.com".to_string()));
        assert_eq!(config.min_version, Some("1.3".to_string()));
        assert_eq!(config.alpn_protocols, vec!["h2".to_string()]);
    }

    #[test]
    fn test_tls_config_validation() {
        let client_config = TlsConfig::new().with_server_name("example.com");
        assert!(client_config.is_valid_client());
        assert!(!client_config.is_valid_server());

        let server_config = TlsConfig::new().with_cert("/path/to/cert.pem", "/path/to/key.pem");
        assert!(server_config.is_valid_server());

        let self_signed_client = TlsConfig::new().allow_self_signed();
        assert!(self_signed_client.is_valid_client());
    }

    #[test]
    fn test_dtls_config() {
        let dtls = DtlsConfig::new()
            .with_mtu(1400)
            .with_retransmit_timeout(500);

        assert_eq!(dtls.mtu, Some(1400));
        assert_eq!(dtls.retransmit_timeout_ms, Some(500));
        assert!(dtls.replay_protection);
    }

    #[test]
    fn test_tls_state_default() {
        let state = TlsState::default();
        assert_eq!(state, TlsState::Disconnected);
    }
}
