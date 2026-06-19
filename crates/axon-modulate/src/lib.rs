use std::error::Error;
use std::fmt;

use axon_core::Priority;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Mode {
    Baseline,
    Focused,
    Exploratory,
    Salient,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Gain(f32);

impl Gain {
    pub fn new(value: f32) -> Result<Self, ModulateError> {
        if value.is_finite() && value >= 0.0 {
            Ok(Self(value))
        } else {
            Err(ModulateError::InvalidKnob { value })
        }
    }

    const fn trusted(value: f32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LearningRate(Gain);

impl LearningRate {
    pub fn new(value: f32) -> Result<Self, ModulateError> {
        Gain::new(value).map(Self)
    }

    pub const fn get(self) -> f32 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Exploration(Gain);

impl Exploration {
    pub fn new(value: f32) -> Result<Self, ModulateError> {
        Gain::new(value).map(Self)
    }

    pub const fn get(self) -> f32 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Attention(Gain);

impl Attention {
    pub fn new(value: f32) -> Result<Self, ModulateError> {
        Gain::new(value).map(Self)
    }

    pub const fn get(self) -> f32 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RiskTolerance(Gain);

impl RiskTolerance {
    pub fn new(value: f32) -> Result<Self, ModulateError> {
        Gain::new(value).map(Self)
    }

    pub const fn get(self) -> f32 {
        self.0.get()
    }
}

/// Acetylcholine mode — ACh biases memory between *laying down* new memories and
/// *retrieving* old ones. High ACh ([`Encode`](Self::Encode)) favors encoding and
/// suppresses retrieval interference; low ACh ([`Recall`](Self::Recall)) favors
/// broad recall and consolidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Acetylcholine {
    #[default]
    Encode,
    Recall,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Modulators {
    mode: Mode,
    /// Phasic dopamine: the RPE learning signal that scales plasticity.
    learning_rate: LearningRate,
    exploration: Exploration,
    attention: Attention,
    risk: RiskTolerance,
    /// Tonic dopamine: baseline vigor / willingness to act, distinct from the
    /// phasic `learning_rate`. Higher means more readily Go (a lower NoGo margin).
    tonic: Gain,
    /// Acetylcholine: encode-vs-recall bias for memory.
    acetylcholine: Acetylcholine,
}

impl Modulators {
    pub const fn baseline() -> Self {
        Self {
            mode: Mode::Baseline,
            learning_rate: LearningRate(Gain::trusted(0.10)),
            exploration: Exploration(Gain::trusted(0.20)),
            attention: Attention(Gain::trusted(1.00)),
            risk: RiskTolerance(Gain::trusted(0.30)),
            tonic: Gain::trusted(0.50),
            acetylcholine: Acetylcholine::Encode,
        }
    }

    pub const fn new(
        mode: Mode,
        learning_rate: LearningRate,
        exploration: Exploration,
        attention: Attention,
        risk: RiskTolerance,
    ) -> Self {
        Self {
            mode,
            learning_rate,
            exploration,
            attention,
            risk,
            tonic: Gain::trusted(0.50),
            acetylcholine: Acetylcholine::Encode,
        }
    }

    pub const fn with_mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the acetylcholine encode/recall bias.
    #[must_use]
    pub const fn with_acetylcholine(mut self, acetylcholine: Acetylcholine) -> Self {
        self.acetylcholine = acetylcholine;
        self
    }

    /// Set the tonic-dopamine level (baseline vigor).
    pub fn with_tonic_dopamine(mut self, tonic: f32) -> Result<Self, ModulateError> {
        self.tonic = Gain::new(tonic)?;
        Ok(self)
    }

    /// The acetylcholine mode (encode vs recall bias).
    pub const fn acetylcholine(self) -> Acetylcholine {
        self.acetylcholine
    }

    /// Tonic dopamine — baseline vigor, distinct from the phasic `learning_rate`.
    pub const fn tonic_dopamine(self) -> Gain {
        self.tonic
    }

    pub const fn mode(self) -> Mode {
        self.mode
    }

    pub const fn exploration(self) -> Exploration {
        self.exploration
    }

    pub const fn learning_rate(self) -> LearningRate {
        self.learning_rate
    }

    pub const fn attention(self) -> Attention {
        self.attention
    }

    pub const fn risk_tolerance(self) -> RiskTolerance {
        self.risk
    }

    pub fn apply_attention(self, priority: Priority) -> Priority {
        let bump = match self.mode {
            Mode::Baseline => 0,
            Mode::Focused => 4,
            Mode::Exploratory => 1,
            Mode::Salient => 10,
        };
        Priority::new(priority.get().saturating_add(bump))
    }

    pub const fn verification_gain(self) -> Gain {
        match self.mode {
            Mode::Baseline => Gain::trusted(1.0),
            Mode::Focused => Gain::trusted(1.5),
            Mode::Exploratory => Gain::trusted(0.8),
            Mode::Salient => Gain::trusted(2.0),
        }
    }
}

impl Default for Modulators {
    fn default() -> Self {
        Self::baseline()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModulateError {
    InvalidKnob { value: f32 },
}

impl fmt::Display for ModulateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKnob { value } => {
                write!(formatter, "invalid neuromodulator knob: {value}")
            }
        }
    }
}

impl Error for ModulateError {}
