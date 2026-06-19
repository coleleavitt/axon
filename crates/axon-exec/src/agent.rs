//! The plan -> act -> observe loop expressed as `axon-core` modules wired by
//! routes and content-addressed gates, driven by [`axon_core::Runtime`].
//!
//! This is the integrated counterpart to [`crate::Executor`]: instead of one
//! struct privately owning every cognitive layer and running its own loop, each
//! responsibility is a [`Module`] and every hand-off is a gated [`Route`]. The
//! transmission substrate (the core) does the routing; the layers only compute.
//!
//! [`Route`]: axon_core::Route

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use axon_core::{
    Gate,
    InputId,
    Module,
    ModuleError,
    ModuleId,
    ModuleOutput,
    Runtime,
    RuntimeError,
    Signal,
    Weight,
};
use axon_memory::{Episode, EpisodicStore, MemoryStore};
use axon_modulate::Modulators;
use axon_predict::{Outcome, Prediction, Verifier};
use axon_workspace::{Broadcast, Workspace};

use crate::learning::OutcomeError;
use crate::{Decision, Plan, decide};

/// The typed payload that flows around the routed agent loop. Each variant names
/// the stage of the cycle it belongs to, and the gates below admit on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentSignal {
    /// Kick the loop off from an input endpoint.
    Goal,
    /// The planner has selected the action for `step`.
    Act {
        step: usize,
        action: String,
        prediction: Prediction,
    },
    /// The tool has produced an observation for `step`.
    Observe {
        step: usize,
        action: String,
        prediction: Prediction,
        observed: String,
    },
    /// The executive released the loop to plan `next`.
    Advance { next: usize },
    /// The executive asks the planner to re-plan after a severe mismatch at
    /// `step`, rather than retrying the same failing step.
    Replan { step: usize, reason: String },
    /// Terminal: the loop stopped, with a human-readable reason.
    Halt { reason: String },
}

/// Default release weight for every loop edge. Equal weights keep selection
/// unambiguous because each source has exactly one admitting gate per stage.
const LOOP_WEIGHT: Weight = Weight::new(10);

macro_rules! stage_gate {
    ($(#[$doc:meta])* $name:ident => $pattern:pat) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl Gate<AgentSignal> for $name {
            fn admits(&self, signal: &Signal<AgentSignal>) -> bool {
                matches!(signal.payload(), $pattern)
            }
        }
    };
}

stage_gate!(
    /// Admits only the initial [`AgentSignal::Goal`].
    OnGoal => AgentSignal::Goal
);
stage_gate!(
    /// Admits only [`AgentSignal::Act`] hand-offs to the tool.
    OnAct => AgentSignal::Act { .. }
);
stage_gate!(
    /// Admits only [`AgentSignal::Observe`] hand-offs to the executive.
    OnObserve => AgentSignal::Observe { .. }
);
stage_gate!(
    /// Admits only [`AgentSignal::Advance`] feedback to the planner.
    OnAdvance => AgentSignal::Advance { .. }
);
stage_gate!(
    /// Admits only [`AgentSignal::Replan`] requests back to the planner.
    OnReplan => AgentSignal::Replan { .. }
);

/// A boxed re-planning function: maps a failure reason to a fresh [`Plan`].
type Replanner = Box<dyn FnMut(&str) -> Plan>;

/// Prefrontal source of actions: walks a [`Plan`], emitting one
/// [`AgentSignal::Act`] per step and stopping once the plan is exhausted. With a
/// replanner installed it can also swap in a fresh plan on
/// [`AgentSignal::Replan`] instead of perseverating on a failing one.
pub struct Planner {
    id: ModuleId,
    plan: Plan,
    replanner: Option<Replanner>,
}

impl Planner {
    pub const fn new(id: ModuleId, plan: Plan) -> Self {
        Self {
            id,
            plan,
            replanner: None,
        }
    }

    /// Install a replanner: on a [`Replan`](AgentSignal::Replan) request it is
    /// called with the failure reason to produce a fresh plan (e.g. an LLM
    /// `propose_plan`). Without one, a replan request halts the loop.
    #[must_use]
    pub fn with_replanner<F>(mut self, replanner: F) -> Self
    where
        F: FnMut(&str) -> Plan + 'static,
    {
        self.replanner = Some(Box::new(replanner));
        self
    }

    fn act_for(&self, step: usize) -> Option<AgentSignal> {
        self.plan.step(step).map(|step_def| AgentSignal::Act {
            step,
            action: step_def.action().to_owned(),
            prediction: step_def.prediction().clone(),
        })
    }

    fn emit_act(&self, step: usize) -> ModuleOutput<AgentSignal> {
        match self.act_for(step) {
            Some(act) => ModuleOutput::emit(act),
            None => ModuleOutput::stop(AgentSignal::Halt {
                reason: format!("plan complete after {step} steps"),
            }),
        }
    }
}

