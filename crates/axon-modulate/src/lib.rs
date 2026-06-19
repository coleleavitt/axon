use std::error::Error;
use std::fmt;

use axon_core::Priority;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Baseline,
    Focused,
    Exploratory,
    Salient,
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
pub struct RiskTolerance(Gain);

impl RiskTolerance {
    pub fn new(value: f32) -> Result<Self, ModulateError> {
        Gain::new(value).map(Self)
    }

    pub const fn get(self) -> f32 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Modulators {
    mode: Mode,
    learning_rate: LearningRate,
    exploration: Exploration,
    attention: Attention,
    risk: RiskTolerance,
}

impl Modulators {
    pub const fn baseline() -> Self {
        Self {
            mode: Mode::Baseline,
            learning_rate: LearningRate(Gain::trusted(0.10)),
            exploration: Exploration(Gain::trusted(0.20)),
            attention: Attention(Gain::trusted(1.00)),
            risk: RiskTolerance(Gain::trusted(0.30)),
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
        }
    }

    pub const fn with_mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
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
