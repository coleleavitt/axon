use std::error::Error;

use axon_core::{
    Allow,
    EdgeId,
    EndpointId,
    FnModule,
    InputId,
    ModuleId,
    ModuleOutput,
    RunEvent,
    RunStatus,
    Runtime,
    Signal,
    StepLimit,
    Weight,
};

#[test]
fn an_oscillating_edge_is_halted_by_the_stall_guard() -> Result<(), Box<dyn Error>> {
    // Given: a module that re-emits onto a self-loop, which would otherwise spin
    // until the step limit, under a stall threshold of 3.
    let mut runtime: Runtime<u32> = Runtime::new(StepLimit::default()).with_stall_threshold(3);
    runtime.insert_module(FnModule::new(ModuleId::new("a")?, |signal: Signal<u32>| {
        Ok(ModuleOutput::emit(*signal.payload()))
    }))?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("a")?,
        Weight::new(1),
        Allow,
    )?;
    runtime.add_module_route(
        ModuleId::new("a")?,
        ModuleId::new("a")?,
        Weight::new(1),
        Allow,
    )?;

    // When: the run executes and the self-loop repeats.
    let mut stalled = None;
    let report = runtime.run_observed(InputId::new("in")?, Signal::new(1), &mut |event| {
        if let RunEvent::Stalled { edge, count } = event {
            stalled = Some((edge.clone(), *count));
        }
    })?;

    // Then: the run halts on the oscillating edge once it exceeds the threshold,
    // rather than erroring out at the step limit.
    assert!(matches!(report.status(), RunStatus::Halted { .. }));
    let Some((edge, count)) = stalled else {
        panic!("expected a stall event");
    };
    assert_eq!(edge.from(), &EndpointId::Module(ModuleId::new("a")?));
    assert_eq!(edge.to(), &ModuleId::new("a")?);
    assert_eq!(count, 4);
    // The self-loop edge identity is the one reported.
    assert_eq!(
        edge,
        EdgeId::new(EndpointId::Module(ModuleId::new("a")?), ModuleId::new("a")?)
    );
    Ok(())
}
