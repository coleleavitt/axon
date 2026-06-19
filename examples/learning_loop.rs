//! The learning loop, closed end to end.
//!
//! Two tools compete for every action: a flaky one (higher prior, so it wins
//! first) that contradicts the prediction, and a good one that satisfies it.
//! After each episode the agent reinforces the routes it traversed by the run's
//! graded prediction error — so a path that failed is weakened. Within a couple
//! of episodes the agent learns to route around the flaky tool. Nothing about
//! the wiring changes; only the learned weights do.

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use axon::exec::{
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
use axon::memory::EpisodicStore;
use axon::modulate::Modulators;
use axon::predict::{Expected, Prediction, Verifier};
use axon::workspace::Workspace;
use axon::{
    EndpointId,
    InputId,
    ModuleError,
    ModuleId,
    ProportionalPlasticity,
    RunReport,
    Runtime,
    Weight,
};

fn tool_used(report: &RunReport<AgentSignal>) -> String {
    report
        .steps()
        .iter()
        .find(|step| matches!(step.from(), EndpointId::Module(id) if id.as_str() == "planner"))
        .map_or_else(|| "<none>".to_owned(), |step| step.to().as_str().to_owned())
}

fn main() -> Result<(), Box<dyn Error>> {
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

    println!("episode  tool         error");
    for episode in 1..=4 {
        let report = agent.run_and_learn(&goal, AgentSignal::Goal)?;
        println!(
            "   {episode}     {:<12} {:.2}",
            tool_used(&report),
            agent.last_error()
        );
    }
    println!("\nthe agent learned to route around the flaky tool without rewiring.");
    Ok(())
}
