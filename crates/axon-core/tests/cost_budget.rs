use std::error::Error;

use axon_core::{
    Allow,
    Cost,
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

/// Chain in -> a -> b where a emits onward and b stops; each hop has a cost.
fn costed_chain(cost_in_a: u32, cost_a_b: u32) -> Result<Runtime<u32>, Box<dyn Error>> {
    let mut runtime = Runtime::new(StepLimit::default());
    runtime.insert_module(FnModule::new(ModuleId::new("a")?, |signal: Signal<u32>| {
        Ok(ModuleOutput::emit(*signal.payload()))
    }))?;
    runtime.insert_module(FnModule::new(ModuleId::new("b")?, |signal: Signal<u32>| {
        Ok(ModuleOutput::stop(*signal.payload()))
    }))?;
    runtime.add_route_with_cost(
        InputId::new("in")?.into(),
        ModuleId::new("a")?,
        Weight::new(1),
        Cost::new(cost_in_a),
        Allow,
    )?;
    runtime.add_route_with_cost(
        ModuleId::new("a")?.into(),
        ModuleId::new("b")?,
        Weight::new(1),
        Cost::new(cost_a_b),
        Allow,
    )?;
    Ok(runtime)
}

#[test]
fn a_run_refuses_a_route_it_cannot_afford() -> Result<(), Box<dyn Error>> {
    // Given: hops costing 3 then 5, under a budget of 6.
    let mut runtime = costed_chain(3, 5)?.with_budget(6);

    // When: the run spends 3 on the first hop, then cannot afford the second (8 > 6).
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;

    // Then: it sheds the unaffordable hop — stopping after the first, no route on.
    assert!(matches!(report.status(), RunStatus::NoRoute { .. }));
    assert_eq!(report.steps().len(), 1);
    Ok(())
}

#[test]
fn a_sufficient_budget_completes_the_run() -> Result<(), Box<dyn Error>> {
    // Given: the same costs under a budget that covers both hops (3 + 5 = 8).
    let mut runtime = costed_chain(3, 5)?.with_budget(8);

    // When/Then: the run completes through both hops.
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;
    assert!(matches!(report.status(), RunStatus::Stopped(_)));
    assert_eq!(report.steps().len(), 2);
    Ok(())
}

#[test]
fn no_budget_means_cost_is_ignored() -> Result<(), Box<dyn Error>> {
    // Given: expensive hops but no configured budget.
    let mut runtime = costed_chain(1000, 1000)?;

    // When/Then: cost is irrelevant without a budget; the run completes.
    let report = runtime.run(InputId::new("in")?, Signal::new(1))?;
    assert!(matches!(report.status(), RunStatus::Stopped(_)));
    Ok(())
}
