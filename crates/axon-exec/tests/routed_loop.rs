use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use axon_core::{InputId, ModuleError, ModuleId, RunStatus, Runtime, Signal};
use axon_exec::{AgentSignal, Executive, Plan, Planner, RoutedTool, Step, wire_loop};
use axon_memory::EpisodicStore;
use axon_modulate::{Mode, Modulators};
use axon_predict::{Expected, Prediction, Verifier};
use axon_workspace::Workspace;

fn failing_tool(action: &str) -> Result<String, ModuleError> {
    Ok(format!("could not {action}"))
}

fn step(action: &'static str, expected: &str) -> Step {
    Step::new(
        action,
        Prediction::new(action, Expected::Contains(expected.to_owned())),
    )
}

fn echo_tool(action: &str) -> Result<String, ModuleError> {
    Ok(format!("did {action}"))
}

fn halt_reason(status: RunStatus<AgentSignal>) -> String {
    match status {
        RunStatus::Stopped(signal) => match signal.into_payload() {
            AgentSignal::Halt { reason } => reason,
            other => panic!("expected halt, got {other:?}"),
        },
        other => panic!("expected stopped run, got {other:?}"),
    }
}

#[test]
fn routed_loop_drives_every_layer_through_the_core() -> Result<(), Box<dyn Error>> {
    // Given: planner, tool, and executive wired only by core routes and gates.
    let plan = Plan::new([step("read", "did"), step("write", "did")]);
    let memory = Rc::new(RefCell::new(EpisodicStore::new()));
    let workspace = Rc::new(RefCell::new(Workspace::new(8)?));

    let goal = InputId::new("goal")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let executive = ModuleId::new("executive")?;

    let mut runtime = Runtime::default();
    runtime.insert_module(Planner::new(planner.clone(), plan))?;
    runtime.insert_module(RoutedTool::new(tool.clone(), echo_tool))?;
    runtime.insert_module(Executive::new(
        executive.clone(),
        Rc::clone(&memory),
        Verifier,
        Modulators::baseline(),
        Rc::clone(&workspace),
    ))?;
    wire_loop(&mut runtime, &goal, &planner, &tool, &executive)?;

    // When: a single goal signal is injected at the input endpoint.
    let report = runtime.run(goal, Signal::new(AgentSignal::Goal))?;

    // Then: the whole cycle ran through the core and both layers were populated
    // by routed signals, not by a private side loop.
    assert_eq!(report.steps().len(), 7);
    assert_eq!(memory.borrow().episodes().len(), 2);
    assert_eq!(workspace.borrow().broadcasts().len(), 2);
    assert!(halt_reason(report.into_status()).contains("complete after 2 steps"));
    Ok(())
}

#[test]
fn routed_executive_halts_the_core_loop_on_mismatch() -> Result<(), Box<dyn Error>> {
    // Given: a focused executive and a first step whose prediction cannot hold.
    let plan = Plan::new([step("read", "absent"), step("write", "did")]);
    let memory = Rc::new(RefCell::new(EpisodicStore::new()));
    let workspace = Rc::new(RefCell::new(Workspace::new(8)?));

    let goal = InputId::new("goal")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let executive = ModuleId::new("executive")?;

    let mut runtime = Runtime::default();
    runtime.insert_module(Planner::new(planner.clone(), plan))?;
    runtime.insert_module(RoutedTool::new(tool.clone(), echo_tool))?;
    runtime.insert_module(Executive::new(
        executive.clone(),
        Rc::clone(&memory),
        Verifier,
        Modulators::baseline().with_mode(Mode::Focused),
        Rc::clone(&workspace),
    ))?;
    wire_loop(&mut runtime, &goal, &planner, &tool, &executive)?;

    // When: the loop runs and the executive sees a contradicted prediction.
    let report = runtime.run(goal, Signal::new(AgentSignal::Goal))?;

    // Then: it halts the core loop at the first step rather than advancing.
    assert_eq!(report.steps().len(), 3);
    assert_eq!(memory.borrow().episodes().len(), 1);
    assert!(halt_reason(report.into_status()).contains("prediction mismatch in read"));
    Ok(())
}

#[test]
fn exploratory_executive_abandons_a_perpetually_failing_step() -> Result<(), Box<dyn Error>> {
    // Given: an Exploratory executive (which maps a mismatch to Retry) bounded to
    // one retry, a tool that always contradicts the prediction, and a generous
    // step budget — so only the perseveration guard, not the step limit, can stop
    // the loop.
    let plan = Plan::new([step("read", "did")]);
    let memory = Rc::new(RefCell::new(EpisodicStore::new()));
    let workspace = Rc::new(RefCell::new(Workspace::new(8)?));

    let goal = InputId::new("goal")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let executive = ModuleId::new("executive")?;

    let mut runtime = Runtime::default();
    runtime.insert_module(Planner::new(planner.clone(), plan))?;
    runtime.insert_module(RoutedTool::new(tool.clone(), failing_tool))?;
    runtime.insert_module(
        Executive::new(
            executive.clone(),
            Rc::clone(&memory),
            Verifier,
            Modulators::baseline().with_mode(Mode::Exploratory),
            Rc::clone(&workspace),
        )
        .with_max_retries(1),
    )?;
    wire_loop(&mut runtime, &goal, &planner, &tool, &executive)?;

    // When: the loop runs against a step that can never succeed.
    let report = runtime.run(goal, Signal::new(AgentSignal::Goal))?;

    // Then: it abandons the step after its retry budget instead of perseverating
    // until the step limit — the run completes Ok with a clear reason.
    assert!(halt_reason(report.into_status()).contains("abandoned step 0 after 1 retries"));
    Ok(())
}