impl fmt::Debug for Planner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Planner")
            .field("id", &self.id)
            .field("plan", &self.plan)
            .field("replanner", &self.replanner.is_some())
            .finish()
    }
}

impl Module<AgentSignal> for Planner {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(
        &mut self,
        signal: Signal<AgentSignal>,
    ) -> Result<ModuleOutput<AgentSignal>, ModuleError> {
        match signal.into_payload() {
            AgentSignal::Goal => Ok(self.emit_act(0)),
            AgentSignal::Advance { next } => Ok(self.emit_act(next)),
            AgentSignal::Replan { reason, .. } => {
                let new_plan = match &mut self.replanner {
                    Some(replan) => replan(&reason),
                    None => {
                        return Ok(ModuleOutput::stop(AgentSignal::Halt {
                            reason: format!("cannot replan: {reason}"),
                        }));
                    }
                };
                self.plan = new_plan;
                // The fresh plan starts from the top.
                Ok(self.emit_act(0))
            }
            _ => Err(ModuleError::new(
                "planner expects a goal, advance, or replan signal",
            )),
        }
    }
}

/// Sensorimotor module: turns an [`AgentSignal::Act`] into an
/// [`AgentSignal::Observe`] by running the supplied effect over the action text.
pub struct RoutedTool<F> {
    id: ModuleId,
    run: F,
}

impl<F> RoutedTool<F> {
    pub const fn new(id: ModuleId, run: F) -> Self {
        Self { id, run }
    }
}

impl<F> fmt::Debug for RoutedTool<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoutedTool")
            .field("id", &self.id)
            .finish()
    }
}

impl<F> Module<AgentSignal> for RoutedTool<F>
where
    F: FnMut(&str) -> Result<String, ModuleError>,
{
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(
        &mut self,
        signal: Signal<AgentSignal>,
    ) -> Result<ModuleOutput<AgentSignal>, ModuleError> {
        match signal.into_payload() {
            AgentSignal::Act {
                step,
                action,
                prediction,
            } => {
                let observed = (self.run)(&action)?;
                Ok(ModuleOutput::emit(AgentSignal::Observe {
                    step,
                    action,
                    prediction,
                    observed,
                }))
            }
            _ => Err(ModuleError::new("tool expects an act signal")),
        }
    }
}

/// Prefrontal executive: verifies each observation against its prediction,
/// records the episode, broadcasts it to the bounded workspace, then releases
/// the next action or halts. Memory and workspace are shared handles so callers
/// can inspect what routing produced after [`Runtime::run`] returns.
/// How many times the executive will re-try the *same* failing step before
/// abandoning it, by default. Bounds perseveration without being so tight that a
/// transiently-flaky step is given up on immediately.
const DEFAULT_MAX_RETRIES: u32 = 2;

#[derive(Debug)]
pub struct Executive {
    id: ModuleId,
    memory: Rc<RefCell<EpisodicStore>>,
    verifier: Verifier,
    modulators: Modulators,
    workspace: Rc<RefCell<Workspace>>,
    errors: Option<Rc<RefCell<OutcomeError>>>,
    max_retries: u32,
    last_retry_step: Option<usize>,
    retry_count: u32,
    /// If set, a mismatch whose magnitude reaches this triggers a replan request
    /// instead of a retry/halt.
    replan_threshold: Option<f32>,
}

impl Executive {
    pub const fn new(
        id: ModuleId,
        memory: Rc<RefCell<EpisodicStore>>,
        verifier: Verifier,
        modulators: Modulators,
        workspace: Rc<RefCell<Workspace>>,
    ) -> Self {
        Self {
            id,
            memory,
            verifier,
            modulators,
            workspace,
            errors: None,
            max_retries: DEFAULT_MAX_RETRIES,
            last_retry_step: None,
            retry_count: 0,
            replan_threshold: None,
        }
    }

    /// Trigger a re-plan when a mismatch's graded magnitude reaches `threshold`
    /// (in `[0.0, 1.0]`), instead of retrying the same step or halting. Pair with
    /// a [`Planner::with_replanner`] to actually swap the plan.
    #[must_use]
    pub const fn with_replan_threshold(mut self, threshold: f32) -> Self {
        self.replan_threshold = Some(threshold);
        self
    }

    /// Attach a shared [`OutcomeError`] meter so a
    /// [`LearningLoop`](crate::LearningLoop) can read this executive's graded
    /// error after a run and reinforce the routes accordingly.
    #[must_use]
    pub fn with_error_meter(mut self, errors: Rc<RefCell<OutcomeError>>) -> Self {
        self.errors = Some(errors);
        self
    }

    /// Bound how many times the same failing step may be retried before it is
    /// abandoned (the perseveration guard). Defaults to 2.
    #[must_use]
    pub const fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    fn reset_retries(&mut self) {
        self.last_retry_step = None;
        self.retry_count = 0;
    }

