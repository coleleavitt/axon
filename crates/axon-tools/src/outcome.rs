/// The success/failure status of a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    Success,
    Failure,
}

/// A structured tool result: the output plus the success/failure status and the
/// cost of the call.
///
/// Unlike a bare output string, this carries the ground-truth learning signal a
/// coding agent gets for free — *did the tool succeed, and how expensive was it*
/// — so tool outcomes can feed plasticity (via [`graded_error`](Self::graded_error))
/// and budgeting (via [`cost`](Self::cost)) instead of being flattened away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolReport {
    output: String,
    status: ToolStatus,
    cost: u32,
}

impl ToolReport {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            status: ToolStatus::Success,
            cost: 0,
        }
    }

    pub fn failure(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            status: ToolStatus::Failure,
            cost: 0,
        }
    }

    /// Tag the call's cost (tokens, latency units, dollars) for budgeting.
    #[must_use]
    pub fn with_cost(mut self, cost: u32) -> Self {
        self.cost = cost;
        self
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    pub const fn status(&self) -> ToolStatus {
        self.status
    }

    pub const fn cost(&self) -> u32 {
        self.cost
    }

    pub const fn is_success(&self) -> bool {
        matches!(self.status, ToolStatus::Success)
    }

    /// The graded error this outcome contributes to learning: `0.0` on success,
    /// `1.0` on failure — the cheapest teaching signal available without an LLM,
    /// ready to scale reinforcement of the route that produced it.
    pub fn graded_error(&self) -> f32 {
        if self.is_success() { 0.0 } else { 1.0 }
    }
}
