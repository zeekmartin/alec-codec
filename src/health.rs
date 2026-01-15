//! Health monitoring for ALEC components
//!
//! Provides health checks and degradation management.

use std::time::{Duration, Instant};

/// Health status of a component
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HealthStatus {
    /// Component is healthy
    Healthy,
    /// Component is degraded but functional
    Degraded,
    /// Component is unhealthy
    Unhealthy,
    /// Component status is unknown
    #[default]
    Unknown,
}

impl HealthStatus {
    /// Check if the status is operational (healthy or degraded)
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }

    /// Check if the status is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
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
    /// Create a healthy check result
    pub fn healthy(component: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Healthy,
            last_check: Instant::now(),
            message: "OK".to_string(),
            latency: Duration::ZERO,
        }
    }

    /// Create an unhealthy check result
    pub fn unhealthy(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Unhealthy,
            last_check: Instant::now(),
            message: message.into(),
            latency: Duration::ZERO,
        }
    }

    /// Create a degraded check result
    pub fn degraded(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Degraded,
            last_check: Instant::now(),
            message: message.into(),
            latency: Duration::ZERO,
        }
    }

    /// Set the latency for this check
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    /// Set the message for this check
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }
}

/// Health configuration thresholds
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
    /// Max memory before degraded (bytes)
    pub degraded_memory_bytes: usize,
    /// Max memory before unhealthy (bytes)
    pub unhealthy_memory_bytes: usize,
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
            degraded_memory_bytes: 10_000_000,
            unhealthy_memory_bytes: 100_000_000,
            check_interval: Duration::from_secs(10),
        }
    }
}

/// Health monitor for the system
#[derive(Debug, Default)]
pub struct HealthMonitor {
    /// Component health checks
    checks: Vec<HealthCheck>,
    /// Degradation thresholds
    config: HealthConfig,
    /// Current system status
    system_status: HealthStatus,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
            config: HealthConfig::default(),
            system_status: HealthStatus::Unknown,
        }
    }

    /// Create a health monitor with custom configuration
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

    /// Update overall system status based on all checks
    fn update_system_status(&mut self) {
        if self.checks.is_empty() {
            self.system_status = HealthStatus::Unknown;
            return;
        }

        let unhealthy = self.checks.iter().any(|c| c.status == HealthStatus::Unhealthy);
        let degraded = self.checks.iter().any(|c| c.status == HealthStatus::Degraded);

        self.system_status = if unhealthy {
            HealthStatus::Unhealthy
        } else if degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };
    }

    /// Get current system status
    pub fn status(&self) -> HealthStatus {
        self.system_status
    }

    /// Get all checks
    pub fn checks(&self) -> &[HealthCheck] {
        &self.checks
    }

    /// Get configuration
    pub fn config(&self) -> &HealthConfig {
        &self.config
    }

    /// Clear all checks
    pub fn clear(&mut self) {
        self.checks.clear();
        self.system_status = HealthStatus::Unknown;
    }

    /// Get check for a specific component
    pub fn get_check(&self, component: &str) -> Option<&HealthCheck> {
        self.checks.iter().find(|c| c.component == component)
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

    /// Check if system is operational
    pub fn is_operational(&self) -> bool {
        self.system_status.is_ok()
    }

    /// Get count of healthy components
    pub fn healthy_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == HealthStatus::Healthy)
            .count()
    }

    /// Get count of degraded components
    pub fn degraded_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == HealthStatus::Degraded)
            .count()
    }

    /// Get count of unhealthy components
    pub fn unhealthy_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == HealthStatus::Unhealthy)
            .count()
    }
}

/// Trait for components that can be health-checked
pub trait HealthCheckable {
    /// Perform health check
    fn health_check(&self) -> HealthCheck;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_is_ok() {
        assert!(HealthStatus::Healthy.is_ok());
        assert!(HealthStatus::Degraded.is_ok());
        assert!(!HealthStatus::Unhealthy.is_ok());
        assert!(!HealthStatus::Unknown.is_ok());
    }

    #[test]
    fn test_health_status_default() {
        assert_eq!(HealthStatus::default(), HealthStatus::Unknown);
    }

    #[test]
    fn test_health_check_healthy() {
        let check = HealthCheck::healthy("test_component");
        assert_eq!(check.component, "test_component");
        assert_eq!(check.status, HealthStatus::Healthy);
        assert_eq!(check.message, "OK");
    }

    #[test]
    fn test_health_check_unhealthy() {
        let check = HealthCheck::unhealthy("test_component", "Something is wrong");
        assert_eq!(check.component, "test_component");
        assert_eq!(check.status, HealthStatus::Unhealthy);
        assert_eq!(check.message, "Something is wrong");
    }

