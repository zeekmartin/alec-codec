// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.


//! Automatic recovery mechanisms
//!
//! Provides circuit breaker, retry logic, and recovery strategies.

use std::time::{Duration, Instant};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CircuitState {
    /// Normal operation - requests are allowed
    #[default]
    Closed,
    /// Failing - rejecting requests
    Open,
    /// Testing if recovery is possible
    HalfOpen,
}

impl CircuitState {
    /// Check if requests should be allowed
    pub fn allows_requests(&self) -> bool {
        matches!(self, Self::Closed | Self::HalfOpen)
    }
}

/// Configuration for the circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Number of successes in half-open state to close circuit
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

/// Circuit breaker for fault tolerance
///
/// Implements the circuit breaker pattern to prevent cascade failures.
/// When failures exceed the threshold, the circuit opens and rejects
/// requests until a recovery timeout allows a half-open state.
#[derive(Debug)]
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure: Option<Instant>,
    config: CircuitConfig,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with default configuration
    pub fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure: None,
            config: CircuitConfig::default(),
        }
    }

    /// Create a circuit breaker with custom configuration
    pub fn with_config(config: CircuitConfig) -> Self {
        Self {
            config,
            ..Self::new()
        }
    }

    /// Check if request should be allowed
    ///
    /// Returns true if the request can proceed, false if it should be rejected.
    /// Also handles state transitions from Open to HalfOpen when recovery timeout expires.
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
    ///
    /// In closed state: resets failure count
    /// In half-open state: increments success count, closes circuit if threshold reached
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
    ///
    /// In closed state: increments failure count, opens circuit if threshold reached
    /// In half-open state: immediately opens circuit
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

    /// Get current circuit state
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Get current failure count
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    /// Get current success count (in half-open state)
    pub fn success_count(&self) -> u32 {
        self.success_count
    }

    /// Get time since last failure
    pub fn time_since_last_failure(&self) -> Option<Duration> {
        self.last_failure.map(|t| t.elapsed())
    }

    /// Reset the circuit breaker to initial state
    pub fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.success_count = 0;
        self.last_failure = None;
    }

    /// Force the circuit open
    pub fn force_open(&mut self) {
        self.state = CircuitState::Open;
        self.last_failure = Some(Instant::now());
    }

    /// Force the circuit closed
    pub fn force_closed(&mut self) {
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.success_count = 0;
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Retry strategy for operations
#[derive(Debug, Clone, Default)]
pub enum RetryStrategy {
    /// No retries
    #[default]
    None,
    /// Fixed number of retries with constant delay
    Fixed {
        /// Maximum number of retry attempts
        max_retries: u32,
        /// Delay between retries
        delay: Duration,
    },
    /// Exponential backoff with jitter
    ExponentialBackoff {
        /// Maximum number of retry attempts
        max_retries: u32,
        /// Initial delay
        initial_delay: Duration,
        /// Maximum delay
        max_delay: Duration,
        /// Multiplier for each attempt
        multiplier: f64,
    },
    /// Linear backoff
    LinearBackoff {
        /// Maximum number of retry attempts
        max_retries: u32,
        /// Initial delay
        initial_delay: Duration,
        /// Increment per attempt
        increment: Duration,
        /// Maximum delay
        max_delay: Duration,
    },
}

impl RetryStrategy {
    /// Calculate delay for a given attempt number (0-indexed)
    ///
    /// Returns None if no more retries should be attempted
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
                    let delay_ms =
                        initial_delay.as_millis() as f64 * multiplier.powi(attempt as i32);
                    let delay = Duration::from_millis(delay_ms as u64);
                    Some(delay.min(*max_delay))
                } else {
                    None
                }
            }
            Self::LinearBackoff {
                max_retries,
                initial_delay,
                increment,
                max_delay,
            } => {
                if attempt < *max_retries {
                    let delay = *initial_delay + (*increment * attempt);
                    Some(delay.min(*max_delay))
                } else {
                    None
                }
            }
        }
    }

    /// Get maximum number of retries
    pub fn max_retries(&self) -> u32 {
        match self {
            Self::None => 0,
            Self::Fixed { max_retries, .. }
            | Self::ExponentialBackoff { max_retries, .. }
            | Self::LinearBackoff { max_retries, .. } => *max_retries,
        }
    }

    /// Create a fixed retry strategy
    pub fn fixed(max_retries: u32, delay: Duration) -> Self {
        Self::Fixed { max_retries, delay }
    }

    /// Create an exponential backoff strategy
    pub fn exponential(max_retries: u32, initial_delay: Duration) -> Self {
        Self::ExponentialBackoff {
            max_retries,
            initial_delay,
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
        }
    }

    /// Create a linear backoff strategy
    pub fn linear(max_retries: u32, initial_delay: Duration, increment: Duration) -> Self {
        Self::LinearBackoff {
            max_retries,
            initial_delay,
            increment,
            max_delay: Duration::from_secs(30),
        }
    }
}

