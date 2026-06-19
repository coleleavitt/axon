use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::signal::{Phase, Priority, Signal};

pub trait Gate<P>: fmt::Debug {
    fn admits(&self, signal: &Signal<P>) -> bool;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Allow;

impl<P> Gate<P> for Allow {
    fn admits(&self, _signal: &Signal<P>) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DropSignal;

impl<P> Gate<P> for DropSignal {
    fn admits(&self, _signal: &Signal<P>) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MinPriority {
    minimum: Priority,
}

impl MinPriority {
    pub const fn new(minimum: Priority) -> Self {
        Self { minimum }
    }

    pub const fn minimum(self) -> Priority {
        self.minimum
    }
}

impl<P> Gate<P> for MinPriority {
    fn admits(&self, signal: &Signal<P>) -> bool {
        signal.priority() >= self.minimum
    }
}

/// A phase gate: admits a signal only when its [`Phase`] is one this route is
/// active in. Discrete communication-through-coherence — the same graph routes
/// differently per phase without rewiring (temporal multiplexing).
#[derive(Debug, Clone)]
pub struct PhaseGate {
    active: Vec<Phase>,
}

impl PhaseGate {
    /// Active in any of the given phases.
    pub fn new(phases: impl IntoIterator<Item = Phase>) -> Self {
        Self {
            active: phases.into_iter().collect(),
        }
    }

    /// Active in exactly one phase.
    pub fn at(phase: Phase) -> Self {
        Self::new([phase])
    }
}

impl<P> Gate<P> for PhaseGate {
    fn admits(&self, signal: &Signal<P>) -> bool {
        self.active.contains(&signal.phase())
    }
}

/// A shared, cloneable switch that top-down state toggles to *release* a
/// [`Disinhibit`] gate. All clones share one flag (like a neuromodulator
/// broadcast).
#[derive(Debug, Clone, Default)]
pub struct Release {
    released: Arc<AtomicBool>,
}

impl Release {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the disinhibitory gate (VIP fires, suppressing the inhibitor).
    pub fn release(&self) {
        self.released.store(true, Ordering::Relaxed);
    }

    /// Re-engage the inhibitor (stop releasing).
    pub fn hold(&self) {
        self.released.store(false, Ordering::Relaxed);
    }

    pub fn is_released(&self) -> bool {
        self.released.load(Ordering::Relaxed)
    }
}

/// A disinhibitory gate combinator (the VIP→SST/PV motif): it normally defers to
/// `inhibitor`, but while its [`Release`] switch is active it admits regardless,
/// *opening* an otherwise-closed route for top-down signals. This is distinct
/// from a flat threshold — attention/top-down state releases the route rather
/// than lowering a bar.
#[derive(Debug, Clone)]
pub struct Disinhibit<I> {
    inhibitor: I,
    release: Release,
}

impl<I> Disinhibit<I> {
    pub const fn new(inhibitor: I, release: Release) -> Self {
        Self { inhibitor, release }
    }
}

impl<P, I: Gate<P>> Gate<P> for Disinhibit<I> {
    fn admits(&self, signal: &Signal<P>) -> bool {
        self.release.is_released() || self.inhibitor.admits(signal)
    }
}
