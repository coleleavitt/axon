use std::error::Error;

use axon_core::{
    Allow,
    FnModule,
    InputId,
    ModuleId,
    ModuleOutput,
    Route,
    RunStatus,
    Runtime,
    Sign,
    Signal,
    StepLimit,
    Weight,
};

/// A runtime where input `in` has an excitatory route to `act` and an admitted
/// inhibitory edge to `veto`, at the given weights.
fn inhibited_runtime(act_weight: i16, veto_weight: i16) -> Result<Runtime<u32>, Box<dyn Error>> {
    let mut runtime = Runtime::new(StepLimit::default());
    runtime.insert_module(FnModule::new(
        ModuleId::new("act")?,
        |signal: Signal<u32>| Ok(ModuleOutput::stop(*signal.payload())),
    ))?;
    runtime.insert_module(FnModule::new(
        ModuleId::new("veto")?,
        |signal: Signal<u32>| Ok(ModuleOutput::stop(*signal.payload())),
    ))?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("act")?,
        Weight::new(act_weight),
        Allow,
    )?;
    runtime.add_built_route(
        Route::new(
            InputId::new("in")?.into(),
            ModuleId::new("veto")?,
            Weight::new(veto_weight),
            Allow,
        )
        .with_sign(Sign::Inhibitory),
    )?;
    Ok(runtime)
}

#[test]
fn an_inhibitory_edge_suppresses_the_excitatory_winner() -> Result<(), Box<dyn Error>> {
    // Given: an excitatory route (weight 5) and inhibition strong enough (6) to
    // overwhelm it.
    let mut runtime = inhibited_runtime(5, 6)?;

    // When/Then: nothing fires — and the inhibitory edge is never itself selected.
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;
    assert!(matches!(report.status(), RunStatus::NoRoute { .. }));
    assert!(report.steps().is_empty());
    Ok(())
}

#[test]
fn weak_inhibition_does_not_suppress_a_strong_excitatory_edge() -> Result<(), Box<dyn Error>> {
    // Given: a strong excitatory route (weight 9) and weak inhibition (weight 2).
    let mut runtime = inhibited_runtime(9, 2)?;

    // When/Then: the excitatory edge clears the inhibition and fires.
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;
    assert!(matches!(report.status(), RunStatus::Stopped(_)));
    assert_eq!(report.steps()[0].edge().to(), &ModuleId::new("act")?);
    Ok(())
}
