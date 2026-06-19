use axon_memory::ProceduralStore;

use crate::Plan;

/// A library of learned skills: plans that worked, promoted and recalled by goal.
///
/// This wires the otherwise-dormant [`ProceduralStore`] into the agent loop.
/// After a run that succeeded, [`learn`](Self::learn) promotes its plan; on a new
/// goal, [`recall`](Self::recall) returns a runnable plan so the agent reuses a
/// known-good skill instead of re-planning from scratch — procedural-memory
/// consolidation closing one more learning loop.
///
/// It is opt-in: an agent that never constructs a `SkillLibrary` is unchanged.
/// It pays off only when goals recur and a whole plan is worth caching; for
/// one-off goals it is dead weight, so the caller decides what to promote.
#[derive(Debug, Default)]
pub struct SkillLibrary {
    store: ProceduralStore,
}

impl SkillLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Promote a plan that achieved `goal` into the library, replacing any prior
    /// skill for the same goal.
    pub fn learn(&mut self, goal: &str, plan: &Plan) {
        self.store.learn(plan.to_procedure(goal));
    }

    /// Recall the plan learned for an exact `goal`, ready to run.
    pub fn recall(&self, goal: &str) -> Option<Plan> {
        self.store.get(goal).map(Plan::from_procedure)
    }

    /// Recall the best plan for a partial goal `cue` (token overlap), if any.
    pub fn recall_similar(&self, cue: &str) -> Option<Plan> {
        self.store
            .recall(cue)
            .first()
            .map(|procedure| Plan::from_procedure(procedure))
    }

    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}
