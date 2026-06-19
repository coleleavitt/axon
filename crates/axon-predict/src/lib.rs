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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Correction {
    Proceed,
    Retry { reason: String },
    Escalate(Mismatch),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    action: String,
    expected: Expected,
    observed: String,
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
