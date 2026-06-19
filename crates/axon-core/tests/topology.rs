use std::error::Error;

use axon_core::{
    Allow,
    EndpointId,
    FnModule,
    InputId,
    ModuleId,
    ModuleOutput,
    Runtime,
    Signal,
    StepLimit,
};

/// Wire the canonical agent loop: goal -> planner -> tool -> executive -> planner.
fn loop_runtime() -> Result<Runtime<u32>, Box<dyn Error>> {
    let mut runtime = Runtime::new(StepLimit::default());
    for name in ["planner", "tool", "executive"] {
        runtime.insert_module(FnModule::new(
            ModuleId::new(name)?,
            |signal: Signal<u32>| Ok(ModuleOutput::stop(*signal.payload())),
        ))?;
    }
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let executive = ModuleId::new("executive")?;
    runtime.add_input_route(
        InputId::new("goal")?,
        planner.clone(),
        Default::default(),
        Allow,
    )?;
    runtime.add_module_route(planner.clone(), tool.clone(), Default::default(), Allow)?;
    runtime.add_module_route(tool, executive.clone(), Default::default(), Allow)?;
    runtime.add_module_route(executive, planner, Default::default(), Allow)?;
    Ok(runtime)
}

#[test]
fn graph_reports_degree_hub_and_reachability() -> Result<(), Box<dyn Error>> {
    // Given: the wired loop topology.
    let runtime = loop_runtime()?;
    let graph = runtime.graph();
    let planner = ModuleId::new("planner")?;

    // Then: degrees match the wiring — the planner is fed by both the goal and the
    // executive (in-degree 2) and feeds the tool (out-degree 1).
    assert_eq!(graph.edge_count(), 4);
    assert_eq!(graph.in_degree(&planner), 2);
    assert_eq!(graph.out_degree(&EndpointId::Module(planner.clone())), 1);

    // And: the planner is the hub (highest total degree).
    assert_eq!(graph.hub(), Some(planner));

    // And: every module is reachable from the goal input.
    let reachable = graph.reachable_from(&EndpointId::from(InputId::new("goal")?));
    assert_eq!(reachable.len(), 3);
    assert!(reachable.contains(&ModuleId::new("tool")?));
    assert!(reachable.contains(&ModuleId::new("executive")?));
    Ok(())
}
