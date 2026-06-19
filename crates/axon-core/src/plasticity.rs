use crate::route::round_to_i16;

/// The per-edge teaching signal handed to a [`Plasticity`] policy.
///
/// It bundles how wrong the trajectory was (`error`), how much this particular
/// edge contributed to it (`eligibility`), and the global plasticity gain
/// (`learning_rate`, typically a neuromodulator scalar).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Credit {
    /// Graded prediction error in `[0.0, 1.0]`; `0.0` is a perfect outcome.
    pub error: f32,
    /// How eligible this edge is for credit — recent or repeated edges score
    /// higher (see the eligibility trace built during reinforcement).
    pub eligibility: f32,
    /// Global plasticity gain, e.g. `Modulators::learning_rate()`.
    pub learning_rate: f32,
}

/// A plasticity policy: maps a [`Credit`] signal onto an integer weight change.
/// A positive delta strengthens an edge, a negative one weakens it.
///
/// The trait is kept in pure scalar terms so the routing core stays free of any
/// prediction or neuromodulation dependency. Callers compute `error` from
/// `axon_predict::Mismatch::magnitude()` and pass it in.
pub trait Plasticity {
    fn delta(&self, credit: Credit) -> i16;
}

/// The default reward-modulated rule — the discrete analogue of dopamine-gated
/// TD(λ) reinforcement.
///
/// A graded error is mapped to a signed teaching signal `reward = 1 - 2·error`
/// (a perfect outcome strengthens, the midpoint is neutral, a total miss
/// weakens), then scaled by eligibility, learning rate, and `gain` — the maximum
/// single-edge step at full strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProportionalPlasticity {
    gain: i16,
}

impl ProportionalPlasticity {
    pub const fn new(gain: i16) -> Self {
        Self { gain }
    }

    pub const fn gain(self) -> i16 {
        self.gain
    }
}

impl Default for ProportionalPlasticity {
    fn default() -> Self {
        Self { gain: 10 }
    }
}

impl Plasticity for ProportionalPlasticity {
    fn delta(&self, credit: Credit) -> i16 {
        let reward = 1.0 - 2.0 * credit.error;
        round_to_i16(f32::from(self.gain) * credit.learning_rate * credit.eligibility * reward)
    }
}

/// Parameters for one credit-assignment pass over a finished trajectory.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reinforcement {
    /// Graded outcome error in `[0.0, 1.0]`, shared by every edge in the run.
    pub error: f32,
    /// Global plasticity gain (e.g. `Modulators::learning_rate()`).
    pub learning_rate: f32,
    /// Eligibility decay `λ` in `[0.0, 1.0]`: how fast credit fades into the
    /// past, so edges nearer the outcome are reinforced more than distant ones.
    pub decay: f32,
}

impl Reinforcement {
    pub const fn new(error: f32, learning_rate: f32, decay: f32) -> Self {
        Self {
            error,
            learning_rate,
            decay,
        }
    }
}