    #[test]
    fn test_health_check_degraded() {
        let check = HealthCheck::degraded("test_component", "Running slow");
        assert_eq!(check.component, "test_component");
        assert_eq!(check.status, HealthStatus::Degraded);
        assert_eq!(check.message, "Running slow");
    }

    #[test]
    fn test_health_check_with_latency() {
        let check = HealthCheck::healthy("test").with_latency(Duration::from_millis(50));
        assert_eq!(check.latency, Duration::from_millis(50));
    }

    #[test]
    fn test_health_monitor_empty() {
        let monitor = HealthMonitor::new();
        assert_eq!(monitor.status(), HealthStatus::Unknown);
        assert!(monitor.checks().is_empty());
    }

    #[test]
    fn test_health_monitor_add_check() {
        let mut monitor = HealthMonitor::new();

        monitor.add_check(HealthCheck::healthy("component_a"));
        assert_eq!(monitor.status(), HealthStatus::Healthy);
        assert_eq!(monitor.checks().len(), 1);

        monitor.add_check(HealthCheck::healthy("component_b"));
        assert_eq!(monitor.status(), HealthStatus::Healthy);
        assert_eq!(monitor.checks().len(), 2);
    }

    #[test]
    fn test_health_monitor_degraded_status() {
        let mut monitor = HealthMonitor::new();

        monitor.add_check(HealthCheck::healthy("component_a"));
        monitor.add_check(HealthCheck::degraded("component_b", "slow"));

        assert_eq!(monitor.status(), HealthStatus::Degraded);
        assert!(monitor.is_operational());
    }

    #[test]
    fn test_health_monitor_unhealthy_status() {
        let mut monitor = HealthMonitor::new();

        monitor.add_check(HealthCheck::healthy("component_a"));
        monitor.add_check(HealthCheck::unhealthy("component_b", "failed"));

        assert_eq!(monitor.status(), HealthStatus::Unhealthy);
        assert!(!monitor.is_operational());
    }

    #[test]
    fn test_health_monitor_replaces_check() {
        let mut monitor = HealthMonitor::new();

        monitor.add_check(HealthCheck::healthy("component_a"));
        assert_eq!(monitor.status(), HealthStatus::Healthy);

        // Replace with degraded status
        monitor.add_check(HealthCheck::degraded("component_a", "slow"));
        assert_eq!(monitor.checks().len(), 1);
        assert_eq!(monitor.status(), HealthStatus::Degraded);
    }

    #[test]
    fn test_health_monitor_get_check() {
        let mut monitor = HealthMonitor::new();

        monitor.add_check(HealthCheck::healthy("component_a"));
        monitor.add_check(HealthCheck::degraded("component_b", "slow"));

        let check = monitor.get_check("component_a");
        assert!(check.is_some());
        assert_eq!(check.unwrap().status, HealthStatus::Healthy);

        let check = monitor.get_check("component_c");
        assert!(check.is_none());
    }

    #[test]
    fn test_health_monitor_counts() {
        let mut monitor = HealthMonitor::new();

        monitor.add_check(HealthCheck::healthy("a"));
        monitor.add_check(HealthCheck::healthy("b"));
        monitor.add_check(HealthCheck::degraded("c", "slow"));
        monitor.add_check(HealthCheck::unhealthy("d", "failed"));

        assert_eq!(monitor.healthy_count(), 2);
        assert_eq!(monitor.degraded_count(), 1);
        assert_eq!(monitor.unhealthy_count(), 1);
    }

    #[test]
    fn test_health_monitor_clear() {
        let mut monitor = HealthMonitor::new();

        monitor.add_check(HealthCheck::healthy("a"));
        monitor.add_check(HealthCheck::healthy("b"));
        assert_eq!(monitor.checks().len(), 2);

        monitor.clear();
        assert!(monitor.checks().is_empty());
        assert_eq!(monitor.status(), HealthStatus::Unknown);
    }

    #[test]
    fn test_health_monitor_report() {
        let mut monitor = HealthMonitor::new();

        monitor.add_check(HealthCheck::healthy("component_a"));
        monitor.add_check(HealthCheck::degraded("component_b", "running slow"));

        let report = monitor.report();
        assert!(report.contains("System Status: Degraded"));
        assert!(report.contains("component_a"));
        assert!(report.contains("component_b"));
        assert!(report.contains("running slow"));
    }

    #[test]
    fn test_health_config_default() {
        let config = HealthConfig::default();
        assert_eq!(config.degraded_latency_ms, 100);
        assert_eq!(config.unhealthy_latency_ms, 1000);
        assert_eq!(config.check_interval, Duration::from_secs(10));
    }
}
