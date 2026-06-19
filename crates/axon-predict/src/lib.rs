use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prediction {
    action: String,
    expected: Expected,
}

impl Prediction {
    pub fn new(action: impl Into<String>, expected: Expected) -> Self {
        Self {
            action: action.into(),
            expected,
        }
    }

    pub fn action(&self) -> &str {
        &self.action
    }

    pub const fn expected(&self) -> &Expected {
        &self.expected
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expected {
    Contains(String),
    Equals(String),
    Anything,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    observed: String,
}

impl Outcome {
    pub fn new(observed: impl Into<String>) -> Self {
        Self {
            observed: observed.into(),
        }
    }

    pub fn observed(&self) -> &str {
        &self.observed
    }
}

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Correction {
    Proceed,
    Retry { reason: String },
    Escalate(Mismatch),
}

impl Correction {
    /// The propagable prediction error, when the outcome contradicted the
    /// prediction. This is the "diff" a predictive layer sends upward.
    pub const fn mismatch(&self) -> Option<&Mismatch> {
        match self {
            Self::Escalate(mismatch) => Some(mismatch),
            Self::Proceed | Self::Retry { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    action: String,
    expected: Expected,
    observed: String,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: expected {:?}, observed {:?}",
            self.action, self.expected, self.observed
        )
    }
}

impl Mismatch {
    fn new(prediction: &Prediction, outcome: &Outcome) -> Self {
        Self {
            action: prediction.action.clone(),
            expected: prediction.expected.clone(),
            observed: outcome.observed.clone(),
        }
    }

    pub fn action(&self) -> &str {
        &self.action
    }

    pub const fn expected(&self) -> &Expected {
        &self.expected
    }

    pub fn observed(&self) -> &str {
        &self.observed
    }

    /// The graded prediction error in `[0.0, 1.0]`: *how wrong* the outcome
    /// was, not merely *that* it was wrong. `0.0` means the observation fully
    /// covered the expectation; `1.0` means no overlap at all. Computed as the
    /// token (Jaccard) distance between the expected text and the observation.
    ///
    /// This is the scalar a predictive layer propagates upward and that
    /// plasticity scales learning by — the prerequisite for graded
    /// reinforcement (a categorical "wrong" cannot be scaled).
    pub fn magnitude(&self) -> f32 {
        let expected = match &self.expected {
            Expected::Contains(text) | Expected::Equals(text) => text.as_str(),
            // An `Anything` expectation can never be contradicted, so a mismatch
            // built from one carries no error.
            Expected::Anything => return 0.0,
        };
        token_distance(expected, &self.observed)
    }
}

/// Split text into lowercased alphanumeric tokens — the unit graded error is
/// measured over, so wording, case, and punctuation differences don't dominate.
fn tokenize(text: &str) -> BTreeSet<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Jaccard distance `1 - |A ∩ B| / |A ∪ B|` over token sets, in `[0.0, 1.0]`.
/// Two empty strings are treated as identical (distance `0.0`).
fn token_distance(expected: &str, observed: &str) -> f32 {
    let expected = tokenize(expected);
    let observed = tokenize(observed);
    let union = expected.union(&observed).count();
    if union == 0 {
        return 0.0;
    }
    let intersection = expected.intersection(&observed).count();
    1.0 - (intersection as f32 / union as f32)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Verifier;

impl Verifier {
    pub fn verify(&self, prediction: &Prediction, outcome: &Outcome) -> Correction {
        match prediction.expected() {
            Expected::Contains(needle) if outcome.observed().contains(needle) => {
                Correction::Proceed
            }
            Expected::Contains(_) => Correction::Escalate(Mismatch::new(prediction, outcome)),
            Expected::Equals(expected) if outcome.observed() == expected => Correction::Proceed,
            Expected::Equals(_) => Correction::Escalate(Mismatch::new(prediction, outcome)),
            Expected::Anything => Correction::Proceed,
        }
    }
}
