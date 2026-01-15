# Configuration Reference

Complete configuration options for ALEC.

## ContextConfig

```rust
pub struct ContextConfig {
    pub max_patterns: usize,    // Default: 65535
    pub max_memory: usize,      // Default: 64KB
    pub history_size: usize,    // Default: 100
    pub ema_alpha: f64,         // Default: 0.3
    pub evolution: EvolutionConfig,
}
```

## EvolutionConfig

```rust
pub struct EvolutionConfig {
    pub min_frequency: u64,       // Default: 2
    pub max_age: u64,             // Default: 10000
    pub evolution_interval: u64,  // Default: 100
    pub promotion_threshold: u64, // Default: 10
    pub enabled: bool,            // Default: true
}
```

## ClassifierConfig

```rust
pub struct ClassifierConfig {
    pub critical_low: f64,       // Default: f64::MIN
    pub critical_high: f64,      // Default: f64::MAX
    pub anomaly_threshold: f64,  // Default: 3.0
}
```

## SecurityConfig

```rust
pub struct SecurityConfig {
    pub tls_enabled: bool,
    pub mtls_required: bool,
    pub allowed_fingerprints: Vec<String>,
    pub audit_enabled: bool,
    pub rate_limit: Option<u32>,
    pub rate_burst: Option<u32>,
}
```

## FleetConfig

```rust
pub struct FleetConfig {
    pub max_emitters: usize,
    pub stale_timeout: u64,
    pub cleanup_interval: u64,
    pub cross_fleet_threshold: f64,
}
```

## CircuitConfig

```rust
pub struct CircuitConfig {
    pub failure_threshold: u32,   // Default: 5
    pub success_threshold: u32,   // Default: 3
    pub recovery_timeout: Duration, // Default: 30s
}
```

## HealthConfig

```rust
pub struct HealthConfig {
    pub degraded_latency_ms: u64,
    pub unhealthy_latency_ms: u64,
    pub degraded_queue_depth: usize,
    pub unhealthy_queue_depth: usize,
    pub check_interval: Duration,
}
```
