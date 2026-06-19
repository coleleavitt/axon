//! An async counterpart to [`Runtime`](crate::Runtime) for modules that do
//! real I/O — tool calls, network, model providers. It is deliberately
//! runtime-agnostic: modules return boxed futures and the caller awaits the run
//! on whatever executor they already use. The core depends on no async runtime.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

use crate::error::RuntimeError;
use crate::event::RunEvent;
use crate::gate::Gate;
use crate::id::{EndpointId, InputId, ModuleId};
use crate::limit::StepLimit;
use crate::module::{ModuleError, ModuleOutput};
use crate::report::{RunReport, RunStatus, TraceStep};
use crate::route::{Route, Weight};
use crate::routing::RoutingTable;
use crate::signal::Signal;
use crate::stop::StopToken;

/// A heap-allocated future, so the [`AsyncModule`] trait stays dyn-compatible.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// The async analogue of [`Module`](crate::Module): `handle` returns a future
/// instead of a value, so a module can await I/O before producing its output.
pub trait AsyncModule<P>: fmt::Debug {
    fn id(&self) -> &ModuleId;

    fn handle(&mut self, signal: Signal<P>) -> BoxFuture<'_, Result<ModuleOutput<P>, ModuleError>>;
}

/// Routes typed signals between [`AsyncModule`]s, awaiting each module in turn.
#[derive(Debug)]
pub struct AsyncRuntime<P> {
    modules: HashMap<ModuleId, Box<dyn AsyncModule<P>>>,
    routing: RoutingTable<P>,
    step_limit: StepLimit,
    stop: Option<StopToken>,
}

impl<P> AsyncRuntime<P> {
    pub fn new(step_limit: StepLimit) -> Self {
        Self {
            modules: HashMap::new(),
            routing: RoutingTable::new(),
            step_limit,
            stop: None,
        }
    }

    /// Attach a cooperative cancellation handle, checked at each step boundary
    /// (between awaited modules). See [`Runtime::with_stop_token`](crate::Runtime::with_stop_token).
    #[must_use]
    pub fn with_stop_token(mut self, stop: StopToken) -> Self {
        self.stop = Some(stop);
        self
    }

    pub fn insert_module<M>(&mut self, module: M) -> Result<(), RuntimeError>
    where
        M: AsyncModule<P> + 'static,
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

    pub async fn run_async(
        &mut self,
        entry: InputId,
        initial: Signal<P>,
    ) -> Result<RunReport<P>, RuntimeError> {
        self.run_async_observed(entry, initial, &mut |_event| {})
            .await
    }

    /// Like [`run_async`](Self::run_async), streaming each [`RunEvent`] to
    /// `observer` as it happens.
    pub async fn run_async_observed(
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

            let Some(route) = self.routing.select(&at, &signal)? else {
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

            match module.handle(signal).await {
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
}

impl<P> Default for AsyncRuntime<P> {
    fn default() -> Self {
        Self::new(StepLimit::default())
    }
}
