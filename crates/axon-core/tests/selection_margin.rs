use std::error::Error;

use axon_core::{
    Allow,
    FnModule,
    InputId,
    ModuleId,
    ModuleOutput,
    RunStatus,
    Runtime,
    Signal,
    StepLimit,
    Weight,
};

/// One input feeding two stopping modules `a` and `b` at the given weights.
fn two_route_runtime(weight_a: i16, weight_b: i16) -> Result<Runtime<u32>, Box<dyn Error>> {
    let mut runtime = Runtime::new(StepLimit::default());
    runtime.insert_module(FnModule::new(ModuleId::new("a")?, |signal: Signal<u32>| {
        Ok(ModuleOutput::stop(*signal.payload()))
    }))?;
    runtime.insert_module(FnModule::new(ModuleId::new("b")?, |signal: Signal<u32>| {
        Ok(ModuleOutput::stop(*signal.payload()))
    }))?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("a")?,
        Weight::new(weight_a),
        Allow,
    )?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("b")?,
        Weight::new(weight_b),
        Allow,
    )?;
    Ok(runtime)
}

#[test]
fn a_close_call_is_suppressed_under_a_nogo_margin() -> Result<(), Box<dyn Error>> {
    // Given: two near-equal routes (5 vs 4) and a NoGo margin of 2.
    let mut runtime = two_route_runtime(5, 4)?.with_margin(2);

    // When: the runtime selects.
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;

    // Then: no option clearly wins, so the runtime declines to act.
    assert!(matches!(report.status(), RunStatus::NoRoute { .. }));
    assert!(report.steps().is_empty());
    Ok(())
}

#[test]
fn a_clear_winner_acts_despite_the_margin() -> Result<(), Box<dyn Error>> {
    // Given: a decisive lead (5 vs 2) that clears the margin of 2.
    let mut runtime = two_route_runtime(5, 2)?.with_margin(2);

    // When/Then: the clear winner is taken.
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;
    assert!(matches!(report.status(), RunStatus::Stopped(_)));
    assert_eq!(report.steps()[0].edge().to(), &ModuleId::new("a")?);
    Ok(())
}

#[test]
fn a_sole_candidate_always_acts() -> Result<(), Box<dyn Error>> {
    // Given: a single route under a large margin.
    let mut runtime = Runtime::new(StepLimit::default()).with_margin(100);
    runtime.insert_module(FnModule::new(
        ModuleId::new("only")?,
        |signal: Signal<u32>| Ok(ModuleOutput::stop(*signal.payload())),
    ))?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("only")?,
        Weight::new(1),
        Allow,
    )?;

    // When/Then: with no competitor, there is nothing to out-margin, so it acts.
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;
    assert!(matches!(report.status(), RunStatus::Stopped(_)));
    Ok(())
}
