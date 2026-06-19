use std::error::Error;

use axon_core::{
    FnModule,
    InputId,
    ModuleId,
    ModuleOutput,
    Phase,
    PhaseGate,
    RunStatus,
    Runtime,
    Signal,
    StepLimit,
    Weight,
};

/// One input feeding two modules, each active in a different phase.
fn phased_runtime() -> Result<Runtime<u32>, Box<dyn Error>> {
    let mut runtime = Runtime::new(StepLimit::default());
    runtime.insert_module(FnModule::new(
        ModuleId::new("day")?,
        |signal: Signal<u32>| Ok(ModuleOutput::stop(*signal.payload())),
    ))?;
    runtime.insert_module(FnModule::new(
        ModuleId::new("night")?,
        |signal: Signal<u32>| Ok(ModuleOutput::stop(*signal.payload())),
    ))?;
    // Same graph; phase decides which route is open.
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("day")?,
        Weight::new(10),
        PhaseGate::at(Phase::new(0)),
    )?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("night")?,
        Weight::new(10),
        PhaseGate::at(Phase::new(1)),
    )?;
    Ok(runtime)
}

#[test]
fn phase_multiplexes_routes_over_one_graph() -> Result<(), Box<dyn Error>> {
    let mut runtime = phased_runtime()?;

    // Phase 0: only the "day" route admits.
    let report = runtime.run(
        InputId::new("in")?,
        Signal::new(1).with_phase(Phase::new(0)),
    )?;
    assert_eq!(report.steps()[0].edge().to(), &ModuleId::new("day")?);

    // Phase 1: the same input now routes to "night" — no rewiring.
    let report = runtime.run(
        InputId::new("in")?,
        Signal::new(2).with_phase(Phase::new(1)),
    )?;
    assert_eq!(report.steps()[0].edge().to(), &ModuleId::new("night")?);

    // A phase no route is active in: nothing fires.
    let report = runtime.run(
        InputId::new("in")?,
        Signal::new(3).with_phase(Phase::new(2)),
    )?;
    assert!(matches!(report.status(), RunStatus::NoRoute { .. }));
    Ok(())
}

#[test]
fn a_signal_keeps_its_phase_across_a_map() -> Result<(), Box<dyn Error>> {
    // Phase rides along with the payload through transforms.
    let signal = Signal::new(1u32).with_phase(Phase::new(3));
    let mapped = signal.map(|value| value + 1);
    assert_eq!(mapped.phase(), Phase::new(3));
    assert_eq!(*mapped.payload(), 2);
    Ok(())
}
