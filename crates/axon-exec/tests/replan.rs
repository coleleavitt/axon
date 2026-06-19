use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use axon_core::{InputId, ModuleError, ModuleId, RunStatus, Runtime, Signal};
use axon_exec::{AgentSignal, Executive, Plan, Planner, RoutedTool, Step, wire_loop};
use axon_memory::{EpisodicStore, MemoryStore, RecallQuery};
use axon_modulate::Modulators;
use axon_predict::{Expected, Prediction, Verifier};
use axon_workspace::Workspace;

fn step(action: &'static str) -> Step {
    Step::new(
        action,
        Prediction::new(action, Expected::Contains("ok".to_owned())),
    )
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
fn a_severe_mismatch_triggers_a_plan_swap() -> Result<(), Box<dyn Error>> {
    // Given: a first plan whose step always fails, a replanner that swaps in a
    // plan whose step succeeds, and an executive that replans on a severe
    // mismatch instead of halting.
    let plan = Plan::new([step("risky")]);
    let memory = Rc::new(RefCell::new(EpisodicStore::new()));
    let workspace = Rc::new(RefCell::new(Workspace::new(8)?));

    let goal = InputId::new("goal")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let executive = ModuleId::new("executive")?;

    let mut runtime = Runtime::default();
    runtime.insert_module(
        Planner::new(planner.clone(), plan).with_replanner(|_reason| Plan::new([step("safe")])),
    )?;
    runtime.insert_module(RoutedTool::new(tool.clone(), |action: &str| {
        // "safe" yields the expected "ok"; everything else contradicts it.
        match action {
            "safe" => Ok::<_, ModuleError>("ok".to_owned()),
            other => Ok(format!("failed {other}")),
        }
    }))?;
    runtime.insert_module(
        Executive::new(
            executive.clone(),
            Rc::clone(&memory),
            Verifier,
            Modulators::baseline(),
            Rc::clone(&workspace),
        )
        .with_replan_threshold(0.5),
    )?;
    wire_loop(&mut runtime, &goal, &planner, &tool, &executive)?;

    // When: the loop runs; "risky" fails hard, the executive requests a replan,
    // and the planner swaps in the "safe" plan.
    let report = runtime.run(goal, Signal::new(AgentSignal::Goal))?;

    // Then: the run completes via the replanned plan rather than halting on the
    // mismatch, and both the failed and the recovered steps were recorded.
    assert!(halt_reason(report.into_status()).contains("complete"));
    assert_eq!(memory.borrow().recall(&RecallQuery::new("safe")).len(), 1);
    assert_eq!(memory.borrow().recall(&RecallQuery::new("risky")).len(), 1);
    Ok(())
}

#[test]
fn without_a_replanner_a_replan_request_halts_cleanly() -> Result<(), Box<dyn Error>> {
    // Given: the same severe-mismatch setup but no replanner installed.
    let plan = Plan::new([step("risky")]);
    let memory = Rc::new(RefCell::new(EpisodicStore::new()));
    let workspace = Rc::new(RefCell::new(Workspace::new(8)?));

    let goal = InputId::new("goal")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let executive = ModuleId::new("executive")?;

    let mut runtime = Runtime::default();
    runtime.insert_module(Planner::new(planner.clone(), plan))?;
    runtime.insert_module(RoutedTool::new(tool.clone(), |action: &str| {
        Ok::<_, ModuleError>(format!("failed {action}"))
    }))?;
    runtime.insert_module(
        Executive::new(
            executive.clone(),
            Rc::clone(&memory),
            Verifier,
            Modulators::baseline(),
            Rc::clone(&workspace),
        )
        .with_replan_threshold(0.5),
    )?;
    wire_loop(&mut runtime, &goal, &planner, &tool, &executive)?;

    // When/Then: the replan request reaches a planner that cannot replan, so the
    // loop halts cleanly rather than looping.
    let report = runtime.run(goal, Signal::new(AgentSignal::Goal))?;
    assert!(halt_reason(report.into_status()).contains("cannot replan"));
    Ok(())
}
