use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use axon_core::{InputId, ModuleError, ModuleId, RunStatus, Runtime, Signal, Weight};
use axon_exec::{
    AgentSignal,
    Executive,
    KeywordRisk,
    OnAdvance,
    OnGoal,
    OnObserve,
    Plan,
    Planner,
    RiskGate,
    RoutedTool,
    Step,
};
use axon_memory::EpisodicStore;
use axon_modulate::Modulators;
use axon_predict::{Expected, Prediction, Verifier};
use axon_workspace::Workspace;

/// Wire goal -> planner -> tool (gated by `risk_gate`) -> executive -> planner,
/// with a one-step plan whose action is `action`.
fn risk_gated_runtime(
    action: &'static str,
    tolerance: f32,
) -> Result<Runtime<AgentSignal>, Box<dyn Error>> {
    let plan = Plan::new([Step::new(
        action,
        Prediction::new(action, Expected::Anything),
    )]);
    let memory = Rc::new(RefCell::new(EpisodicStore::new()));
    let workspace = Rc::new(RefCell::new(Workspace::new(8)?));

    let goal = InputId::new("goal")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let executive = ModuleId::new("executive")?;

    let mut runtime = Runtime::default();
    runtime.insert_module(Planner::new(planner.clone(), plan))?;
    runtime.insert_module(RoutedTool::new(tool.clone(), |action: &str| {
        Ok::<_, ModuleError>(format!("ran {action}"))
    }))?;
    runtime.insert_module(Executive::new(
        executive.clone(),
        Rc::clone(&memory),
        Verifier,
        Modulators::baseline(),
        Rc::clone(&workspace),
    ))?;
    runtime.add_input_route(goal, planner.clone(), Weight::new(10), OnGoal)?;
    runtime.add_module_route(
        planner.clone(),
        tool.clone(),
        Weight::new(10),
        RiskGate::new(KeywordRisk::default(), tolerance),
    )?;
    runtime.add_module_route(tool, executive.clone(), Weight::new(10), OnObserve)?;
    runtime.add_module_route(executive, planner, Weight::new(10), OnAdvance)?;
    Ok(runtime)
}

#[test]
fn a_dangerous_action_is_deferred_at_baseline_risk_tolerance() -> Result<(), Box<dyn Error>> {
    // Given: the baseline risk tolerance (0.30) — now actually consulted.
    let tolerance = Modulators::baseline().risk_tolerance().get();
    let mut runtime = risk_gated_runtime("rm -rf /", tolerance)?;

    // When/Then: the destructive action exceeds tolerance, so the act hand-off is
    // not admitted and the agent declines rather than executing it.
    let report = runtime.run(InputId::new("goal")?, Signal::new(AgentSignal::Goal))?;
    assert!(matches!(report.status(), RunStatus::NoRoute { .. }));
    Ok(())
}

#[test]
fn a_safe_action_passes_the_risk_gate() -> Result<(), Box<dyn Error>> {
    // Given: the same low tolerance but a harmless action.
    let tolerance = Modulators::baseline().risk_tolerance().get();
    let mut runtime = risk_gated_runtime("read the manifest", tolerance)?;

    // When/Then: a safe action is admitted and the loop runs to completion.
    let report = runtime.run(InputId::new("goal")?, Signal::new(AgentSignal::Goal))?;
    assert!(matches!(report.status(), RunStatus::Stopped(_)));
    Ok(())
}

#[test]
fn high_risk_tolerance_admits_the_dangerous_action() -> Result<(), Box<dyn Error>> {
    // Given: maximal risk tolerance.
    let mut runtime = risk_gated_runtime("rm -rf /", 1.0)?;

    // When/Then: with full tolerance even the dangerous action is admitted — the
    // knob is genuinely load-bearing.
    let report = runtime.run(InputId::new("goal")?, Signal::new(AgentSignal::Goal))?;
    assert!(matches!(report.status(), RunStatus::Stopped(_)));
    Ok(())
}
