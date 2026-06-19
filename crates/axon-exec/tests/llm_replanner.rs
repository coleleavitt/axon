use std::cell::RefCell;
use std::error::Error;
use std::iter;
use std::rc::Rc;

use axon_core::{InputId, ModuleError, ModuleId, RunStatus, Runtime, Signal};
use axon_exec::{AgentSignal, Executive, Plan, Planner, RoutedTool, Step, propose_plan, wire_loop};
use axon_memory::{EpisodicStore, MemoryStore, RecallQuery};
use axon_modulate::Modulators;
use axon_predict::{Expected, Prediction, Verifier};
use axon_provider::MockProvider;
use axon_workspace::Workspace;

#[test]
fn an_llm_backed_replanner_swaps_in_a_proposed_plan() -> Result<(), Box<dyn Error>> {
    // A current-thread executor lets the sync routed loop drive the async provider
    // from inside the replanner — the app owns the executor; the library stays
    // async-pure (the core never calls a model).
    let executor = tokio::runtime::Builder::new_current_thread().build()?;
    // The provider proposes a one-line plan whose step is satisfiable.
    let provider = MockProvider::scripted("run safely => ok");

    let first = Plan::new([Step::new(
        "risky",
        Prediction::new("risky", Expected::Contains("ok".to_owned())),
    )]);
    let memory = Rc::new(RefCell::new(EpisodicStore::new()));
    let workspace = Rc::new(RefCell::new(Workspace::new(8)?));

    let goal = InputId::new("goal")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let executive = ModuleId::new("executive")?;

    let mut runtime = Runtime::default();
    runtime.insert_module(Planner::new(planner.clone(), first).with_replanner(
        move |reason: &str| {
            // Ask the LLM (provider) for a fresh plan, blocking on the async call.
            executor
                .block_on(propose_plan(&provider, reason))
                .unwrap_or_else(|_| Plan::new(iter::empty::<Step>()))
        },
    ))?;
    runtime.insert_module(RoutedTool::new(tool.clone(), |action: &str| match action {
        "run safely" => Ok::<_, ModuleError>("ok".to_owned()),
        other => Ok(format!("failed {other}")),
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

    // When: "risky" fails hard, the executive requests a replan, and the planner
    // asks the provider for a new plan.
    let report = runtime.run(goal, Signal::new(AgentSignal::Goal))?;

    // Then: the LLM-proposed "run safely" plan ran and the loop completed.
    assert!(matches!(report.status(), RunStatus::Stopped(_)));
    assert_eq!(memory.borrow().recall(&RecallQuery::new("safely")).len(), 1);
    Ok(())
}
