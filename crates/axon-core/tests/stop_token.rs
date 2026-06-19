use std::error::Error;

use axon_core::{
    Allow,
    FnModule,
    InputId,
    ModuleId,
    ModuleOutput,
    RunEvent,
    RunStatus,
    Runtime,
    Signal,
    StepLimit,
    StopToken,
    Weight,
};

#[test]
fn a_pre_stopped_token_halts_before_any_step() -> Result<(), Box<dyn Error>> {
    // Given: a runtime with an already-stopped token.
    let stop = StopToken::new();
    let mut runtime: Runtime<u32> =
        Runtime::new(StepLimit::default()).with_stop_token(stop.clone());
    runtime.insert_module(FnModule::new(ModuleId::new("a")?, |signal: Signal<u32>| {
        Ok(ModuleOutput::stop(*signal.payload()))
    }))?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("a")?,
        Weight::new(1),
        Allow,
    )?;
    stop.stop();

    // When/Then: the run halts immediately, taking no steps.
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;
    assert!(matches!(report.status(), RunStatus::Halted { .. }));
    assert!(report.steps().is_empty());
    Ok(())
}

#[test]
fn a_module_can_stop_the_run_mid_flight() -> Result<(), Box<dyn Error>> {
    // Given: a chain in -> a -> b, where module `a` trips a shared stop token as
    // a side effect of running, then emits onward.
    let stop = StopToken::new();
    let trip = stop.clone();
    let mut runtime: Runtime<u32> =
        Runtime::new(StepLimit::default()).with_stop_token(stop.clone());
    runtime.insert_module(FnModule::new(
        ModuleId::new("a")?,
        move |signal: Signal<u32>| {
            trip.stop();
            Ok(ModuleOutput::emit(*signal.payload()))
        },
    ))?;
    runtime.insert_module(FnModule::new(ModuleId::new("b")?, |signal: Signal<u32>| {
        Ok(ModuleOutput::stop(*signal.payload()))
    }))?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("a")?,
        Weight::new(1),
        Allow,
    )?;
    runtime.add_module_route(
        ModuleId::new("a")?,
        ModuleId::new("b")?,
        Weight::new(1),
        Allow,
    )?;

    // When: the run proceeds; `a` runs, trips the brake, then the loop checks it.
    let mut halted_at = None;
    let report = runtime.run_observed(InputId::new("in")?, Signal::new(1), &mut |event| {
        if let RunEvent::Halted { at } = event {
            halted_at = Some(at.clone());
        }
    })?;

    // Then: the run halts before `b` ever handles the signal — `a` ran, `b` did not.
    assert!(matches!(report.status(), RunStatus::Halted { .. }));
    assert_eq!(report.steps().len(), 1);
    assert!(halted_at.is_some());
    Ok(())
}
