use crate::{Plan, Step};

/// One active plan frame: a [`Plan`] plus a cursor into it.
#[derive(Debug, Clone)]
struct Frame {
    plan: Plan,
    cursor: usize,
}

/// A stack of plan frames for hierarchical, *lazily expanded* execution.
///
/// A long-horizon plan decomposes into sub-goals. Rather than flattening them up
/// front (see [`Plan::compose`](crate::Plan::compose)), a sub-goal is expanded
/// into a child frame only when reached and only if its precondition holds
/// ([`expand_if`](Self::expand_if)) — the corticostriatal release gate that
/// admits a child frame when the parent precondition fires. The child runs to
/// completion, then control returns to the parent frame.
#[derive(Debug, Clone)]
pub struct PlanStack {
    frames: Vec<Frame>,
}

impl PlanStack {
    pub fn new(root: Plan) -> Self {
        Self {
            frames: vec![Frame {
                plan: root,
                cursor: 0,
            }],
        }
    }

    /// The current step — the cursor of the deepest unfinished frame — popping
    /// any exhausted child frames back to their parent first. `None` once the
    /// root frame is exhausted.
    pub fn current(&mut self) -> Option<&Step> {
        while self.frames.len() > 1
            && self
                .frames
                .last()
                .is_some_and(|frame| frame.cursor >= frame.plan.len())
        {
            self.frames.pop();
        }
        let frame = self.frames.last()?;
        frame.plan.step(frame.cursor)
    }

    /// Advance past the current step of the deepest frame.
    pub fn advance(&mut self) {
        if let Some(frame) = self.frames.last_mut() {
            frame.cursor = frame.cursor.saturating_add(1);
        }
    }

    /// Lazily expand a sub-goal as a child frame that runs before the parent
    /// continues — but only if `precondition` holds (the release gate). Returns
    /// whether the sub-goal was expanded.
    pub fn expand_if(&mut self, subgoal: Plan, precondition: bool) -> bool {
        if precondition {
            self.frames.push(Frame {
                plan: subgoal,
                cursor: 0,
            });
            true
        } else {
            false
        }
    }

    /// Expand a sub-goal unconditionally.
    pub fn expand(&mut self, subgoal: Plan) {
        let _ = self.expand_if(subgoal, true);
    }

    /// How many frames are active (the root counts as 1).
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Whether every frame is exhausted.
    pub fn is_done(&mut self) -> bool {
        self.current().is_none()
    }
}
