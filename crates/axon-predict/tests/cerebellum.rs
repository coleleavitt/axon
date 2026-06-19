use std::error::Error;

use axon_predict::{
    ActiveInference,
    AssociativePredictor,
    Correction,
    Expected,
    FixedPredictor,
    Outcome,
    Prediction,
    PredictiveHierarchy,
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
fn error_is_precision_weighted_by_prediction_confidence() -> Result<(), Box<dyn Error>> {
    // Given: the same total miss made with full vs. half confidence.
    let confident = Prediction::new("inspect", Expected::Contains("ok".to_owned()));
    let unsure =
        Prediction::new("inspect", Expected::Contains("ok".to_owned())).with_confidence(0.5);
    let observed = Outcome::new("nope");

    // When: each is verified into a mismatch.
    let Some(confident) = Verifier.verify(&confident, &observed).mismatch().cloned() else {
        panic!("expected a mismatch");
    };
    let Some(unsure) = Verifier.verify(&unsure, &observed).mismatch().cloned() else {
        panic!("expected a mismatch");
    };

    // Then: the raw magnitude is identical, but the precision-weighted error
    // (what learning uses) is halved for the unsure prediction.
    assert!((confident.magnitude() - unsure.magnitude()).abs() < f32::EPSILON);
    assert!((confident.precision_weighted_magnitude() - 1.0).abs() < f32::EPSILON);
    assert!((unsure.precision_weighted_magnitude() - 0.5).abs() < f32::EPSILON);
    Ok(())
}

#[test]
fn forward_model_confidence_grows_with_evidence() -> Result<(), Box<dyn Error>> {
    // Given: an untrained forward model.
    let mut model = AssociativePredictor::new();

    // An unseen context predicts Anything with zero confidence.
    assert!(model.predict("build").confidence() < f32::EPSILON);

    // After one observation, confidence is 0.5; after a second, it rises.
    model.observe(&model.predict("build"), &Outcome::new("ok"));
    let once = model.predict("build").confidence();
    model.observe(&model.predict("build"), &Outcome::new("ok"));
    let twice = model.predict("build").confidence();

    assert!((once - 0.5).abs() < 0.01);
    assert!(twice > once);
    Ok(())
}

#[test]
fn predictive_hierarchy_explains_via_recursive_error_flow() -> Result<(), Box<dyn Error>> {
    // Given: a three-level hierarchy of increasingly specific hypotheses.
    let hierarchy = PredictiveHierarchy::new()
        .with_level(Box::new(FixedPredictor::new(Expected::Contains(
            "compiles".to_owned(),
        ))))
        .with_level(Box::new(FixedPredictor::new(Expected::Contains(
            "syntax".to_owned(),
        ))))
        .with_level(Box::new(FixedPredictor::new(Expected::Contains(
            "type".to_owned(),
        ))));
    assert_eq!(hierarchy.depth(), 3);

    // When: a type error is observed. Level 0 ("compiles") fails, its error flows
    // up to level 1 ("syntax") which also fails, and level 2 ("type") holds.
    let chain = hierarchy.explain("build", &Outcome::new("error: type mismatch on x"));

    // Then: the failure chain is two deep, and each level's rendered error fed the
    // next as context.
    assert_eq!(chain.len(), 2);
    assert!(chain[0].to_string().contains("compiles"));
    assert!(chain[1].to_string().contains("syntax"));

    // And: when the top-level prediction holds immediately, nothing escalates.
    let explained = hierarchy.explain("build", &Outcome::new("it compiles fine"));
    assert!(explained.is_empty());
    Ok(())
}

#[test]
fn active_inference_trades_curiosity_against_exploitation() -> Result<(), Box<dyn Error>> {
    // Given: a forward model confident about "known" and unsure about "novel".
    let mut model = AssociativePredictor::new();
    model.observe(&model.predict("known"), &Outcome::new("ok"));
    model.observe(&model.predict("known"), &Outcome::new("ok"));
    let actions = ["known", "novel"];

    // When/Then: curiosity ranks the uncertain action first (to learn)...
    let curious = ActiveInference::curious().rank(&model, &actions);
    assert_eq!(curious[0].0, "novel");

    // ...while goal-directed inference ranks the confident action first (exploit).
    let exploit = ActiveInference::goal_directed().rank(&model, &actions);
    assert_eq!(exploit[0].0, "known");
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
