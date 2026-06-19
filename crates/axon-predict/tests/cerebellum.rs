use std::error::Error;

use axon_predict::{Correction, Expected, Outcome, Prediction, Verifier};

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
