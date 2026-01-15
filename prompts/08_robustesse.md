# Prompt 08 — Robustesse (v1.0.0)

## Contexte

Pour une version production, ALEC doit être robuste :
- Résister aux pannes
- Récupérer automatiquement
- Dégrader gracieusement sous charge

## Objectif

Implémenter les mécanismes de robustesse :
1. Tests de stress
2. Recovery automatique
3. Graceful degradation
4. Health checks

## Étapes

### 1. Créer `src/health.rs`

```rust
//! Health monitoring for ALEC components
//!
//! Provides health checks and degradation management.

use std::time::{Duration, Instant};

/// Health status of a component
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Component is healthy
    Healthy,
    /// Component is degraded but functional
    Degraded,
    /// Component is unhealthy
    Unhealthy,
    /// Component status is unknown
    Unknown,
}

impl HealthStatus {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheck {
    /// Component name
    pub component: String,
    /// Status
    pub status: HealthStatus,
    /// Last check time
    pub last_check: Instant,
    /// Details message
    pub message: String,
    /// Response time of the check
    pub latency: Duration,
}

impl HealthCheck {
    pub fn healthy(component: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Healthy,
            last_check: Instant::now(),
            message: "OK".to_string(),
            latency: Duration::ZERO,
        }
    }
    
    pub fn unhealthy(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Unhealthy,
            last_check: Instant::now(),
            message: message.into(),
            latency: Duration::ZERO,
        }
    }
    
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }
}

/// Health monitor for the system
#[derive(Debug)]
pub struct HealthMonitor {
    /// Component health checks
    checks: Vec<HealthCheck>,
    /// Degradation thresholds
    config: HealthConfig,
    /// Current system status
    system_status: HealthStatus,
}

#[derive(Debug, Clone)]
pub struct HealthConfig {
    /// Max latency before degraded (ms)
    pub degraded_latency_ms: u64,
    /// Max latency before unhealthy (ms)
    pub unhealthy_latency_ms: u64,
    /// Max queue depth before degraded
    pub degraded_queue_depth: usize,
    /// Max queue depth before unhealthy
    pub unhealthy_queue_depth: usize,
    /// Check interval
    pub check_interval: Duration,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            degraded_latency_ms: 100,
            unhealthy_latency_ms: 1000,
            degraded_queue_depth: 1000,
            unhealthy_queue_depth: 10000,
            check_interval: Duration::from_secs(10),
        }
    }
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
            config: HealthConfig::default(),
            system_status: HealthStatus::Unknown,
        }
    }
    
    pub fn with_config(config: HealthConfig) -> Self {
        Self {
            config,
            ..Self::new()
        }
    }
    
    /// Add a health check result
    pub fn add_check(&mut self, check: HealthCheck) {
        // Remove old check for same component
        self.checks.retain(|c| c.component != check.component);
        self.checks.push(check);
        self.update_system_status();
    }
    
    /// Update overall system status
    fn update_system_status(&mut self) {
        if self.checks.is_empty() {
            self.system_status = HealthStatus::Unknown;
            return;
        }
        
        let unhealthy = self.checks.iter()
            .any(|c| c.status == HealthStatus::Unhealthy);
        let degraded = self.checks.iter()
            .any(|c| c.status == HealthStatus::Degraded);
        
        self.system_status = if unhealthy {
            HealthStatus::Unhealthy
        } else if degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };
    }
    
    /// Get system status
    pub fn status(&self) -> HealthStatus {
        self.system_status
    }
    
    /// Get all checks
    pub fn checks(&self) -> &[HealthCheck] {
        &self.checks
    }
    
    /// Generate health report
    pub fn report(&self) -> String {
        let mut report = String::new();
        report.push_str(&format!("System Status: {:?}\n\n", self.system_status));
        
        for check in &self.checks {
            report.push_str(&format!(
                "[{:?}] {} - {} ({}ms)\n",
                check.status,
                check.component,
                check.message,
                check.latency.as_millis()
            ));
        }
        
        report
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for components that can be health-checked
pub trait HealthCheckable {
    /// Perform health check
    fn health_check(&self) -> HealthCheck;
}
```

### 2. Créer `src/recovery.rs`

