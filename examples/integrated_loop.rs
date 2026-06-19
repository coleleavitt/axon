//! The brain-layer stack driven *through* the routing core.
//!
//! Unlike `neuro_stack` (which composes the layers directly inside an
//! `Executor`), here the planner, tool, and executive are independent core
//! modules and every hand-off is a gated route. `Runtime::run` does the driving.

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use axon::exec::{AgentSignal, Executive, Plan, Planner, RoutedTool, Step, wire_loop};
use axon::memory::EpisodicStore;
use axon::modulate::Modulators;
use axon::predict::{Expected, Prediction, Verifier};
use axon::workspace::Workspace;
use axon::{InputId, ModuleError, ModuleId, RunStatus, Runtime, Signal};

fn main() -> Result<(), Box<dyn Error>> {
    let plan = Plan::new([
        Step::new(
            "list files",
            Prediction::new("list files", Expected::Contains("Cargo".to_owned())),
        ),
        Step::new(
            "read manifest",
            Prediction::new("read manifest", Expected::Contains("axon".to_owned())),
        ),
    ]);

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

    let report = runtime.run(goal, Signal::new(AgentSignal::Goal))?;

    println!("routed {} hops through the core:", report.steps().len());
    for hop in report.steps() {
        println!("  {} -> {}", hop.from(), hop.to());
    }
    if let RunStatus::Stopped(signal) = report.status() {
        if let AgentSignal::Halt { reason } = signal.payload() {
            println!("halted: {reason}");
        }
    }
    println!(
        "episodes recorded via routing: {}",
        memory.borrow().episodes().len()
    );
    println!(
        "workspace broadcasts via routing: {}",
        workspace.borrow().broadcasts().len()
    );
    Ok(())
}
