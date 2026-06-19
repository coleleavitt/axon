use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// Full confidence, in permille (so `Prediction` stays `Eq` — no raw `f32`).
const FULL_CONFIDENCE: u16 = 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prediction {
    action: String,
    expected: Expected,
    /// Precision in permille `[0, 1000]`: how confident this prediction is.
    /// Stored as an integer so the type remains `Eq`.
    confidence: u16,
}

impl Prediction {
    pub fn new(action: impl Into<String>, expected: Expected) -> Self {
        Self {
            action: action.into(),
            expected,
            confidence: FULL_CONFIDENCE,
        }
    }

    /// Set the prediction's precision (confidence) in `[0.0, 1.0]`. A
    /// hand-written prediction is fully confident (1.0); a learned forward model
    /// lowers this when it has little evidence. Precision-weights the error the
    /// prediction produces, so being wrong about something you were unsure of
    /// teaches less than being wrong about something you were sure of.
    #[must_use]
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = (confidence.clamp(0.0, 1.0) * 1000.0).round() as u16;
        self
    }

    pub fn action(&self) -> &str {
        &self.action
    }

    pub const fn expected(&self) -> &Expected {
        &self.expected
    }

    /// The prediction's precision (confidence) in `[0.0, 1.0]`.
    pub fn confidence(&self) -> f32 {
        f32::from(self.confidence) / 1000.0
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
    confidence: u16,
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
            confidence: prediction.confidence,
        }
    }

    /// The precision (confidence) of the prediction this mismatch came from, in
    /// `[0.0, 1.0]`.
    pub fn precision(&self) -> f32 {
        f32::from(self.confidence) / 1000.0
    }

    /// The precision-weighted error: [`magnitude`](Self::magnitude) scaled by
    /// [`precision`](Self::precision). This is the quantity a predictive-coding
    /// layer actually learns from — a confident miss is a strong teaching signal;
    /// an unsure miss is discounted (inverse-variance weighting).
    pub fn precision_weighted_magnitude(&self) -> f32 {
        self.magnitude() * self.precision()
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

/// A forward model: it *predicts* what a context will produce and *learns* from
/// the outcome, so predictions stop being hand-written and improve with
/// experience (the cerebellar forward model trained by the error signal).
///
/// Predictors stack: a higher-level one can consume a lower's [`Mismatch`] as
/// its own observation, forming a hierarchy of predictive coding.
pub trait Predictor: fmt::Debug {
    /// Predict what `context` (an action description) will yield.
    fn predict(&self, context: &str) -> Prediction;

    /// Update internal state from an observed `outcome` for `prediction`.
    fn observe(&mut self, prediction: &Prediction, outcome: &Outcome);
}

/// A minimal learned forward model: it remembers the most recent outcome per
/// context and predicts that it recurs. Unknown contexts predict
/// [`Expected::Anything`] (no basis to be wrong yet); once observed, the context
/// predicts [`Expected::Contains`] of what was seen, so a contradiction
/// escalates.
#[derive(Debug, Default, Clone)]
pub struct AssociativePredictor {
    seen: BTreeMap<String, (String, u32)>,
}

impl AssociativePredictor {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many distinct contexts the model has learned.
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

impl Predictor for AssociativePredictor {
    fn predict(&self, context: &str) -> Prediction {
        match self.seen.get(context) {
            Some((observed, count)) => {
                // Confidence grows with evidence: count / (count + 1).
                let count = f32::from(u16::try_from(*count).unwrap_or(u16::MAX));
                Prediction::new(context, Expected::Contains(observed.clone()))
                    .with_confidence(count / (count + 1.0))
            }
            // No evidence yet — predict anything, with zero confidence.
            None => Prediction::new(context, Expected::Anything).with_confidence(0.0),
        }
    }

    fn observe(&mut self, prediction: &Prediction, outcome: &Outcome) {
        let entry = self
            .seen
            .entry(prediction.action().to_owned())
            .or_insert_with(|| (outcome.observed().to_owned(), 0));
        entry.0 = outcome.observed().to_owned();
        entry.1 = entry.1.saturating_add(1);
    }
}

/// A predictor that always predicts a fixed [`Expected`], ignoring context and
/// learning nothing — a constant level in a [`PredictiveHierarchy`] and a simple
/// building block.
#[derive(Debug, Clone)]
pub struct FixedPredictor {
    expected: Expected,
}

impl FixedPredictor {
    pub const fn new(expected: Expected) -> Self {
        Self { expected }
    }
}

impl Predictor for FixedPredictor {
    fn predict(&self, context: &str) -> Prediction {
        Prediction::new(context, self.expected.clone())
    }

    fn observe(&mut self, _prediction: &Prediction, _outcome: &Outcome) {}
}

/// A hierarchy of predictors for recursive predictive coding.
///
/// Level 0 predicts the outcome; if it mismatches, the *rendered error* becomes
/// the context for level 1, which predicts a higher-level cause; and so on. A
/// high-level miss ("won't compile") thus generates a chain of increasingly
/// specific hypotheses ("→ wrong type → rename"), errors flowing up through the
/// stack until a level's hypothesis holds (the explanation) or levels run out.
#[derive(Debug, Default)]
pub struct PredictiveHierarchy {
    levels: Vec<Box<dyn Predictor>>,
}

impl PredictiveHierarchy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a level above the current top of the hierarchy (builder).
    #[must_use]
    pub fn with_level(mut self, predictor: Box<dyn Predictor>) -> Self {
        self.levels.push(predictor);
        self
    }

    pub fn depth(&self) -> usize {
        self.levels.len()
    }

    /// Explain `outcome` for `context` by recursive predictive coding: each
    /// level that mismatches passes its rendered error down as the next level's
    /// context. Returns the chain of mismatches at the levels that failed,
    /// stopping at the first level whose hypothesis holds (the explanation) or
    /// when the levels are exhausted.
    pub fn explain(&self, context: &str, outcome: &Outcome) -> Vec<Mismatch> {
        let verifier = Verifier;
        let mut chain = Vec::new();
        let mut context = context.to_owned();
        for level in &self.levels {
            let prediction = level.predict(&context);
            match verifier.verify(&prediction, outcome) {
                Correction::Escalate(mismatch) => {
                    context = mismatch.to_string();
                    chain.push(mismatch);
                }
                Correction::Proceed | Correction::Retry { .. } => break,
            }
        }
        chain
    }
}

/// An active-inference action selector: it scores candidate actions by expected
/// free energy rather than only past reward.
///
/// Each action's expected value combines a **pragmatic** term (exploit what the
/// forward model is confident will go as expected) and an **epistemic** term
/// (information gain — explore where the model is uncertain, to make it right).
/// Tuning the two weights slides from curiosity to goal-directed exploitation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveInference {
    epistemic: f32,
    pragmatic: f32,
}

