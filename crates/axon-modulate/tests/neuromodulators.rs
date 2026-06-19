use std::error::Error;

use axon_core::Priority;
use axon_modulate::{
    Acetylcholine,
    Attention,
    Exploration,
    LearningRate,
    Mode,
    Modulators,
    RiskTolerance,
};

#[test]
fn salient_mode_raises_priority_and_verification_gain() -> Result<(), Box<dyn Error>> {
    // Given: a baseline neuromodulator state.
    let modulators = Modulators::baseline().with_mode(Mode::Salient);

    // When: priority and verification gain are derived.
    let priority = modulators.apply_attention(Priority::new(1));

    // Then: salient mode raises priority and requests stronger verification.
    assert!(priority > Priority::new(1));
    assert!(modulators.verification_gain().get() > 1.0);
    Ok(())
}

#[test]
fn explicit_knobs_are_typed_and_do_not_rewire_routes() -> Result<(), Box<dyn Error>> {
    // Given: explicit neuromodulator knobs.
    let modulators = Modulators::new(
        Mode::Focused,
        LearningRate::new(0.25)?,
        Exploration::new(0.10)?,
        Attention::new(1.50)?,
        RiskTolerance::new(0.20)?,
    );

    // When: consumers inspect the values.
    let exploration = modulators.exploration();

    // Then: the state is data-only and can be consumed by gates/modules.
    assert_eq!(modulators.mode(), Mode::Focused);
    assert_eq!(exploration.get(), 0.10);
    Ok(())
}

#[test]
fn acetylcholine_and_tonic_dopamine_are_distinct_load_bearing_knobs() -> Result<(), Box<dyn Error>>
{
    // Given: the baseline state — ACh defaults to Encode, phasic and tonic
    // dopamine are separate knobs.
    let baseline = Modulators::baseline();
    assert_eq!(baseline.acetylcholine(), Acetylcholine::Encode);
    // Phasic dopamine (learning_rate) and tonic dopamine (vigor) are independent.
    assert!((baseline.learning_rate().get() - 0.10).abs() < f32::EPSILON);
    assert!((baseline.tonic_dopamine().get() - 0.50).abs() < f32::EPSILON);

    // When: ACh is switched to Recall and tonic dopamine is raised.
    let modulators = baseline
        .with_acetylcholine(Acetylcholine::Recall)
        .with_tonic_dopamine(0.9)?;

    // Then: both knobs reflect the change, independently of the phasic signal.
    assert_eq!(modulators.acetylcholine(), Acetylcholine::Recall);
    assert!((modulators.tonic_dopamine().get() - 0.9).abs() < f32::EPSILON);
    assert!((modulators.learning_rate().get() - 0.10).abs() < f32::EPSILON);
    Ok(())
}
