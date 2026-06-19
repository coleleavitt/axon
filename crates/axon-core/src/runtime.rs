use std::collections::HashMap;
use std::collections::hash_map::Entry;

use crate::edge::EdgeId;
use crate::error::RuntimeError;
use crate::event::RunEvent;
use crate::gate::Gate;
use crate::graph::ModuleGraph;
use crate::id::{EndpointId, InputId, ModuleId};
use crate::limit::StepLimit;
use crate::module::{Module, ModuleOutput};
use crate::plasticity::{Plasticity, Reinforcement};
use crate::profile::RoutingProfile;
use crate::report::{RunReport, RunStatus, TraceStep};
use crate::rng::{DEFAULT_SEED, Rng};
use crate::route::{Cost, Route, Weight};
use crate::routing::RoutingTable;
use crate::signal::Signal;
use crate::stop::StopToken;

#[derive(Debug)]
pub struct Runtime<P> {
    modules: HashMap<ModuleId, Box<dyn Module<P>>>,
    routing: RoutingTable<P>,
    step_limit: StepLimit,
    /// Optional cooperative cancellation handle, checked at each step boundary.
    stop: Option<StopToken>,
    /// Softmax temperature for route selection — the load-bearing `exploration`
    /// neuromodulator. `0.0` (default) means deterministic argmax selection.
    exploration: f32,
    /// NoGo selection margin: a winner must beat the runner-up by at least this
    /// many weight points or the runtime declines to act. `0` (default) disables.
    margin: i16,
    /// The seed this runtime's [`Rng`] was created from, surfaced for replay.
    seed: u64,
    rng: Rng,
    /// Optional per-run energy budget; a run refuses a route it cannot afford.
    budget: Option<u32>,
    /// Optional loop guard: if any edge fires more than this many times in one
    /// run, the run halts with a stall instead of thrashing to the step limit.
    stall_threshold: Option<usize>,
}

impl<P> Runtime<P> {
    pub fn new(step_limit: StepLimit) -> Self {
        Self {
            modules: HashMap::new(),
            routing: RoutingTable::new(),
            step_limit,
            exploration: 0.0,
            margin: 0,
            seed: DEFAULT_SEED,
            rng: Rng::seeded(DEFAULT_SEED),
            stop: None,
            budget: None,
            stall_threshold: None,
        }
    }

    /// Halt the run if any single edge fires more than `max_repeats` times — a
    /// loop guard against oscillation (e.g. a reinforced feedback edge thrashing
    /// to the step limit). Emits [`RunEvent::Stalled`] and ends with
    /// [`RunStatus::Halted`].
    #[must_use]
    pub const fn with_stall_threshold(mut self, max_repeats: usize) -> Self {
        self.stall_threshold = Some(max_repeats);
        self
    }

    /// Cap the total route cost a single run may spend. When the next route would
    /// exceed the remaining budget it is refused and the run ends with
    /// [`RunStatus::NoRoute`] — shedding load under budget pressure. The budget is
    /// per run (reset each call), keeping runs independent and reproducible.
    #[must_use]
    pub const fn with_budget(mut self, budget: u32) -> Self {
        self.budget = Some(budget);
        self
    }

    /// The per-run energy budget, if any.
    pub const fn budget(&self) -> Option<u32> {
        self.budget
    }

    /// Attach a cooperative cancellation handle. Calling [`StopToken::stop`] on
    /// any clone halts the run at the next step boundary with
    /// [`RunStatus::Halted`].
    #[must_use]
    pub fn with_stop_token(mut self, stop: StopToken) -> Self {
        self.stop = Some(stop);
        self
    }

    /// Set the exploration temperature consulted at selection time. `0.0` keeps
    /// deterministic argmax routing; larger values let lower-weight routes win
    /// more often. Typically driven by `Modulators::exploration()`.
    #[must_use]
    pub fn with_exploration(mut self, exploration: f32) -> Self {
        self.exploration = exploration;
        self
    }