```rust
//! Automatic recovery mechanisms
//!
//! Provides circuit breaker, retry logic, and recovery strategies.

use std::time::{Duration, Instant};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation
    Closed,
    /// Failing, rejecting requests
    Open,
    /// Testing if recovery is possible
    HalfOpen,
}

/// Circuit breaker for fault tolerance
#[derive(Debug)]
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure: Option<Instant>,
    config: CircuitConfig,
}

#[derive(Debug, Clone)]
pub struct CircuitConfig {
    /// Failures before opening circuit
    pub failure_threshold: u32,
    /// Successes in half-open to close circuit
    pub success_threshold: u32,
    /// Time before attempting recovery
    pub recovery_timeout: Duration,
}

impl Default for CircuitConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            recovery_timeout: Duration::from_secs(30),
        }
    }
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure: None,
            config: CircuitConfig::default(),
        }
    }
    
    pub fn with_config(config: CircuitConfig) -> Self {
        Self {
            config,
            ..Self::new()
        }
    }
    
    /// Check if request should be allowed
    pub fn should_allow(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if recovery timeout has passed
                if let Some(last) = self.last_failure {
                    if last.elapsed() >= self.config.recovery_timeout {
                        self.state = CircuitState::HalfOpen;
                        self.success_count = 0;
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }
    
    /// Record a successful operation
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.config.success_threshold {
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                }
            }
            CircuitState::Open => {}
        }
    }
    
    /// Record a failed operation
    pub fn record_failure(&mut self) {
        self.last_failure = Some(Instant::now());
        
        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.config.failure_threshold {
                    self.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
            }
            CircuitState::Open => {}
        }
    }
    
    /// Get current state
    pub fn state(&self) -> CircuitState {
        self.state
    }
    
    /// Reset the circuit breaker
    pub fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.success_count = 0;
        self.last_failure = None;
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Retry strategy
#[derive(Debug, Clone)]
pub enum RetryStrategy {
    /// No retries
    None,
    /// Fixed number of retries with delay
    Fixed { max_retries: u32, delay: Duration },
    /// Exponential backoff
    ExponentialBackoff {
        max_retries: u32,
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: f64,
    },
}

impl RetryStrategy {
    /// Calculate delay for a given attempt
    pub fn delay_for_attempt(&self, attempt: u32) -> Option<Duration> {
        match self {
            Self::None => None,
            Self::Fixed { max_retries, delay } => {
                if attempt < *max_retries {
                    Some(*delay)
                } else {
                    None
                }
            }
            Self::ExponentialBackoff {
                max_retries,
                initial_delay,
                max_delay,
                multiplier,
            } => {
                if attempt < *max_retries {
                    let delay = initial_delay.as_millis() as f64 
                        * multiplier.powi(attempt as i32);
                    let delay = Duration::from_millis(delay as u64);
                    Some(delay.min(*max_delay))
                } else {
                    None
                }
            }
        }
    }
}

/// Execute with retry
pub fn with_retry<T, E, F>(strategy: &RetryStrategy, mut operation: F) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
{
    let mut attempt = 0;
    loop {
        match operation() {
            Ok(result) => return Ok(result),
            Err(e) => {
                if let Some(delay) = strategy.delay_for_attempt(attempt) {
                    std::thread::sleep(delay);
                    attempt += 1;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_circuit_breaker_opens() {
        let mut cb = CircuitBreaker::with_config(CircuitConfig {
            failure_threshold: 3,
            ..Default::default()
        });
        
        assert_eq!(cb.state(), CircuitState::Closed);
        
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }
    
    #[test]
    fn test_circuit_breaker_recovery() {
        let mut cb = CircuitBreaker::with_config(CircuitConfig {
            failure_threshold: 1,
            success_threshold: 2,
            recovery_timeout: Duration::from_millis(10),
        });
        
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.should_allow());
        
        // Wait for recovery
        std::thread::sleep(Duration::from_millis(15));
        
        assert!(cb.should_allow());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        
        cb.record_success();
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }
    
    #[test]
    fn test_exponential_backoff() {
        let strategy = RetryStrategy::ExponentialBackoff {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
        };
        
        assert_eq!(strategy.delay_for_attempt(0), Some(Duration::from_millis(100)));
        assert_eq!(strategy.delay_for_attempt(1), Some(Duration::from_millis(200)));
        assert_eq!(strategy.delay_for_attempt(2), Some(Duration::from_millis(400)));
        assert_eq!(strategy.delay_for_attempt(5), None);
    }
}
```

### 3. Créer des tests de stress

`tests/stress.rs` :

