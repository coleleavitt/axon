use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::gate::Gate;
use crate::signal::Signal;

/// A circuit breaker [`Gate`] for hub robustness: it admits signals while
/// closed, but once recorded failures reach its threshold it *opens* and denies
/// everything, isolating the route so a failing module cannot cascade through
/// the graph.
///
/// The breaker is a cheap, cloneable handle over shared atomic state, so the
/// clone installed on a [`Route`](crate::Route) and the clone a supervisor keeps
/// to call [`trip`](Self::trip)/[`reset`](Self::reset) observe the same circuit.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    failures: Arc<AtomicUsize>,
    threshold: usize,
}

impl CircuitBreaker {
    /// Create a closed breaker that opens after `threshold` failures. A
    /// `threshold` of zero is treated as one (it opens on the first failure).
    pub fn new(threshold: usize) -> Self {
        Self {
            failures: Arc::new(AtomicUsize::new(0)),
            threshold: threshold.max(1),
        }
    }

    /// Record one failure. The breaker opens once recorded failures reach the
    /// threshold.
    pub fn trip(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Close the breaker again, clearing the failure count.
    pub fn reset(&self) {
        self.failures.store(0, Ordering::Relaxed);
    }

    /// Number of failures recorded since the last reset.
    pub fn failures(&self) -> usize {
        self.failures.load(Ordering::Relaxed)
    }

    /// Whether the circuit is open (denying all signals).
    pub fn is_open(&self) -> bool {
        self.failures() >= self.threshold
    }
}

impl<P> Gate<P> for CircuitBreaker {
    fn admits(&self, _signal: &Signal<P>) -> bool {
        !self.is_open()
    }
}
