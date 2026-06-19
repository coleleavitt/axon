#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Priority(i16);

impl Priority {
    pub const fn new(value: i16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i16 {
        self.0
    }
}

/// A discrete routing phase — the slot a signal travels in.
///
/// Phases give axon a discrete analogue of neural oscillatory multiplexing
/// ("communication through coherence"): a [`PhaseGate`](crate::PhaseGate) admits a
/// signal only in matching phases, so one fixed graph routes differently per
/// phase without rewiring. Default phase is `0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Phase(u16);

impl Phase {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal<P> {
    payload: P,
    priority: Priority,
    phase: Phase,
}

impl<P> Signal<P> {
    pub fn new(payload: P) -> Self {
        Self::with_priority(payload, Priority::default())
    }

    pub const fn with_priority(payload: P, priority: Priority) -> Self {
        Self {
            payload,
            priority,
            phase: Phase::new(0),
        }
    }

    /// Set the routing phase this signal travels in (see [`Phase`]).
    #[must_use]
    pub const fn with_phase(mut self, phase: Phase) -> Self {
        self.phase = phase;
        self
    }

    pub const fn payload(&self) -> &P {
        &self.payload
    }

    pub const fn priority(&self) -> Priority {
        self.priority
    }

    pub const fn phase(&self) -> Phase {
        self.phase
    }

    pub fn into_payload(self) -> P {
        self.payload
    }

    pub fn map<Q>(self, transform: impl FnOnce(P) -> Q) -> Signal<Q> {
        Signal {
            payload: transform(self.payload),
            priority: self.priority,
            phase: self.phase,
        }
    }
}
