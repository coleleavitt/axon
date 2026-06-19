use std::fmt;

use crate::edge::EdgeId;
use crate::gate::{Allow, Gate};
use crate::id::{EndpointId, ModuleId};
use crate::signal::Signal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Weight(i16);

impl Weight {
    pub const fn new(value: i16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i16 {
        self.0
    }
}

impl fmt::Display for Weight {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// The energy/spend a route costs to traverse — tokens, latency, or dollars for
/// a coding SDK calling paid endpoints. Default `0` (free), so untagged routes
/// never affect a budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cost(u32);

impl Cost {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Round a real-valued weight adjustment to the nearest `i16`, saturating at
/// the bounds. Shared by plastic decay and the [`Plasticity`](crate::Plasticity)
/// policy so both clamp learning identically.
pub(crate) fn round_to_i16(value: f32) -> i16 {
    let rounded = value.round();
    if rounded >= f32::from(i16::MAX) {
        i16::MAX
    } else if rounded <= f32::from(i16::MIN) {
        i16::MIN
    } else {
        rounded as i16
    }
}

pub struct Route<P> {
    from: EndpointId,
    to: ModuleId,
    /// The static prior set at construction.
    base: Weight,
    /// The plastic component accumulated through reinforcement; the effective
    /// weight is `base + learned`. Kept separate so decay can pull learning back
    /// toward the prior without erasing it, and so only `learned` is persisted.
    learned: i16,
    cost: Cost,
    gate: Box<dyn Gate<P> + Send + Sync + 'static>,
}

impl<P> Route<P> {
    pub fn new<G>(from: EndpointId, to: ModuleId, weight: Weight, gate: G) -> Self
    where
        G: Gate<P> + Send + Sync + 'static,
    {
        Self {
            from,
            to,
            base: weight,
            learned: 0,
            cost: Cost::new(0),
            gate: Box::new(gate),
        }
    }

    /// Tag this route with a traversal [`Cost`] for budget-aware routing.
    #[must_use]
    pub fn with_cost(mut self, cost: Cost) -> Self {
        self.cost = cost;
        self
    }

    pub fn open(from: EndpointId, to: ModuleId, weight: Weight) -> Self {
        Self::new(from, to, weight, Allow)
    }

    pub const fn from(&self) -> &EndpointId {
        &self.from
    }

    pub const fn to(&self) -> &ModuleId {
        &self.to
    }

    /// The effective weight used for selection: the static `base` prior plus the
    /// `learned` plastic component, saturating at the `i16` bounds.
    pub const fn weight(&self) -> Weight {
        Weight(self.base.0.saturating_add(self.learned))
    }

    /// The static prior, before any reinforcement.
    pub const fn base_weight(&self) -> Weight {
        self.base
    }

    /// The plastic component accumulated through reinforcement.
    pub const fn learned(&self) -> i16 {
        self.learned
    }

    /// This route's traversal cost.
    pub const fn cost(&self) -> Cost {
        self.cost
    }

    /// This route's `(from, to)` identity.
    pub fn edge(&self) -> EdgeId {
        EdgeId::new(self.from.clone(), self.to.clone())
    }

    /// Strengthen (`delta > 0`) or weaken (`delta < 0`) the learned component,
    /// saturating at the `i16` bounds.
    pub fn reinforce(&mut self, delta: i16) {
        self.learned = self.learned.saturating_add(delta);
    }

    /// Pull the learned component toward zero by `rate` (clamped to `[0.0, 1.0]`),
    /// preserving the static prior. `rate = 0.0` keeps all learning; `rate = 1.0`
    /// forgets it entirely.
    pub fn decay(&mut self, rate: f32) {
        let rate = rate.clamp(0.0, 1.0);
        self.learned = round_to_i16(f32::from(self.learned) * (1.0 - rate));
    }

    /// Overwrite the learned component outright — used to restore a persisted
    /// weight snapshot.
    pub fn set_learned(&mut self, learned: i16) {
        self.learned = learned;
    }

    pub fn admits(&self, signal: &Signal<P>) -> bool {
        self.gate.admits(signal)
    }
}

impl<P> fmt::Debug for Route<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Route")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("weight", &self.weight())
            .field("base", &self.base)
            .field("learned", &self.learned)
            .field("cost", &self.cost)
            .field("gate", &self.gate)
            .finish()
    }
}
