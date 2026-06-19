use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::gate::Gate;
use crate::signal::Signal;

/// The health state of a [`CircuitBreaker`]: fully healthy, degraded (some
/// failures but still admitting), or open (isolating the route).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    Closed,
    Degraded,
    Open,
}

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
    half_open: Arc<AtomicBool>,
    threshold: usize,
}

impl CircuitBreaker {
    /// Create a closed breaker that opens after `threshold` failures. A
    /// `threshold` of zero is treated as one (it opens on the first failure).
    pub fn new(threshold: usize) -> Self {
        Self {
            failures: Arc::new(AtomicUsize::new(0)),
            half_open: Arc::new(AtomicBool::new(false)),
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

    /// Record a success: recover fully (clear failures) and cancel any pending
    /// half-open probe. Drives the half-open → closed transition.
    pub fn record_success(&self) {
        self.failures.store(0, Ordering::Relaxed);
        self.half_open.store(false, Ordering::Relaxed);
    }

    /// Record a failure (like [`trip`](Self::trip)) and cancel any pending
    /// half-open probe, so a failed probe re-opens the circuit.
    pub fn record_failure(&self) {
        self.trip();
        self.half_open.store(false, Ordering::Relaxed);
    }

    /// While open, allow a single probe signal through to test recovery
    /// (half-open). The next [`admits`](Gate::admits) consumes the permit; follow
    /// it with [`record_success`](Self::record_success) to close or
    /// [`record_failure`](Self::record_failure) to re-open.
    pub fn allow_probe(&self) {
        if self.is_open() {
            self.half_open.store(true, Ordering::Relaxed);
        }
    }

    /// Per-route health in `[0.0, 1.0]`: `1.0` fully closed, `0.0` fully open,
    /// in between degraded. The hub-robustness score a supervisor watches.
    pub fn health(&self) -> f32 {
        let failures = self.failures().min(self.threshold);
        1.0 - (failures as f32 / self.threshold as f32)
    }

    /// The current [`BreakerState`].
    pub fn state(&self) -> BreakerState {
        let failures = self.failures();
        if failures == 0 {
            BreakerState::Closed
        } else if failures >= self.threshold {
            BreakerState::Open
        } else {
            BreakerState::Degraded
        }
    }
}

impl<P> Gate<P> for CircuitBreaker {
    fn admits(&self, _signal: &Signal<P>) -> bool {
        if !self.is_open() {
            return true;
        }
        // Open: let a single half-open probe through, consuming the permit so the
        // circuit stays isolated until the probe's outcome is recorded.
        self.half_open.swap(false, Ordering::AcqRel)
    }
}