/// Execute an operation with retry logic
///
/// Retries the operation according to the strategy, sleeping between attempts.
///
/// # Example
///
/// ```ignore
/// use alec::recovery::{RetryStrategy, with_retry};
/// use std::time::Duration;
///
/// let strategy = RetryStrategy::exponential(3, Duration::from_millis(100));
/// let result = with_retry(&strategy, || {
///     // Your fallible operation here
///     Ok::<_, &str>(42)
/// });
/// ```
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

/// Result of a retry operation with metrics
#[derive(Debug, Clone)]
pub struct RetryResult<T> {
    /// The result value
    pub value: T,
    /// Number of attempts made
    pub attempts: u32,
    /// Total time spent retrying
    pub total_duration: Duration,
}

/// Execute an operation with retry logic and return metrics
pub fn with_retry_metrics<T, E, F>(
    strategy: &RetryStrategy,
    mut operation: F,
) -> Result<RetryResult<T>, E>
where
    F: FnMut() -> Result<T, E>,
{
    let start = Instant::now();
    let mut attempt = 0;
    loop {
        match operation() {
            Ok(result) => {
                return Ok(RetryResult {
                    value: result,
                    attempts: attempt + 1,
                    total_duration: start.elapsed(),
                })
            }
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

/// Graceful degradation level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum DegradationLevel {
    /// Normal operation
    #[default]
    Normal,
    /// Light degradation - non-essential features disabled
    Light,
    /// Moderate degradation - some features disabled
    Moderate,
    /// Heavy degradation - only essential features
    Heavy,
    /// Emergency - minimal operation
    Emergency,
}

impl DegradationLevel {
    /// Check if features at the given level should be disabled
    pub fn should_disable(&self, feature_level: DegradationLevel) -> bool {
        *self >= feature_level
    }

    /// Get next degradation level
    pub fn escalate(&self) -> Self {
        match self {
            Self::Normal => Self::Light,
            Self::Light => Self::Moderate,
            Self::Moderate => Self::Heavy,
            Self::Heavy => Self::Emergency,
            Self::Emergency => Self::Emergency,
        }
    }

    /// Get previous degradation level
    pub fn de_escalate(&self) -> Self {
        match self {
            Self::Normal => Self::Normal,
            Self::Light => Self::Normal,
            Self::Moderate => Self::Light,
            Self::Heavy => Self::Moderate,
            Self::Emergency => Self::Heavy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_state_default() {
        assert_eq!(CircuitState::default(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_state_allows_requests() {
        assert!(CircuitState::Closed.allows_requests());
        assert!(!CircuitState::Open.allows_requests());
        assert!(CircuitState::HalfOpen.allows_requests());
    }

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
        assert_eq!(cb.failure_count(), 2);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_success_resets_count() {
        let mut cb = CircuitBreaker::with_config(CircuitConfig {
            failure_threshold: 3,
            ..Default::default()
        });

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);

        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
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
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_half_open_failure() {
        let mut cb = CircuitBreaker::with_config(CircuitConfig {
            failure_threshold: 1,
            success_threshold: 2,
            recovery_timeout: Duration::from_millis(10),
        });

        cb.record_failure();
        std::thread::sleep(Duration::from_millis(15));
        cb.should_allow(); // Transition to half-open

        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_reset() {
        let mut cb = CircuitBreaker::new();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_circuit_breaker_force_open() {
        let mut cb = CircuitBreaker::new();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.force_open();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_force_closed() {
        let mut cb = CircuitBreaker::new();
        cb.force_open();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.force_closed();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_retry_strategy_none() {
        let strategy = RetryStrategy::None;
        assert_eq!(strategy.delay_for_attempt(0), None);
        assert_eq!(strategy.max_retries(), 0);
    }

    #[test]
    fn test_retry_strategy_fixed() {
        let strategy = RetryStrategy::Fixed {
            max_retries: 3,
            delay: Duration::from_millis(100),
        };

        assert_eq!(
            strategy.delay_for_attempt(0),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            strategy.delay_for_attempt(1),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            strategy.delay_for_attempt(2),
            Some(Duration::from_millis(100))
        );
        assert_eq!(strategy.delay_for_attempt(3), None);
    }

    #[test]
    fn test_exponential_backoff() {
        let strategy = RetryStrategy::ExponentialBackoff {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
        };

        assert_eq!(
            strategy.delay_for_attempt(0),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            strategy.delay_for_attempt(1),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            strategy.delay_for_attempt(2),
            Some(Duration::from_millis(400))
        );
        assert_eq!(
            strategy.delay_for_attempt(3),
            Some(Duration::from_millis(800))
        );
        assert_eq!(
            strategy.delay_for_attempt(4),
            Some(Duration::from_millis(1600))
        );
        assert_eq!(strategy.delay_for_attempt(5), None);
    }

    #[test]
    fn test_exponential_backoff_max_delay() {
        let strategy = RetryStrategy::ExponentialBackoff {
            max_retries: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(500),
            multiplier: 2.0,
        };

        assert_eq!(
            strategy.delay_for_attempt(0),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            strategy.delay_for_attempt(1),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            strategy.delay_for_attempt(2),
            Some(Duration::from_millis(400))
        );
        // Capped at max_delay
        assert_eq!(
            strategy.delay_for_attempt(3),
            Some(Duration::from_millis(500))
        );
        assert_eq!(
            strategy.delay_for_attempt(4),
            Some(Duration::from_millis(500))
        );
    }

    #[test]
    fn test_linear_backoff() {
        let strategy = RetryStrategy::LinearBackoff {
            max_retries: 4,
            initial_delay: Duration::from_millis(100),
            increment: Duration::from_millis(50),
            max_delay: Duration::from_secs(1),
        };

        assert_eq!(
            strategy.delay_for_attempt(0),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            strategy.delay_for_attempt(1),
            Some(Duration::from_millis(150))
        );
        assert_eq!(
            strategy.delay_for_attempt(2),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            strategy.delay_for_attempt(3),
            Some(Duration::from_millis(250))
        );
        assert_eq!(strategy.delay_for_attempt(4), None);
    }

    #[test]
    fn test_retry_strategy_helpers() {
        let fixed = RetryStrategy::fixed(3, Duration::from_millis(100));
        assert_eq!(fixed.max_retries(), 3);

        let exp = RetryStrategy::exponential(5, Duration::from_millis(50));
        assert_eq!(exp.max_retries(), 5);

        let linear = RetryStrategy::linear(4, Duration::from_millis(100), Duration::from_millis(25));
        assert_eq!(linear.max_retries(), 4);
    }

    #[test]
    fn test_with_retry_success() {
        let strategy = RetryStrategy::fixed(3, Duration::from_millis(1));
        let result = with_retry(&strategy, || Ok::<_, &str>(42));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_with_retry_eventual_success() {
        let strategy = RetryStrategy::fixed(3, Duration::from_millis(1));
        let mut attempts = 0;
        let result = with_retry(&strategy, || {
            attempts += 1;
            if attempts < 3 {
                Err("not yet")
            } else {
                Ok(42)
            }
        });
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts, 3);
    }

    #[test]
    fn test_with_retry_all_failures() {
        let strategy = RetryStrategy::fixed(2, Duration::from_millis(1));
        let mut attempts = 0;
        let result = with_retry(&strategy, || {
            attempts += 1;
            Err::<i32, _>("always fails")
        });
        assert!(result.is_err());
        assert_eq!(attempts, 3); // Initial + 2 retries
    }

    #[test]
    fn test_with_retry_metrics() {
        let strategy = RetryStrategy::fixed(2, Duration::from_millis(1));
        let result = with_retry_metrics(&strategy, || Ok::<_, &str>(42));
        let metrics = result.unwrap();
        assert_eq!(metrics.value, 42);
        assert_eq!(metrics.attempts, 1);
    }

    #[test]
    fn test_degradation_level_ordering() {
        assert!(DegradationLevel::Normal < DegradationLevel::Light);
        assert!(DegradationLevel::Light < DegradationLevel::Moderate);
        assert!(DegradationLevel::Moderate < DegradationLevel::Heavy);
        assert!(DegradationLevel::Heavy < DegradationLevel::Emergency);
    }

    #[test]
    fn test_degradation_level_should_disable() {
        let level = DegradationLevel::Moderate;

        assert!(!level.should_disable(DegradationLevel::Heavy));
        assert!(!level.should_disable(DegradationLevel::Emergency));
        assert!(level.should_disable(DegradationLevel::Moderate));
        assert!(level.should_disable(DegradationLevel::Light));
        assert!(level.should_disable(DegradationLevel::Normal));
    }

    #[test]
    fn test_degradation_level_escalate() {
        assert_eq!(DegradationLevel::Normal.escalate(), DegradationLevel::Light);
        assert_eq!(
            DegradationLevel::Light.escalate(),
            DegradationLevel::Moderate
        );
        assert_eq!(
            DegradationLevel::Moderate.escalate(),
            DegradationLevel::Heavy
        );
        assert_eq!(
            DegradationLevel::Heavy.escalate(),
            DegradationLevel::Emergency
        );
        assert_eq!(
            DegradationLevel::Emergency.escalate(),
            DegradationLevel::Emergency
        );
    }

    #[test]
    fn test_degradation_level_de_escalate() {
        assert_eq!(
            DegradationLevel::Emergency.de_escalate(),
            DegradationLevel::Heavy
        );
        assert_eq!(
            DegradationLevel::Heavy.de_escalate(),
            DegradationLevel::Moderate
        );
        assert_eq!(
            DegradationLevel::Moderate.de_escalate(),
            DegradationLevel::Light
        );
        assert_eq!(
            DegradationLevel::Light.de_escalate(),
            DegradationLevel::Normal
        );
        assert_eq!(
            DegradationLevel::Normal.de_escalate(),
            DegradationLevel::Normal
        );
    }
}
