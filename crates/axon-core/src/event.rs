use crate::edge::EdgeId;
use crate::id::{EndpointId, ModuleId};

/// A single observable transition emitted while a runtime drives a signal. An
/// observer receives these in order, turning an otherwise opaque run into a
/// stream a caller can log, trace, or forward to a UI.
///
/// Topological variants ([`Entered`](Self::Entered)..[`Dropped`](Self::Dropped))
/// describe routing; [`Reinforced`](Self::Reinforced) makes the *learning* loop
/// observable — without it, what an agent learned and why would be invisible and
/// therefore untestable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEvent {
    /// A signal was routed from `from` into module `to`, about to be handled.
    Entered { from: EndpointId, to: ModuleId },
    /// Module `at` emitted a signal to continue routing.
    Emitted { at: ModuleId },
    /// Module `at` stopped the run with a final signal.
    Stopped { at: ModuleId },
    /// Module `at` dropped the signal, ending the run.
    Dropped { at: ModuleId },
    /// Credit assignment changed `edge`'s learned weight by `delta` (positive
    /// strengthens, negative weakens).
    Reinforced { edge: EdgeId, delta: i16 },
    /// The run was cancelled at `at` by a [`StopToken`](crate::StopToken).
    Halted { at: EndpointId },
}
