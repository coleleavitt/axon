//! The capstone: a model proposes a plan, the type system pins it down, and the
//! routing core drives it — with the run streamed as events.
//!
//! It ties together the provider seam (`axon::provider`), provider-backed
//! planning (`axon::exec::propose_plan`), the routed brain-layer loop
//! (`wire_loop`), and runtime event streaming (`run_observed`). The provider is
//! mocked, so this runs fully offline.

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use axon::exec::{AgentSignal, Executive, Planner, RoutedTool, propose_plan, wire_loop};
use axon::memory::EpisodicStore;
use axon::modulate::Modulators;
use axon::predict::Verifier;
use axon::provider::MockProvider;
use axon::workspace::Workspace;
use axon::{InputId, ModuleError, ModuleId, RunStatus, Runtime, Signal};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    // A model (mocked, offline) decomposes the goal into actions, each with an
    // expectation after `=>`. Planning is the only async hop here.
    let provider = MockProvider::scripted("list files => Cargo\nread manifest => axon");
    let plan = propose_plan(&provider, "summarize the axon manifest").await?;
    println!("planner proposed {} steps", plan.steps().len());

    let memory = Rc::new(RefCell::new(EpisodicStore::new()));
    let workspace = Rc::new(RefCell::new(Workspace::new(8)?));
    let goal = InputId::new("goal")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let executive = ModuleId::new("executive")?;

    let mut runtime = Runtime::default();
    runtime.insert_module(Planner::new(planner.clone(), plan))?;
    runtime.insert_module(RoutedTool::new(
        tool.clone(),
        |action: &str| -> Result<String, ModuleError> {
            match action {
                "list files" => Ok("Cargo.toml src target".to_owned()),
                "read manifest" => Ok("name = \"axon\"".to_owned()),
                other => Err(ModuleError::new(format!("unknown action: {other}"))),
            }
        },
    ))?;
    runtime.insert_module(Executive::new(
        executive.clone(),
        Rc::clone(&memory),
        Verifier,
        Modulators::baseline(),
        Rc::clone(&workspace),
    ))?;
    wire_loop(&mut runtime, &goal, &planner, &tool, &executive)?;

    // Drive the plan through the core, streaming each transition as it happens.
    let report = runtime.run_observed(goal, Signal::new(AgentSignal::Goal), &mut |event| {
        println!("  event: {event:?}");
    })?;

    if let RunStatus::Stopped(signal) = report.status() {
        if let AgentSignal::Halt { reason } = signal.payload() {
            println!("done: {reason}");
        }
    }
    println!("episodes recorded: {}", memory.borrow().episodes().len());
    Ok(())
}
