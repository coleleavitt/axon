use std::error::Error;

use axon_predict::{
    AssociativePredictor,
    Correction,
    Expected,
    Outcome,
    Prediction,
    Predictor,
    Verifier,
};

#[test]
fn verifier_accepts_matching_prediction_and_outcome() -> Result<(), Box<dyn Error>> {
    // Given: a prediction for a successful tool outcome.
    let prediction = Prediction::new("read manifest", Expected::Contains("axon".to_owned()));
    let outcome = Outcome::new("Cargo package axon");

    // When: the verifier compares prediction and outcome.
    let correction = Verifier.verify(&prediction, &outcome);

    // Then: matching evidence proceeds without escalation.
    assert_eq!(correction, Correction::Proceed);
    Ok(())
}

#[test]
fn verifier_escalates_missing_expected_evidence() -> Result<(), Box<dyn Error>> {
    // Given: a prediction whose expected text is absent.
    let prediction = Prediction::new(
        "read manifest",
        Expected::Contains("hippocampus".to_owned()),
    );
    let outcome = Outcome::new("Cargo package axon");

    // When: the verifier compares prediction and outcome.
    let correction = Verifier.verify(&prediction, &outcome);

    // Then: the mismatch carries the prediction/action context.
    match correction {
        Correction::Escalate(mismatch) => assert_eq!(mismatch.action(), "read manifest"),
        Correction::Proceed | Correction::Retry { reason: _ } => panic!("expected escalation"),
    }
    Ok(())
}

#[test]
fn correction_exposes_propagable_prediction_error() -> Result<(), Box<dyn Error>> {
    // Given: a contradicted prediction.
    let prediction = Prediction::new("read manifest", Expected::Contains("absent".to_owned()));
    let correction = Verifier.verify(&prediction, &Outcome::new("Cargo package axon"));

    // When: the prediction error delta is requested for upward propagation.
    let Some(mismatch) = correction.mismatch() else {
        panic!("expected a propagable mismatch");
    };

    // Then: it renders the action plus the expected/observed diff.
    let rendered = mismatch.to_string();
    assert!(rendered.contains("read manifest"));
    assert!(rendered.contains("absent"));
    assert!(rendered.contains("Cargo package axon"));

    // And: a proceeding correction carries no error.
    let clean = Verifier.verify(
        &Prediction::new("noop", Expected::Anything),
        &Outcome::new("ok"),
    );
    assert!(clean.mismatch().is_none());
    Ok(())
}

#[test]
fn forward_model_learns_to_predict_observed_outcomes() -> Result<(), Box<dyn Error>> {
    // Given: an untrained forward model.
    let mut model = AssociativePredictor::new();
    assert!(model.is_empty());

    // When/Then: an unseen context predicts Anything, so it cannot yet be wrong.
    let blank = model.predict("read manifest");
    assert_eq!(blank.expected(), &Expected::Anything);
    assert_eq!(
        Verifier.verify(&blank, &Outcome::new("name = axon")),
        Correction::Proceed
    );

    // When: it observes an outcome for that context.
    model.observe(&blank, &Outcome::new("name = axon"));
    assert_eq!(model.len(), 1);

    // Then: it now predicts the learned outcome — the same result verifies, a
    // contradicting one escalates with a real error.
    let learned = model.predict("read manifest");
    assert_eq!(
        Verifier.verify(&learned, &Outcome::new("name = axon")),
        Correction::Proceed
    );
    assert!(
        Verifier
            .verify(&learned, &Outcome::new("totally different"))
            .mismatch()
            .is_some()
    );
    Ok(())
}

#[test]
fn mismatch_magnitude_is_graded_not_categorical() -> Result<(), Box<dyn Error>> {
    // Given: a total miss (no shared words) and a partial miss (some shared).
    let total = Verifier.verify(
        &Prediction::new("inspect", Expected::Contains("absent".to_owned())),
        &Outcome::new("Cargo package axon"),
    );
    let partial = Verifier.verify(
        &Prediction::new("inspect", Expected::Equals("cargo package axon".to_owned())),
        &Outcome::new("cargo package missing"),
    );

    // When: each mismatch's graded error magnitude is measured.
    let Some(total) = total.mismatch() else {
        panic!("expected a total mismatch");
    };
    let Some(partial) = partial.mismatch() else {
        panic!("expected a partial mismatch");
    };

    // Then: a total miss saturates at 1.0, a partial miss lands strictly inside
    // (0, 1), and "more wrong" scores higher than "less wrong" — the scalar
    // plasticity scales learning by, not a yes/no flag.
    assert!((total.magnitude() - 1.0).abs() < f32::EPSILON);
    assert!((partial.magnitude() - 0.5).abs() < f32::EPSILON);
    assert!(partial.magnitude() < total.magnitude());
    Ok(())
}
