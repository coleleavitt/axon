use std::error::Error;
use std::fmt;

use axon_memory::{Episode, EpisodicStore, MemoryStore};
use axon_modulate::{Mode, Modulators};
use axon_predict::{Correction, Outcome, Prediction, Verifier};
use axon_workspace::{Broadcast, Workspace};

mod agent;
mod learning;
mod planning;

pub use agent::{
    AgentSignal,
    Executive,
    OnAct,
    OnAdvance,
    OnGoal,
    OnObserve,
    Planner,
    RoutedTool,
    wire_loop,
};
pub use learning::{LearningLoop, OutcomeError};
pub use planning::{plan_prompt, propose_plan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    action: String,
    prediction: Prediction,
}

impl Step {
    pub fn new(action: impl Into<String>, prediction: Prediction) -> Self {
        Self {
            action: action.into(),
            prediction,
        }
    }

    pub fn action(&self) -> &str {
        &self.action
    }

    pub const fn prediction(&self) -> &Prediction {
        &self.prediction
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    steps: Vec<Step>,
}

impl Plan {
    pub fn new<I>(steps: I) -> Self
    where
        I: IntoIterator<Item = Step>,
    {
        Self {
            steps: steps.into_iter().collect(),
        }
    }

    pub fn step(&self, index: usize) -> Option<&Step> {
        self.steps.get(index)
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Continue,
    Retry { reason: String },
    Escalate { reason: String },
    AskUser { question: String },
}

#[derive(Debug)]
pub struct Executor {
    memory: EpisodicStore,
    verifier: Verifier,
    modulators: Modulators,
    workspace: Workspace,
}

impl Executor {
    pub const fn new(
        memory: EpisodicStore,
        verifier: Verifier,
        modulators: Modulators,
        workspace: Workspace,
    ) -> Self {
        Self {
            memory,
            verifier,
            modulators,
            workspace,
        }
    }

    pub fn observe_step(
        &mut self,
        plan: &Plan,
        index: usize,
        outcome: Outcome,
    ) -> Result<Decision, ExecError> {
        let step = plan.step(index).ok_or(ExecError::MissingStep { index })?;
        let correction = self.verifier.verify(step.prediction(), &outcome);
        self.memory.encode(Episode::new(format!(
            "{} -> {}",
            step.action(),
            outcome.observed()
        )));
        self.workspace.broadcast(Broadcast::observation(format!(
            "{} observed {}",
            step.action(),
            outcome.observed()
        )));
        Ok(self.decide(correction))
    }

    pub const fn memory(&self) -> &EpisodicStore {
        &self.memory
    }

    pub const fn memory_mut(&mut self) -> &mut EpisodicStore {
        &mut self.memory
    }

    pub const fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    fn decide(&self, correction: Correction) -> Decision {
        decide(correction, self.modulators.mode())
    }
}

/// Map a verifier [`Correction`] onto an executive [`Decision`], modulated by the
/// active [`Mode`]. Shared by the directly-composed [`Executor`] and the routed
/// [`Executive`] module so both reach identical policy conclusions.
pub(crate) fn decide(correction: Correction, mode: Mode) -> Decision {
    match correction {
        Correction::Proceed => Decision::Continue,
        Correction::Retry { reason } => Decision::Retry { reason },
        Correction::Escalate(mismatch) => match mode {
            Mode::Exploratory => Decision::Retry {
                reason: format!("retry after mismatch in {}", mismatch.action()),
            },
            Mode::Baseline | Mode::Focused | Mode::Salient => Decision::Escalate {
                reason: format!("prediction mismatch in {}", mismatch.action()),
            },
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecError {
    MissingStep { index: usize },
}

impl fmt::Display for ExecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingStep { index } => write!(formatter, "plan has no step at index {index}"),
        }
    }
}

impl Error for ExecError {}