    /// Retry `step`, or abandon it once the retry budget is spent. Without this
    /// bound an Exploratory executive re-emits the same failing step forever,
    /// perseverating until the run hits the step limit. Returns the loop output.
    fn retry_or_abandon(&mut self, step: usize, reason: String) -> ModuleOutput<AgentSignal> {
        self.retry_count = if self.last_retry_step == Some(step) {
            self.retry_count.saturating_add(1)
        } else {
            1
        };
        self.last_retry_step = Some(step);
        if self.retry_count > self.max_retries {
            self.reset_retries();
            ModuleOutput::stop(AgentSignal::Halt {
                reason: format!(
                    "abandoned step {step} after {} retries: {reason}",
                    self.max_retries
                ),
            })
        } else {
            ModuleOutput::emit(AgentSignal::Advance { next: step })
        }
    }
}

impl Module<AgentSignal> for Executive {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(
        &mut self,
        signal: Signal<AgentSignal>,
    ) -> Result<ModuleOutput<AgentSignal>, ModuleError> {
        let AgentSignal::Observe {
            step,
            action,
            prediction,
            observed,
        } = signal.into_payload()
        else {
            return Err(ModuleError::new("executive expects an observe signal"));
        };

        let correction = self
            .verifier
            .verify(&prediction, &Outcome::new(observed.clone()));
        // Record the graded prediction error (0.0 when the outcome matched) so a
        // learning driver can scale reinforcement by how wrong this run was.
        if let Some(errors) = &self.errors {
            // Precision-weighted: a confident miss teaches more than an unsure one.
            let magnitude = correction
                .mismatch()
                .map_or(0.0, |mismatch| mismatch.precision_weighted_magnitude());
            errors.borrow_mut().record(magnitude);
        }
        // Importance is set from the attention neuromodulator, so a more attentive
        // executive lays down more salient (higher-ranking) memories.
        self.memory.borrow_mut().encode(
            Episode::new(format!("{action} -> {observed}"))
                .with_tags(["observation"])
                .with_importance(self.modulators.attention().get()),
        );
        self.workspace
            .borrow_mut()
            .broadcast(Broadcast::observation(format!(
                "{action} observed {observed}"
            )));
        // Predictive coding: when the outcome contradicts the prediction,
        // propagate the error delta onto the workspace as an alert.
        if let Some(mismatch) = correction.mismatch() {
            self.workspace
                .borrow_mut()
                .broadcast(Broadcast::alert(mismatch.to_string()));
        }

        // Cognitive flexibility: a severe mismatch re-plans rather than retrying
        // the same step or halting — mPFC/ACC detecting a failing plan and
        // re-routing instead of perseverating.
        if let Some(threshold) = self.replan_threshold {
            if let Some(mismatch) = correction.mismatch() {
                if mismatch.precision_weighted_magnitude() >= threshold {
                    self.reset_retries();
                    return Ok(ModuleOutput::emit(AgentSignal::Replan {
                        step,
                        reason: format!("severe mismatch in {}", mismatch.action()),
                    }));
                }
            }
        }

        Ok(match decide(correction, self.modulators.mode()) {
            Decision::Continue => {
                // A step that finally succeeded clears its retry history.
                self.reset_retries();
                ModuleOutput::emit(AgentSignal::Advance {
                    next: step.saturating_add(1),
                })
            }
            Decision::Retry { reason } => self.retry_or_abandon(step, reason),
            Decision::Escalate { reason } | Decision::AskUser { question: reason } => {
                ModuleOutput::stop(AgentSignal::Halt { reason })
            }
        })
    }
}

/// Wire the standard loop topology into `runtime`: goal -> planner -> tool ->
/// executive -> planner. The three modules must already be inserted.
pub fn wire_loop(
    runtime: &mut Runtime<AgentSignal>,
    goal: &InputId,
    planner: &ModuleId,
    tool: &ModuleId,
    executive: &ModuleId,
) -> Result<(), RuntimeError> {
    runtime.add_input_route(goal.clone(), planner.clone(), LOOP_WEIGHT, OnGoal)?;
    runtime.add_module_route(planner.clone(), tool.clone(), LOOP_WEIGHT, OnAct)?;
    runtime.add_module_route(tool.clone(), executive.clone(), LOOP_WEIGHT, OnObserve)?;
    runtime.add_module_route(executive.clone(), planner.clone(), LOOP_WEIGHT, OnAdvance)?;
    // Replan feedback shares the executive -> planner edge; its gate (OnReplan)
    // is disjoint from OnAdvance, so selection stays unambiguous.
    runtime.add_module_route(executive.clone(), planner.clone(), LOOP_WEIGHT, OnReplan)?;
    Ok(())
}
