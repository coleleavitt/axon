use crate::edge::EdgeId;
use crate::id::{EndpointId, ModuleId};
use crate::signal::Signal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceStep {
    from: EndpointId,
    to: ModuleId,
}

impl TraceStep {
    pub const fn new(from: EndpointId, to: ModuleId) -> Self {
        Self { from, to }
    }

    pub const fn from(&self) -> &EndpointId {
        &self.from
    }

    pub const fn to(&self) -> &ModuleId {
        &self.to
    }

    /// This step's `(from, to)` edge identity — the key credit assignment uses to
    /// reinforce the routes a trajectory actually traversed.
    pub fn edge(&self) -> EdgeId {
        EdgeId::new(self.from.clone(), self.to.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport<P> {
    status: RunStatus<P>,
    steps: Vec<TraceStep>,
}

impl<P> RunReport<P> {
    pub const fn new(status: RunStatus<P>, steps: Vec<TraceStep>) -> Self {
        Self { status, steps }
    }

    pub const fn status(&self) -> &RunStatus<P> {
        &self.status
    }

    pub fn into_status(self) -> RunStatus<P> {
        self.status
    }

    pub fn steps(&self) -> &[TraceStep] {
        &self.steps
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStatus<P> {
    Stopped(Signal<P>),
    Dropped {
        at: ModuleId,
    },
    NoRoute {
        at: EndpointId,
        signal: Signal<P>,
    },
    /// The run was cancelled by a [`StopToken`](crate::StopToken) at `at`.
    Halted {
        at: EndpointId,
    },
}
