use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use crate::edge::EdgeId;
use crate::event::RunEvent;
use crate::graph::ModuleGraph;
use crate::id::EndpointId;
use crate::plasticity::{Credit, Plasticity, Reinforcement};
use crate::profile::RoutingProfile;
use crate::report::TraceStep;
use crate::rng::Rng;
use crate::route::{Route, Sign, Weight, round_to_i16};
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

    /// Build the read-only [`ModuleGraph`] of this table's wiring — the
    /// connectome over which degree, reachability, and hubs are computed.
    pub fn graph(&self) -> ModuleGraph {
        let mut graph = ModuleGraph::new();
        for route in &self.routes {
            graph.insert(route.from().clone(), route.to().clone());
        }
        graph
    }

    pub fn select<'a>(
        &'a self,
        from: &EndpointId,
        signal: &Signal<P>,
    ) -> Result<Option<&'a Route<P>>, RoutingError> {
        let mut selected = None;
        let mut ambiguous = false;
        // Total inhibition admitted from this source; it suppresses the
        // excitatory competition rather than competing for selection itself.
        let mut inhibition: i32 = 0;

        for route in self
            .routes
            .iter()
            .filter(|route| route.from() == from && route.admits(signal))
        {
            if route.sign() == Sign::Inhibitory {
                inhibition = inhibition.saturating_add(i32::from(route.weight().get()));
                continue;
            }
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

        // Inhibitory veto: when inhibition is present and overwhelms the best
        // excitatory option, no route fires (default tonic inhibition).
        if inhibition > 0 {
            if let Some(route) = selected {
                if i32::from(route.weight().get()) <= inhibition {
                    return Ok(None);
                }
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
            .filter(|route| route.from() == from && route.is_excitatory() && route.admits(signal))
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

    /// Argmax selection under a NoGo discipline: the winner must beat the
    /// runner-up by at least `margin` (effective weight) or selection returns
    /// `None` — "decide when *not* to act". A sole admitted candidate always
    /// wins. Unlike [`select`](Self::select) this never raises `AmbiguousRoute`:
    /// a tie is simply not a clear-enough winner, so it is suppressed.
    ///
    /// This is the basal-ganglia default-inhibition rule — without a margin a
    /// marginally-best route fires even when no option is clearly right.
    pub fn select_with_margin<'a>(
        &'a self,
        from: &EndpointId,
        signal: &Signal<P>,
        margin: i16,
    ) -> Result<Option<&'a Route<P>>, RoutingError> {
        let mut best: Option<&Route<P>> = None;
        let mut runner_up: Option<Weight> = None;
        for route in self
            .routes
            .iter()
            .filter(|route| route.from() == from && route.is_excitatory() && route.admits(signal))
        {
            match best {
                None => best = Some(route),
                Some(current) if route.weight() > current.weight() => {
                    runner_up = Some(current.weight());
                    best = Some(route);
                }
                Some(_) => {
                    let better_runner_up = match runner_up {
                        Some(weight) => route.weight() > weight,
                        None => true,
                    };
                    if better_runner_up {
                        runner_up = Some(route.weight());
                    }
                }
            }
        }
        let Some(winner) = best else {
            return Ok(None);
        };
        match runner_up {
            Some(second) => {
                let gap = winner.weight().get().saturating_sub(second.get());
                Ok((gap >= margin).then_some(winner))
            }
            None => Ok(Some(winner)),
        }
    }

    /// Select the top-`limit` admitted routes from `from`, ranked by effective
    /// weight (descending, ties broken by target id for determinism).
    ///
    /// This is the multicast / parallel-recruitment primitive: a single event
    /// (an alert that should both write memory *and* interrupt the planner) can
    /// drive several modules at once. Single-winner [`select`](Self::select) stays
    /// the default; callers opt into fan-out explicitly.
    pub fn select_all<'a>(
        &'a self,
        from: &EndpointId,
        signal: &Signal<P>,
        limit: usize,
    ) -> Vec<&'a Route<P>> {
        let mut admitted: Vec<&Route<P>> = self
            .routes
            .iter()
            .filter(|route| route.from() == from && route.is_excitatory() && route.admits(signal))
            .collect();
        admitted.sort_by(|left, right| {
            right
                .weight()
                .cmp(&left.weight())
                .then_with(|| left.to().cmp(right.to()))
        });
        admitted.truncate(limit);
        admitted
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

    /// Apply a [`RoutingProfile`]'s per-edge biases, replacing any prior profile
    /// (edges not in the profile are reset to zero bias). Swapping profiles
    /// reconfigures which routes win on the same graph.
    pub fn apply_profile(&mut self, profile: &RoutingProfile) {
        for route in &mut self.routes {
            route.set_bias(profile.get(&route.edge()));
        }
    }

    /// Clear any active routing profile, returning every edge to `base + learned`.
    pub fn clear_profile(&mut self) {
        for route in &mut self.routes {
            route.set_bias(0);
        }
    }

    /// Decay every learned weight toward its static prior by `rate` — synaptic
    /// forgetting that stops unreinforced learning from accumulating forever.
    pub fn decay(&mut self, rate: f32) {
        for route in &mut self.routes {
            route.decay(rate);
        }
    }

    /// The total magnitude of learned weight across all edges (the L1 norm of the
    /// plastic component) — the quantity homeostasis keeps bounded.
    pub fn learned_magnitude(&self) -> u64 {
        self.routes
            .iter()
            .map(|route| u64::from(route.learned().unsigned_abs()))
            .sum()
    }

    /// Homeostatic synaptic scaling: if the total learned magnitude exceeds
    /// `max_total`, multiplicatively rescale every learned weight to bring it back
    /// to `max_total`, preserving relative strengths.
    ///
    /// This is the E/I-balance brake — a slow global renormalization that stops
    /// repeated reinforcement from blowing the graph up (or, after decay, lets it
    /// settle), without disturbing what was learned *relative* to everything else.
    /// Weights already within budget are left untouched.
    pub fn homeostatic_scale(&mut self, max_total: u64) {
        let total = self.learned_magnitude();
        if total <= max_total || total == 0 {
            return;
        }
        let factor = max_total as f32 / total as f32;
        for route in &mut self.routes {
            route.set_learned(round_to_i16(f32::from(route.learned()) * factor));
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
