use std::error::Error;

use axon_core::{
    Disinhibit,
    DropSignal,
    FnModule,
    InputId,
    ModuleId,
    ModuleOutput,
    Release,
    RunStatus,
    Runtime,
    Signal,
    StepLimit,
    Weight,
};

#[test]
fn a_disinhibited_route_opens_only_while_released() -> Result<(), Box<dyn Error>> {
    // Given: a route guarded by a normally-closed inhibitor (DropSignal) wrapped
    // in a disinhibitory gate driven by a shared release switch.
    let release = Release::new();
    let mut runtime: Runtime<u32> = Runtime::new(StepLimit::default());
    runtime.insert_module(FnModule::new(
        ModuleId::new("top_down")?,
        |signal: Signal<u32>| Ok(ModuleOutput::stop(*signal.payload())),
    ))?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("top_down")?,
        Weight::new(10),
        Disinhibit::new(DropSignal, release.clone()),
    )?;

    // When held (default): the inhibitor blocks, so no route is taken.
    let blocked = runtime.run(InputId::new("in")?, Signal::new(1))?;
    assert!(matches!(blocked.status(), RunStatus::NoRoute { .. }));

    // When released: top-down state opens the otherwise-closed route.
    release.release();
    let opened = runtime.run(InputId::new("in")?, Signal::new(2))?;
    assert!(matches!(opened.status(), RunStatus::Stopped(_)));
    assert_eq!(opened.steps()[0].edge().to(), &ModuleId::new("top_down")?);

    // When held again: it closes back up.
    release.hold();
    let blocked_again = runtime.run(InputId::new("in")?, Signal::new(3))?;
    assert!(matches!(blocked_again.status(), RunStatus::NoRoute { .. }));
    Ok(())
}
