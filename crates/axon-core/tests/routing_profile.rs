use std::error::Error;

use axon_core::{
    Allow,
    EdgeId,
    EndpointId,
    FnModule,
    InputId,
    ModuleId,
    ModuleOutput,
    RoutingProfile,
    Runtime,
    Signal,
    StepLimit,
    Weight,
};

fn two_route_runtime() -> Result<Runtime<u32>, Box<dyn Error>> {
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
        Weight::new(5),
        Allow,
    )?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("b")?,
        Weight::new(4),
        Allow,
    )?;
    Ok(runtime)
}

fn input_edge(to: &'static str) -> Result<EdgeId, Box<dyn Error>> {
    Ok(EdgeId::new(
        EndpointId::from(InputId::new("in")?),
        ModuleId::new(to)?,
    ))
}

#[test]
fn a_profile_reconfigures_routing_without_rewiring() -> Result<(), Box<dyn Error>> {
    // Given: one fixed graph where `a` (5) beats `b` (4) by default.
    let mut runtime = two_route_runtime()?;
    let default = runtime.run(InputId::new("in")?, Signal::new(1))?;
    assert_eq!(default.steps()[0].edge().to(), &ModuleId::new("a")?);

    // When: a profile biases `b` above `a` (same graph, different mode).
    let profile = RoutingProfile::new().bias(input_edge("b")?, 3);
    runtime.apply_profile(&profile);
    let biased = runtime.run(InputId::new("in")?, Signal::new(2))?;

    // Then: `b` now wins — a transient routing configuration over fixed structure.
    assert_eq!(biased.steps()[0].edge().to(), &ModuleId::new("b")?);

    // And: clearing the profile restores the default configuration.
    runtime.clear_profile();
    let restored = runtime.run(InputId::new("in")?, Signal::new(3))?;
    assert_eq!(restored.steps()[0].edge().to(), &ModuleId::new("a")?);
    Ok(())
}