impl ActiveInference {
    pub const fn new(epistemic: f32, pragmatic: f32) -> Self {
        Self {
            epistemic,
            pragmatic,
        }
    }

    /// Curiosity-dominant: prefer actions the model is unsure about (to learn).
    pub const fn curious() -> Self {
        Self::new(1.0, 0.3)
    }

    /// Goal-directed: prefer actions the model is confident will succeed.
    pub const fn goal_directed() -> Self {
        Self::new(0.2, 1.0)
    }

    /// Expected value (negative expected free energy) of `action` under the
    /// forward model: pragmatic value from confidence plus epistemic value from
    /// uncertainty (`1 - confidence`).
    pub fn expected_value(&self, predictor: &dyn Predictor, action: &str) -> f32 {
        let confidence = predictor.predict(action).confidence();
        self.pragmatic * confidence + self.epistemic * (1.0 - confidence)
    }

    /// Rank `actions` by expected value, best first.
    pub fn rank<'a>(&self, predictor: &dyn Predictor, actions: &[&'a str]) -> Vec<(&'a str, f32)> {
        let mut scored: Vec<(&'a str, f32)> = actions
            .iter()
            .map(|action| (*action, self.expected_value(predictor, action)))
            .collect();
        scored.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(Ordering::Equal));
        scored
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