    /// Seed the runtime's RNG so stochastic (exploratory) runs are reproducible.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self.rng = Rng::seeded(seed);
        self
    }

    /// The seed driving stochastic selection — re-creating a runtime
    /// [`with_seed`](Self::with_seed) reproduces the same trajectory sequence.
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// The active exploration temperature.
    pub const fn exploration(&self) -> f32 {
        self.exploration
    }

    /// Require a NoGo selection margin: a winning route must beat the runner-up
    /// by at least `margin` weight points or the runtime declines to act
    /// (the run ends with `NoRoute`). `0` disables. Ignored while `exploration`
    /// is active (stochastic selection resolves competition by sampling).
    #[must_use]
    pub const fn with_margin(mut self, margin: i16) -> Self {
        self.margin = margin;
        self
    }

    /// The active NoGo selection margin.
    pub const fn margin(&self) -> i16 {
        self.margin
    }

    pub fn insert_module<M>(&mut self, module: M) -> Result<(), RuntimeError>
    where
        M: Module<P> + 'static,
    {
        let id = module.id().clone();
        match self.modules.entry(id.clone()) {
            Entry::Vacant(slot) => {
                slot.insert(Box::new(module));
                Ok(())
            }
            Entry::Occupied(_) => Err(RuntimeError::DuplicateModule { id }),
        }
    }

    fn validate_endpoints(&self, from: &EndpointId, to: &ModuleId) -> Result<(), RuntimeError> {
        if let EndpointId::Module(module) = from {
            if !self.modules.contains_key(module) {
                return Err(RuntimeError::MissingModule { id: module.clone() });
            }
        }
        if !self.modules.contains_key(to) {
            return Err(RuntimeError::MissingModule { id: to.clone() });
        }
        Ok(())
    }

    pub fn add_route<G>(
        &mut self,
        from: EndpointId,
        to: ModuleId,
        weight: Weight,
        gate: G,
    ) -> Result<(), RuntimeError>
    where
        G: Gate<P> + Send + Sync + 'static,
    {
        self.validate_endpoints(&from, &to)?;
        self.routing.push(Route::new(from, to, weight, gate));
        Ok(())
    }

    /// Like [`add_route`](Self::add_route), tagging the route with a traversal
    /// [`Cost`] that counts against a [`with_budget`](Self::with_budget) cap.
    pub fn add_route_with_cost<G>(
        &mut self,
        from: EndpointId,
        to: ModuleId,
        weight: Weight,
        cost: Cost,
        gate: G,
    ) -> Result<(), RuntimeError>
    where
        G: Gate<P> + Send + Sync + 'static,
    {
        self.validate_endpoints(&from, &to)?;
        self.routing
            .push(Route::new(from, to, weight, gate).with_cost(cost));
        Ok(())
    }

    /// Add a fully-built [`Route`] (e.g. carrying a [`Sign`](crate::Sign) or
    /// [`Cost`]) after validating its endpoints — the general escape hatch when
    /// the convenience constructors do not cover the route's configuration.
    pub fn add_built_route(&mut self, route: Route<P>) -> Result<(), RuntimeError> {
        self.validate_endpoints(route.from(), route.to())?;
        self.routing.push(route);
        Ok(())
    }

    pub fn add_input_route<G>(
        &mut self,
        from: InputId,
        to: ModuleId,
        weight: Weight,
        gate: G,
    ) -> Result<(), RuntimeError>
    where
        G: Gate<P> + Send + Sync + 'static,
    {
        self.add_route(EndpointId::from(from), to, weight, gate)
    }

    pub fn add_module_route<G>(
        &mut self,
        from: ModuleId,
        to: ModuleId,
        weight: Weight,
        gate: G,
    ) -> Result<(), RuntimeError>
    where
        G: Gate<P> + Send + Sync + 'static,
    {
        self.add_route(EndpointId::from(from), to, weight, gate)
    }

    pub fn run(
        &mut self,
        entry: InputId,
        initial: Signal<P>,
    ) -> Result<RunReport<P>, RuntimeError> {
        self.run_observed(entry, initial, &mut |_event| {})
    }

    /// Drive a signal like [`run`](Self::run), additionally streaming each
    /// [`RunEvent`] to `observer` as the transition happens. `run` is exactly
    /// this with a no-op observer.
    pub fn run_observed(
        &mut self,
        entry: InputId,
        initial: Signal<P>,
        observer: &mut dyn FnMut(&RunEvent),
    ) -> Result<RunReport<P>, RuntimeError> {
        let mut at = EndpointId::from(entry);
        let mut signal = initial;
        let mut steps = Vec::new();
        let mut steps_taken = 0usize;
        let mut spent = 0u32;
        let mut edge_fires: HashMap<EdgeId, usize> = HashMap::new();

        loop {
            // Cooperative cancellation: the global brake, checked each step.
            if self.stop.as_ref().is_some_and(StopToken::is_stopped) {
                observer(&RunEvent::Halted { at: at.clone() });
                return Ok(RunReport::new(RunStatus::Halted { at }, steps));
            }
            if steps_taken >= self.step_limit.get() {
                return Err(RuntimeError::StepLimitExceeded {
                    limit: self.step_limit,
                    at,
                });
            }

            let selected = if self.exploration > 0.0 {
                self.routing
                    .select_modulated(&at, &signal, self.exploration, &mut self.rng)?
            } else if self.margin > 0 {
                self.routing.select_with_margin(&at, &signal, self.margin)?
            } else {
                self.routing.select(&at, &signal)?
            };
            let Some(route) = selected else {
                return Ok(RunReport::new(RunStatus::NoRoute { at, signal }, steps));
            };

            // Budget pressure: refuse a route the run cannot afford.
            let route_cost = route.cost().get();
            if let Some(cap) = self.budget {
                if spent.saturating_add(route_cost) > cap {
                    return Ok(RunReport::new(RunStatus::NoRoute { at, signal }, steps));
                }
            }
            spent = spent.saturating_add(route_cost);

            let to = route.to().clone();

            // Loop guard: break an oscillating edge before it thrashes to the limit.
            if let Some(max_repeats) = self.stall_threshold {
                let edge = EdgeId::new(at.clone(), to.clone());
                let fires = edge_fires.entry(edge.clone()).or_insert(0);
                *fires += 1;
                if *fires > max_repeats {
                    let count = *fires;
                    observer(&RunEvent::Stalled { edge, count });
                    return Ok(RunReport::new(RunStatus::Halted { at }, steps));
                }
            }

            let module = self
                .modules
                .get_mut(&to)
                .ok_or_else(|| RuntimeError::MissingModule { id: to.clone() })?;
            steps.push(TraceStep::new(at.clone(), to.clone()));
            steps_taken = steps_taken.saturating_add(1);
            observer(&RunEvent::Entered {
                from: at.clone(),
                to: to.clone(),
            });

            match module.handle(signal) {
                Ok(ModuleOutput::Emit(next)) => {
                    observer(&RunEvent::Emitted { at: to.clone() });
                    at = EndpointId::from(to);
                    signal = next;
                }
                Ok(ModuleOutput::Stop(final_signal)) => {
                    observer(&RunEvent::Stopped { at: to });
                    return Ok(RunReport::new(RunStatus::Stopped(final_signal), steps));
                }
                Ok(ModuleOutput::Drop) => {
                    observer(&RunEvent::Dropped { at: to.clone() });
                    return Ok(RunReport::new(RunStatus::Dropped { at: to }, steps));
                }
                Err(source) => return Err(RuntimeError::Module { id: to, source }),
            }
        }
    }

    /// Apply credit assignment over a finished run's trajectory, mutating route
    /// weights so paths that led to good outcomes strengthen and ones that led
    /// to bad outcomes weaken. Pass `report.steps()` from a prior
    /// [`run`](Self::run); the graded `error` in `reinforcement` comes from the
    /// caller (e.g. `axon_predict::Mismatch::magnitude()`), which keeps the core
    /// free of any prediction dependency. Reinforcement changes are streamed to
    /// `observer` as [`RunEvent::Reinforced`].
    pub fn reinforce(
        &mut self,
        plasticity: &dyn Plasticity,
        steps: &[TraceStep],
        reinforcement: Reinforcement,
        observer: &mut dyn FnMut(&RunEvent),
    ) {
        self.routing
            .reinforce(plasticity, steps, reinforcement, observer);
    }

    /// Apply a [`RoutingProfile`] so the same graph routes differently under the
    /// active mode — see [`RoutingTable::apply_profile`].
    pub fn apply_profile(&mut self, profile: &RoutingProfile) {
        self.routing.apply_profile(profile);
    }

    /// Clear any active routing profile — see [`RoutingTable::clear_profile`].
    pub fn clear_profile(&mut self) {
        self.routing.clear_profile();
    }

    /// Decay all learned routing weights toward their priors — see
    /// [`RoutingTable::decay`].
    pub fn decay(&mut self, rate: f32) {
        self.routing.decay(rate);
    }

    /// Homeostatically rescale learned weights to keep their total magnitude
    /// within `max_total` — see [`RoutingTable::homeostatic_scale`].
    pub fn homeostatic_scale(&mut self, max_total: u64) {
        self.routing.homeostatic_scale(max_total);
    }

    /// The total magnitude of learned weight across all edges — see
    /// [`RoutingTable::learned_magnitude`].
    pub fn learned_magnitude(&self) -> u64 {
        self.routing.learned_magnitude()
    }

    /// Snapshot learned routing weights for persistence — see
    /// [`RoutingTable::learned_weights`].
    pub fn learned_weights(&self) -> Vec<(EdgeId, i16)> {
        self.routing.learned_weights()
    }

    /// Restore learned routing weights from a snapshot — see
    /// [`RoutingTable::restore_learned`].
    pub fn restore_learned(&mut self, snapshot: &[(EdgeId, i16)]) {
        self.routing.restore_learned(snapshot);
    }

    /// The read-only [`ModuleGraph`] of the runtime's wiring — for degree, hub,
    /// and reachability analysis of the registered routes.
    pub fn graph(&self) -> ModuleGraph {
        self.routing.graph()
    }

    /// The top-`limit` modules that would be recruited from `from` for `signal`,
    /// ranked by weight — the targets of a fan-out / multicast dispatch. Single
    /// signal, several modules; the caller drives each. See
    /// [`RoutingTable::select_all`].
    pub fn recruit(&self, from: &EndpointId, signal: &Signal<P>, limit: usize) -> Vec<ModuleId> {
        self.routing
            .select_all(from, signal, limit)
            .iter()
            .map(|route| route.to().clone())
            .collect()
    }

    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    pub fn route_count(&self) -> usize {
        self.routing.len()
    }
}

impl<P> Default for Runtime<P> {
    fn default() -> Self {
        Self::new(StepLimit::default())
    }
}

pub type AgentLoop<P> = Runtime<P>;
