use crate::event::RunEvent;
use crate::plasticity::{Plasticity, Reinforcement};
use crate::report::TraceStep;
use crate::rng::Rng;
use crate::runtime::Runtime;

/// A buffer of past run trajectories for offline sharp-wave-ripple replay.
///
/// During a run the agent records *what path it took* and *how the outcome
/// graded*. Later — between tasks, or in an idle "sleep" pass — those stored
/// trajectories are sampled and credit-assigned again, consolidating the learned
/// routing **without re-running any module or tool**. This is hippocampal replay
/// training the cortical model (CLS): the agent keeps learning from memory.
#[derive(Debug, Clone)]
pub struct ReplayBuffer {
    episodes: Vec<(Vec<TraceStep>, f32)>,
    capacity: usize,
}

impl ReplayBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            episodes: Vec::new(),
            capacity: capacity.max(1),
        }
    }

    /// Record a finished run's trajectory and its graded outcome error for later
    /// replay. Empty trajectories are ignored; the oldest entry is dropped once
    /// the buffer is full.
    pub fn record(&mut self, steps: &[TraceStep], error: f32) {
        if steps.is_empty() {
            return;
        }
        if self.episodes.len() >= self.capacity {
            self.episodes.remove(0);
        }
        self.episodes.push((steps.to_vec(), error));
    }

    pub fn len(&self) -> usize {
        self.episodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.episodes.is_empty()
    }

    /// Offline consolidation: sample `rounds` stored trajectories with `rng` and
    /// re-apply credit assignment to `runtime`, reinforcing learned routing from
    /// memory alone — no module or tool is run. Reinforcement events stream to
    /// `observer`. A no-op while the buffer is empty.
    pub fn replay<P>(
        &self,
        runtime: &mut Runtime<P>,
        plasticity: &dyn Plasticity,
        learning_rate: f32,
        decay: f32,
        rounds: usize,
        rng: &mut Rng,
        observer: &mut dyn FnMut(&RunEvent),
    ) {
        if self.episodes.is_empty() {
            return;
        }
        for _ in 0..rounds {
            let index = (rng.next_u64() as usize) % self.episodes.len();
            let (steps, error) = &self.episodes[index];
            runtime.reinforce(
                plasticity,
                steps,
                Reinforcement::new(*error, learning_rate, decay),
                observer,
            );
        }
    }
}
