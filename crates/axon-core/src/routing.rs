use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use crate::edge::EdgeId;
use crate::event::RunEvent;
use crate::id::EndpointId;
use crate::plasticity::{Credit, Plasticity, Reinforcement};
use crate::report::TraceStep;
use crate::rng::Rng;
use crate::route::{Route, Weight};
use crate::signal::Signal;

#[derive(Debug)]
pub struct RoutingTable<P> {
    routes: Vec<Route<P>>,
}

impl<P> RoutingTable<P> {
    pub const fn new() -> Self {
        Self { routes: Vec::new() }
    }

    pub fn push(&mut self, route: Route<P>) {
        self.routes.push(route);
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    pub fn routes(&self) -> impl Iterator<Item = &Route<P>> {
        self.routes.iter()
    }

    pub fn select<'a>(
        &'a self,
        from: &EndpointId,
        signal: &Signal<P>,
    ) -> Result<Option<&'a Route<P>>, RoutingError> {
        let mut selected = None;
        let mut ambiguous = false;

        for route in self
            .routes
            .iter()
            .filter(|route| route.from() == from && route.admits(signal))
        {
            match selected {
                None => {
                    selected = Some(route);
                    ambiguous = false;
                }
                Some(current) => match route.weight().cmp(&current.weight()) {
                    Ordering::Greater => {
                        selected = Some(route);
                        ambiguous = false;
                    }
                    Ordering::Equal => ambiguous = true,
                    Ordering::Less => {}
                },
            }
        }

        if ambiguous {
            match selected {
                Some(route) => Err(RoutingError::AmbiguousRoute {
                    from: from.clone(),
                    weight: route.weight(),
                }),
                None => Ok(None),
            }
        } else {
            Ok(selected)
        }
    }

    /// Select a route stochastically, with `exploration` as a softmax
    /// (Boltzmann) temperature over admitted routes' effective weights.
    ///
    /// This is how the otherwise-dead `exploration` neuromodulator becomes
    /// load-bearing: near `0.0` it collapses to the deterministic
    /// [`select`](Self::select) argmax (exploit); larger values flatten the
    /// distribution so lower-weight routes win more often (explore). Sampling
    /// resolves ties, so this path never returns `AmbiguousRoute`. All draws come
    /// from the supplied seeded [`Rng`], keeping stochastic runs reproducible.
    pub fn select_modulated<'a>(
        &'a self,
        from: &EndpointId,
        signal: &Signal<P>,
        exploration: f32,
        rng: &mut Rng,
    ) -> Result<Option<&'a Route<P>>, RoutingError> {
        if exploration <= 0.0 {
            return self.select(from, signal);
        }
        let candidates: Vec<&Route<P>> = self
            .routes
            .iter()
            .filter(|route| route.from() == from && route.admits(signal))
            .collect();
        let Some(max) = candidates.iter().map(|route| route.weight().get()).max() else {
            return Ok(None);
        };
        // Boltzmann weights, shifted by the max for numerical stability.
        let max = f32::from(max);
        let weights: Vec<f32> = candidates
            .iter()
            .map(|route| ((f32::from(route.weight().get()) - max) / exploration).exp())
            .collect();
        let total: f32 = weights.iter().sum();
        let mut threshold = rng.next_f32() * total;
        for (route, weight) in candidates.iter().zip(&weights) {
            threshold -= *weight;
            if threshold <= 0.0 {
                return Ok(Some(*route));
            }
        }
        // Floating-point shortfall: fall back to the last admitted candidate.
        Ok(candidates.last().copied())
    }

    /// Apply graded, eligibility-weighted credit to every edge traversed in
    /// `steps`, mutating learned weights and emitting one
    /// [`RunEvent::Reinforced`] per changed edge.
    ///
    /// Credit is assigned through a TD(λ)-style eligibility trace: edges nearer
    /// the outcome receive more credit than distant ones, so a *delayed* result
    /// (a coding agent's "tests still fail" surfacing many hops after the bad
    /// edit) is attributed across the whole path that produced it rather than
    /// only the last edge — the difference between real credit assignment and
    /// superstitious reinforcement. Default runs that never call this allocate
    /// nothing.
    pub fn reinforce(
        &mut self,
        plasticity: &dyn Plasticity,
        steps: &[TraceStep],
        reinforcement: Reinforcement,
        observer: &mut dyn FnMut(&RunEvent),
    ) {
        for (edge, eligibility) in eligibility_trace(steps, reinforcement.decay) {
            let delta = plasticity.delta(Credit {
                error: reinforcement.error,
                eligibility,
                learning_rate: reinforcement.learning_rate,
            });
            if delta == 0 {
                continue;
            }
            let mut changed = false;
            for route in self.routes.iter_mut().filter(|route| route.edge() == edge) {
                route.reinforce(delta);
                changed = true;
            }
            if changed {
                observer(&RunEvent::Reinforced {
                    edge: edge.clone(),
                    delta,
                });
            }
        }
    }

    /// Decay every learned weight toward its static prior by `rate` — synaptic
    /// forgetting that stops unreinforced learning from accumulating forever.
    pub fn decay(&mut self, rate: f32) {
        for route in &mut self.routes {
            route.decay(rate);
        }
    }

    /// Snapshot the learned (plastic) weight of every edge for persistence,
    /// keyed by edge identity and decoupled from the non-serializable gates.
    pub fn learned_weights(&self) -> Vec<(EdgeId, i16)> {
        self.routes
            .iter()
            .map(|route| (route.edge(), route.learned()))
            .collect()
    }

    /// Restore learned weights from a [`learned_weights`](Self::learned_weights)
    /// snapshot, matching by edge identity. Edges absent from the snapshot keep
    /// their current learning; gates are reconstructed from code at wiring time.
    pub fn restore_learned(&mut self, snapshot: &[(EdgeId, i16)]) {
        for (edge, learned) in snapshot {
            for route in self.routes.iter_mut().filter(|route| route.edge() == *edge) {
                route.set_learned(*learned);
            }
        }
    }
}

/// Fold a trajectory into the terminal eligibility of each distinct edge using a
/// decaying trace: each step discounts all accrued eligibility by `decay`, then
/// the fired edge gains `1.0`. Edges fired later (nearer the outcome) end with
/// higher eligibility, and a repeated edge accumulates.
fn eligibility_trace(steps: &[TraceStep], decay: f32) -> Vec<(EdgeId, f32)> {
    let decay = decay.clamp(0.0, 1.0);
    let mut trace: Vec<(EdgeId, f32)> = Vec::new();
    for step in steps {
        let edge = step.edge();
        for (_, eligibility) in &mut trace {
            *eligibility *= decay;
        }
        match trace.iter_mut().find(|(id, _)| *id == edge) {
            Some((_, eligibility)) => *eligibility += 1.0,
            None => trace.push((edge, 1.0)),
        }
    }
    trace
}

impl<P> Default for RoutingTable<P> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingError {
    AmbiguousRoute { from: EndpointId, weight: Weight },
}

impl fmt::Display for RoutingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AmbiguousRoute { from, weight } => {
                write!(
                    formatter,
                    "ambiguous admitted routes from {from} at weight {weight}"
                )
            }
        }
    }
}

impl Error for RoutingError {}
