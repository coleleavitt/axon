use std::collections::HashMap;
use std::collections::hash_map::Entry;

use crate::edge::EdgeId;
use crate::error::RuntimeError;
use crate::event::RunEvent;
use crate::gate::Gate;
use crate::id::{EndpointId, InputId, ModuleId};
use crate::limit::StepLimit;
use crate::module::{Module, ModuleOutput};
use crate::plasticity::{Plasticity, Reinforcement};
use crate::report::{RunReport, RunStatus, TraceStep};
use crate::rng::{DEFAULT_SEED, Rng};
use crate::route::{Route, Weight};
use crate::routing::RoutingTable;
use crate::signal::Signal;

#[derive(Debug)]
pub struct Runtime<P> {
    modules: HashMap<ModuleId, Box<dyn Module<P>>>,
    routing: RoutingTable<P>,
    step_limit: StepLimit,
    /// Softmax temperature for route selection — the load-bearing `exploration`
    /// neuromodulator. `0.0` (default) means deterministic argmax selection.
    exploration: f32,
    /// The seed this runtime's [`Rng`] was created from, surfaced for replay.
    seed: u64,
    rng: Rng,
}

impl<P> Runtime<P> {
    pub fn new(step_limit: StepLimit) -> Self {
        Self {
            modules: HashMap::new(),
            routing: RoutingTable::new(),
            step_limit,
            exploration: 0.0,
            seed: DEFAULT_SEED,
            rng: Rng::seeded(DEFAULT_SEED),
        }
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
        if let EndpointId::Module(module) = &from {
            if !self.modules.contains_key(module) {
                return Err(RuntimeError::MissingModule { id: module.clone() });
            }
        }
        if !self.modules.contains_key(&to) {
            return Err(RuntimeError::MissingModule { id: to });
        }
        self.routing.push(Route::new(from, to, weight, gate));
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

        loop {
            if steps_taken >= self.step_limit.get() {
                return Err(RuntimeError::StepLimitExceeded {
                    limit: self.step_limit,
                    at,
                });
            }

            let selected = if self.exploration > 0.0 {
                self.routing
                    .select_modulated(&at, &signal, self.exploration, &mut self.rng)?
            } else {
                self.routing.select(&at, &signal)?
            };
            let Some(route) = selected else {
                return Ok(RunReport::new(RunStatus::NoRoute { at, signal }, steps));
            };

            let to = route.to().clone();
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

    /// Decay all learned routing weights toward their priors — see
    /// [`RoutingTable::decay`].
    pub fn decay(&mut self, rate: f32) {
        self.routing.decay(rate);
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
