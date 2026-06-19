//! repo-scout — a tiny repository investigator built **on top of** the axon SDK.
//!
//! It depends on the `axon` crate the way any downstream application would:
//!
//! 1. a [`Provider`] proposes a plan from a goal (OpenAI when configured, a
//!    deterministic mock otherwise, so it runs offline);
//! 2. the plan is decomposed into a typed [`Plan`](axon::exec) by `propose_plan`;
//! 3. the routed brain-layer loop drives it through the core, with each
//!    sensorimotor step backed by real `FsList`/`FsRead` tools;
//! 4. the run is streamed as events and the episodic memory is reported.
//!
//! Run offline:            `cargo run -p repo-scout -- "survey this repo"`
//! Run against OpenAI:     `OPENAI_API_KEY=... cargo run -p repo-scout --features openai`

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use axon::exec::{AgentSignal, Executive, Planner, RoutedTool, propose_plan, wire_loop};
use axon::memory::EpisodicStore;
use axon::modulate::Modulators;
use axon::predict::Verifier;
#[cfg(feature = "openai")]
use axon::provider::OpenAiProvider;
use axon::provider::{MockProvider, Provider};
use axon::tools::{FsList, FsRead, Tool};
use axon::workspace::Workspace;
use axon::{InputId, ModuleError, ModuleId, RunStatus, Runtime, Signal};

fn mock_provider() -> MockProvider {
    MockProvider::scripted("list files\nread manifest => axon")
}

#[cfg(feature = "openai")]
fn select_provider() -> Box<dyn Provider> {
    match OpenAiProvider::from_env() {
        Ok(provider) => {
            eprintln!("[scout] using OpenAI model {}", provider.model());
            Box::new(provider)
        }
        Err(error) => {
            eprintln!("[scout] {error}; falling back to the mock provider");
            Box::new(mock_provider())
        }
    }
}

#[cfg(not(feature = "openai"))]
fn select_provider() -> Box<dyn Provider> {
    eprintln!("[scout] built without the `openai` feature; using the mock provider");
    Box::new(mock_provider())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let goal = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "survey this repository".to_owned());

    // 1-2: provider proposes a plan; the type system pins it down.
    let provider = select_provider();
    let plan = propose_plan(provider.as_ref(), &goal).await?;
    println!("[scout] goal: {goal}");
    println!("[scout] planned {} step(s):", plan.steps().len());
    for step in plan.steps() {
        println!("  - {}", step.action());
    }

    // 3: wire the routed loop, backing the tool stage with real fs tools.
    let root = std::env::current_dir()?;
    let mut fs_read = FsRead::new(root.clone());
    let mut fs_list = FsList::new(root);

    let memory = Rc::new(RefCell::new(EpisodicStore::new()));
    let workspace = Rc::new(RefCell::new(Workspace::new(16)?));
    let goal_id = InputId::new("goal")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let executive = ModuleId::new("executive")?;

    let mut runtime = Runtime::default();
    runtime.insert_module(Planner::new(planner.clone(), plan))?;
    runtime.insert_module(RoutedTool::new(
        tool.clone(),
        move |action: &str| -> Result<String, ModuleError> {
            let observed = if action.contains("list") {
                fs_list
                    .call(".".to_owned())
                    .map(|entries| entries.join(" "))
            } else if action.contains("read") {
                fs_read.call("Cargo.toml".to_owned())
            } else {
                Ok(format!("(no scout tool for action: {action})"))
            };
            observed.map_err(|error| ModuleError::with_source("scout tool failed", error))
        },
    ))?;
    runtime.insert_module(Executive::new(
        executive.clone(),
        Rc::clone(&memory),
        Verifier,
        Modulators::baseline(),
        Rc::clone(&workspace),
    ))?;
    wire_loop(&mut runtime, &goal_id, &planner, &tool, &executive)?;

    // 4: drive it, streaming each transition.
    println!("[scout] running through the core:");
    let report = runtime.run_observed(goal_id, Signal::new(AgentSignal::Goal), &mut |event| {
        println!("  {event:?}");
    })?;

    if let RunStatus::Stopped(signal) = report.status() {
        if let AgentSignal::Halt { reason } = signal.payload() {
            println!("[scout] {reason}");
        }
    }
    println!(
        "[scout] recorded {} episode(s):",
        memory.borrow().episodes().len()
    );
    for episode in memory.borrow().episodes() {
        let preview: String = episode.text().chars().take(80).collect();
        println!("  - {preview}");
    }
    Ok(())
}
