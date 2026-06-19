//! Closing the learning loop around the routed agent: the [`Executive`] records
//! each observation's graded prediction error, and [`LearningLoop`] feeds a
//! run's aggregate error back through credit assignment so the routes that
//! produced good outcomes strengthen and the ones that produced bad outcomes
//! decay. This is what turns the static routing substrate into one that improves
//! with experience.
//!
//! [`Executive`]: crate::Executive

use std::cell::RefCell;
use std::rc::Rc;

use axon_core::{
    InputId,
    Plasticity,
    Reinforcement,
    RunEvent,
    RunReport,
    Runtime,
    RuntimeError,
    Signal,
};

use crate::AgentSignal;

/// A shared accumulator for a run's graded prediction error.
///
/// The [`Executive`](crate::Executive) records each observation's
/// [`Mismatch::magnitude`] here (`0.0` for a clean outcome); a learning driver
/// reads the aggregate after the run to scale reinforcement, then resets it.
///
/// [`Mismatch::magnitude`]: axon_predict::Mismatch::magnitude
#[derive(Debug, Default, Clone)]
pub struct OutcomeError {
    total: f32,
    count: u32,
}

impl OutcomeError {
    pub(crate) fn record(&mut self, magnitude: f32) {
        self.total += magnitude;
        self.count = self.count.saturating_add(1);
    }

    /// The mean graded error across recorded observations, or `0.0` if none.
    pub fn mean(&self) -> f32 {
        if self.count == 0 {
            0.0
        } else {
            self.total / self.count as f32
        }
    }

    /// How many observations were recorded this run.
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// Clear the accumulator for the next run.
    pub fn reset(&mut self) {
        self.total = 0.0;
        self.count = 0;
    }
}

/// Drives the routed agent loop and closes the learning loop.
///
/// Each [`run_and_learn`](Self::run_and_learn) runs the plan→act→observe cycle,
/// then reinforces the traversed routes by the run's mean graded error: a clean
/// outcome (error `0.0`) strengthens the path taken, a failure weakens it. Given
/// competing routes, the agent learns which one works.
pub struct LearningLoop {
    runtime: Runtime<AgentSignal>,
    errors: Rc<RefCell<OutcomeError>>,
    plasticity: Box<dyn Plasticity>,
    learning_rate: f32,
    decay: f32,
    last_error: f32,
}

impl LearningLoop {
    /// Build a driver around an already-wired `runtime`. `errors` must be the
    /// same meter handed to the [`Executive`](crate::Executive) via
    /// [`with_error_meter`](crate::Executive::with_error_meter). `learning_rate`
    /// is typically `Modulators::learning_rate()`; `decay` is the eligibility
    /// trace `λ`.
    pub fn new(
        runtime: Runtime<AgentSignal>,
        errors: Rc<RefCell<OutcomeError>>,
        plasticity: Box<dyn Plasticity>,
        learning_rate: f32,
        decay: f32,
    ) -> Self {
        Self {
            runtime,
            errors,
            plasticity,
            learning_rate,
            decay,
            last_error: 0.0,
        }
    }

    /// Run once from `goal`, then reinforce the traversed routes by the run's
    /// mean graded error. Reinforcement is applied to the routing table in place;
    /// the run report is returned for inspection.
    pub fn run_and_learn(
        &mut self,
        goal: &InputId,
        signal: AgentSignal,
    ) -> Result<RunReport<AgentSignal>, RuntimeError> {
        self.errors.borrow_mut().reset();
        let report = self.runtime.run(goal.clone(), Signal::new(signal))?;
        let error = self.errors.borrow().mean();
        self.last_error = error;
        self.runtime.reinforce(
            self.plasticity.as_ref(),
            report.steps(),
            Reinforcement::new(error, self.learning_rate, self.decay),
            &mut |_event: &RunEvent| {},
        );
        Ok(report)
    }

    /// The mean graded error of the most recent [`run_and_learn`](Self::run_and_learn).
    pub const fn last_error(&self) -> f32 {
        self.last_error
    }

    pub const fn runtime(&self) -> &Runtime<AgentSignal> {
        &self.runtime
    }

    pub const fn runtime_mut(&mut self) -> &mut Runtime<AgentSignal> {
        &mut self.runtime
    }

    pub fn into_runtime(self) -> Runtime<AgentSignal> {
        self.runtime
    }
}
