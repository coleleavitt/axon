use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use axon_core::{
    EndpointId,
    InputId,
    ModuleError,
    ModuleId,
    ProportionalPlasticity,
    RunReport,
    Runtime,
    Weight,
};
use axon_exec::{
    AgentSignal,
    Executive,
    LearningLoop,
    OnAct,
    OnAdvance,
    OnGoal,
    OnObserve,
    OutcomeError,
    Plan,
    Planner,
    RoutedTool,
    Step,
};
use axon_memory::EpisodicStore;
use axon_modulate::Modulators;
use axon_predict::{Expected, Prediction, Verifier};
use axon_workspace::Workspace;

/// Which tool a run actually routed through — the `to` of the planner -> tool hop.
fn tool_used(report: &RunReport<AgentSignal>) -> Option<String> {
    report
        .steps()
        .iter()
        .find(|step| matches!(step.from(), EndpointId::Module(id) if id.as_str() == "planner"))
        .map(|step| step.to().as_str().to_owned())
}

#[test]
fn the_agent_learns_to_route_around_a_failing_tool() -> Result<(), Box<dyn Error>> {
    // Given: a one-step plan and two tools competing for the Act hand-off — a
    // flaky tool (higher prior, so it wins first) that contradicts the prediction
    // and a good tool that satisfies it.
    let plan = Plan::new([Step::new(
        "do",
        Prediction::new("do", Expected::Contains("ok".to_owned())),
    )]);
    let memory = Rc::new(RefCell::new(EpisodicStore::new()));
    let workspace = Rc::new(RefCell::new(Workspace::new(8)?));
    let errors = Rc::new(RefCell::new(OutcomeError::default()));

    let goal = InputId::new("goal")?;
    let planner = ModuleId::new("planner")?;
    let good = ModuleId::new("tool_good")?;
    let flaky = ModuleId::new("tool_flaky")?;
    let executive = ModuleId::new("executive")?;

    let mut runtime = Runtime::default();
    runtime.insert_module(Planner::new(planner.clone(), plan))?;
    runtime.insert_module(RoutedTool::new(good.clone(), |_action: &str| {
        Ok::<_, ModuleError>("ok done".to_owned())
    }))?;
    runtime.insert_module(RoutedTool::new(flaky.clone(), |_action: &str| {
        Ok::<_, ModuleError>("nope".to_owned())
    }))?;
    runtime.insert_module(
        Executive::new(
            executive.clone(),
            Rc::clone(&memory),
            Verifier,
            Modulators::baseline(),
            Rc::clone(&workspace),
        )
        .with_error_meter(Rc::clone(&errors)),
    )?;

    // The flaky tool starts with the higher weight, so argmax picks it first.
    runtime.add_input_route(goal.clone(), planner.clone(), Weight::new(10), OnGoal)?;
    runtime.add_module_route(planner.clone(), good.clone(), Weight::new(10), OnAct)?;
    runtime.add_module_route(planner.clone(), flaky.clone(), Weight::new(11), OnAct)?;
    runtime.add_module_route(good.clone(), executive.clone(), Weight::new(10), OnObserve)?;
    runtime.add_module_route(flaky.clone(), executive.clone(), Weight::new(10), OnObserve)?;
    runtime.add_module_route(
        executive.clone(),
        planner.clone(),
        Weight::new(10),
        OnAdvance,
    )?;

    let mut agent = LearningLoop::new(
        runtime,
        Rc::clone(&errors),
        Box::new(ProportionalPlasticity::default()),
        1.0,
        0.9,
    );

    // When: the first episode runs.
    let episode_1 = agent.run_and_learn(&goal, AgentSignal::Goal)?;

    // Then: it took the flaky tool and got a maximal prediction error.
    assert_eq!(tool_used(&episode_1).as_deref(), Some("tool_flaky"));
    assert!((agent.last_error() - 1.0).abs() < f32::EPSILON);

    // When: the agent runs again, having been penalized for the flaky path.
    let episode_2 = agent.run_and_learn(&goal, AgentSignal::Goal)?;

    // Then: it has learned to route through the good tool, and the outcome is now
    // clean — the static substrate improved with experience.
    assert_eq!(tool_used(&episode_2).as_deref(), Some("tool_good"));
    assert!(agent.last_error().abs() < f32::EPSILON);

    // And: the choice is stable — a third episode stays on the good tool.
    let episode_3 = agent.run_and_learn(&goal, AgentSignal::Goal)?;
    assert_eq!(tool_used(&episode_3).as_deref(), Some("tool_good"));
    Ok(())
}
