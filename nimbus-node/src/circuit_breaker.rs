//! Circuit breaker pattern for external service calls (RPC, Vault, Guardian)
//!
//! Implements the standard three-state circuit breaker:
//! - CLOSED: Normal operation, requests pass through
//! - OPEN: Service is down, requests fail fast without attempting
//! - HALF-OPEN: Trial period, limited requests to test recovery
//!
//! Inspired by:
//! - OpenZeppelin/openzeppelin-relayer circuit breaker (2026)
//! - reliability-toolkit-rs async primitives
//! - tower-resilience-circuitbreaker patterns

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    /// Normal operation -- requests pass through
    Closed,
    /// Service is down -- requests fail fast
    Open,
    /// Trial period -- limited requests to test if service recovered
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "CLOSED"),
            CircuitState::Open => write!(f, "OPEN"),
            CircuitState::HalfOpen => write!(f, "HALF-OPEN"),
        }
    }
}

/// Configuration for a circuit breaker instance
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit
    pub failure_threshold: u32,
    /// Duration to keep circuit open before transitioning to half-open
    pub recovery_timeout: Duration,
    /// Number of successful requests in half-open state before closing
    pub success_threshold: u32,
    /// Human-readable name for logging
    pub name: String,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
            name: "default".to_string(),
        }
    }
}

/// Internal mutable state of the circuit breaker
struct CircuitBreakerInner {
    state: CircuitState,
    consecutive_failures: u32,
    consecutive_successes: u32,
    last_failure_time: Option<Instant>,
    total_requests: u64,
    total_failures: u64,
    total_short_circuits: u64,
}

/// Thread-safe async circuit breaker
#[derive(Clone)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    inner: Arc<Mutex<CircuitBreakerInner>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            inner: Arc::new(Mutex::new(CircuitBreakerInner {
                state: CircuitState::Closed,
                consecutive_failures: 0,
                consecutive_successes: 0,
                last_failure_time: None,
                total_requests: 0,
                total_failures: 0,
                total_short_circuits: 0,
            })),
        }
    }

    /// Execute a fallible async operation through the circuit breaker.
    ///
    /// Returns:
    /// - Ok(T) if the operation succeeds
    /// - Err(String) if the circuit is open (fail fast) or the operation fails
    pub async fn call<F, Fut, T>(&self, operation: F) -> Result<T, String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        // Pre-check: should we allow this request?
        {
            let mut inner = self.inner.lock().await;
            inner.total_requests += 1;

            match inner.state {
                CircuitState::Open => {
                    // Check if recovery timeout has elapsed
                    if let Some(last_fail) = inner.last_failure_time {
                        if last_fail.elapsed() >= self.config.recovery_timeout {
                            // Transition to half-open
                            inner.state = CircuitState::HalfOpen;
                            inner.consecutive_successes = 0;
                            println!(
                                "CIRCUIT-BREAKER [{}]: OPEN -> HALF-OPEN (recovery timeout elapsed)",
                                self.config.name
                            );
                        } else {
                            // Still open, fail fast
                            inner.total_short_circuits += 1;
                            let remaining = self.config.recovery_timeout - last_fail.elapsed();
                            return Err(format!(
                                "Circuit breaker [{}] is OPEN. Retry in {:.1}s",
                                self.config.name,
                                remaining.as_secs_f64()
                            ));
                        }
                    }
                }
                CircuitState::Closed => {
                    // Normal operation, proceed
                }
                CircuitState::HalfOpen => {
                    // Allow limited requests through
                }
            }
        }

        // Execute the operation
        match operation().await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(e)
            }
        }
    }

    /// Record a successful operation
    async fn on_success(&self) {
        let mut inner = self.inner.lock().await;
        inner.consecutive_failures = 0;
        inner.consecutive_successes += 1;

        if inner.state == CircuitState::HalfOpen
            && inner.consecutive_successes >= self.config.success_threshold
        {
            inner.state = CircuitState::Closed;
            inner.consecutive_successes = 0;
            println!(
                "CIRCUIT-BREAKER [{}]: HALF-OPEN -> CLOSED (service recovered after {} successes)",
                self.config.name, self.config.success_threshold
            );
        }
    }

    /// Record a failed operation
    async fn on_failure(&self) {
        let mut inner = self.inner.lock().await;
        inner.consecutive_failures += 1;
        inner.consecutive_successes = 0;
        inner.total_failures += 1;
        inner.last_failure_time = Some(Instant::now());

        if inner.state == CircuitState::HalfOpen {
            // Any failure in half-open immediately re-opens the circuit
            inner.state = CircuitState::Open;
            println!(
                "CIRCUIT-BREAKER [{}]: HALF-OPEN -> OPEN (failure during recovery)",
                self.config.name
            );
        } else if inner.state == CircuitState::Closed
            && inner.consecutive_failures >= self.config.failure_threshold
        {
            inner.state = CircuitState::Open;
            println!(
                "CIRCUIT-BREAKER [{}]: CLOSED -> OPEN (hit failure threshold: {} consecutive failures)",
                self.config.name, inner.consecutive_failures
            );
        }
    }

    /// Get the current state of the circuit breaker (for health checks)
    #[allow(dead_code)]
    pub async fn state(&self) -> CircuitState {
        let inner = self.inner.lock().await;
        inner.state
    }

    /// Get diagnostic stats for monitoring
    #[allow(dead_code)]
    pub async fn stats(&self) -> CircuitBreakerStats {
        let inner = self.inner.lock().await;
        CircuitBreakerStats {
            name: self.config.name.clone(),
            state: inner.state,
            consecutive_failures: inner.consecutive_failures,
            total_requests: inner.total_requests,
            total_failures: inner.total_failures,
            total_short_circuits: inner.total_short_circuits,
        }
    }
}

/// Diagnostic stats for monitoring and health check endpoints
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CircuitBreakerStats {
    pub name: String,
    pub state: CircuitState,
    pub consecutive_failures: u32,
    pub total_requests: u64,
    pub total_failures: u64,
    pub total_short_circuits: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_stays_closed_on_success() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_millis(100),
            success_threshold: 2,
            name: "test-success".to_string(),
        });

        // Successful calls should keep circuit closed
        for _ in 0..10 {
            let result = cb.call(|| async { Ok::<_, String>("ok") }).await;
            assert!(result.is_ok());
        }
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(5),
            success_threshold: 2,
            name: "test-failure".to_string(),
        });

        // Fail 3 times to trigger open
        for _ in 0..3 {
            let _ = cb
                .call(|| async { Err::<String, _>("fail".to_string()) })
                .await;
        }
        assert_eq!(cb.state().await, CircuitState::Open);

        // Next call should fail fast
        let result = cb.call(|| async { Ok::<_, String>("ok") }).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("OPEN"));
    }

    #[tokio::test]
    async fn test_circuit_breaker_recovers() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(50),
            success_threshold: 2,
            name: "test-recovery".to_string(),
        });

        // Open the circuit
        for _ in 0..2 {
            let _ = cb
                .call(|| async { Err::<String, _>("fail".to_string()) })
                .await;
        }
        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(60)).await;

        // First success transitions to half-open, then closed after threshold
        let r1 = cb
            .call(|| async { Ok::<_, String>("ok".to_string()) })
            .await;
        assert!(r1.is_ok());
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        let r2 = cb
            .call(|| async { Ok::<_, String>("ok".to_string()) })
            .await;
        assert!(r2.is_ok());
        assert_eq!(cb.state().await, CircuitState::Closed);
    }
}