```rust
//! Stress tests for ALEC
//!
//! Run with: cargo test --release stress -- --ignored

use alec::*;
use std::time::{Duration, Instant};

#[test]
#[ignore]  // Run manually with --ignored
fn stress_test_encoding() {
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();
    
    let iterations = 1_000_000;
    let start = Instant::now();
    
    for i in 0..iterations {
        let data = RawData::new(20.0 + (i as f64 * 0.001).sin(), i as u64);
        let classification = classifier.classify(&data, &context);
        let _message = encoder.encode(&data, &classification, &context);
        context.observe(&data);
    }
    
    let elapsed = start.elapsed();
    let rate = iterations as f64 / elapsed.as_secs_f64();
    
    println!("Encoded {} messages in {:?}", iterations, elapsed);
    println!("Rate: {:.0} messages/second", rate);
    
    assert!(rate > 100_000.0, "Should encode at least 100k msg/s");
}

#[test]
#[ignore]
fn stress_test_roundtrip() {
    let mut encoder = Encoder::new();
    let mut decoder = Decoder::new();
    let classifier = Classifier::default();
    let mut ctx_enc = Context::new();
    let mut ctx_dec = Context::new();
    
    let iterations = 100_000;
    let start = Instant::now();
    
    for i in 0..iterations {
        let data = RawData::new(20.0 + (i as f64 * 0.01).sin() * 5.0, i as u64);
        let classification = classifier.classify(&data, &ctx_enc);
        let message = encoder.encode(&data, &classification, &ctx_enc);
        let decoded = decoder.decode(&message, &ctx_dec).unwrap();
        
        assert!((decoded.value - data.value).abs() < 0.1);
        
        ctx_enc.observe(&data);
        ctx_dec.observe(&data);
    }
    
    let elapsed = start.elapsed();
    let rate = iterations as f64 / elapsed.as_secs_f64();
    
    println!("Roundtrip {} messages in {:?}", iterations, elapsed);
    println!("Rate: {:.0} messages/second", rate);
    
    assert!(rate > 50_000.0, "Should roundtrip at least 50k msg/s");
}

#[test]
#[ignore]
fn stress_test_fleet() {
    use alec::fleet::FleetManager;
    
    let mut fleet = FleetManager::new();
    let classifier = Classifier::default();
    
    let num_emitters = 100;
    let messages_per_emitter = 1000;
    
    let mut encoders: Vec<_> = (0..num_emitters)
        .map(|_| (Encoder::new(), Context::new()))
        .collect();
    
    let start = Instant::now();
    
    for t in 0..messages_per_emitter {
        for (emitter_id, (encoder, context)) in encoders.iter_mut().enumerate() {
            let temp = 20.0 + (emitter_id as f64 * 0.1) + (t as f64 * 0.01).sin();
            let data = RawData::new(temp, t as u64);
            let classification = classifier.classify(&data, context);
            let message = encoder.encode(&data, &classification, context);
            
            fleet.process_message(emitter_id as u32, &message, t as u64).unwrap();
            context.observe(&data);
        }
    }
    
    let elapsed = start.elapsed();
    let total_messages = num_emitters * messages_per_emitter;
    let rate = total_messages as f64 / elapsed.as_secs_f64();
    
    println!("Fleet processed {} messages in {:?}", total_messages, elapsed);
    println!("Rate: {:.0} messages/second", rate);
    
    assert!(rate > 10_000.0, "Should process at least 10k msg/s");
}
```

### 4. Intégrer health check dans les composants

```rust
impl HealthCheckable for Context {
    fn health_check(&self) -> HealthCheck {
        let start = Instant::now();
        
        // Check memory usage
        let memory = self.estimated_memory();
        let status = if memory > 10_000_000 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };
        
        HealthCheck {
            component: "Context".to_string(),
            status,
            last_check: Instant::now(),
            message: format!("Memory: {} bytes, Patterns: {}", 
                memory, self.pattern_count()),
            latency: start.elapsed(),
        }
    }
}
```

## Livrables

- [ ] `src/health.rs` — Health monitoring
- [ ] `src/recovery.rs` — Circuit breaker et retry
- [ ] `tests/stress.rs` — Tests de stress
- [ ] `HealthCheckable` trait implémenté
- [ ] Intégration avec composants existants
- [ ] Benchmarks documentés

## Critères de succès

```bash
cargo test --release stress -- --ignored  # Performance OK
cargo test health recovery  # Tests unitaires passent
```

Seuils minimaux :
- Encoding : > 100k msg/s
- Roundtrip : > 50k msg/s
- Fleet : > 10k msg/s

## Prochaine étape

→ `09_documentation.md` (v1.0.0 - Documentation)
