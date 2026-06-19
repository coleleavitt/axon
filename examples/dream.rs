//! Sleep-replay consolidation, end to end.
//!
//! The agent runs a few live episodes (a flaky tool competing with a good one),
//! recording each trajectory and its graded outcome into a `ReplayBuffer`. Then
//! it "sleeps": it replays those stored experiences offline — re-applying credit
//! assignment with no tool calls at all — and the learned routing consolidates
//! further. Hippocampal sharp-wave-ripple replay training the model during rest.

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
    EdgeId,
    EndpointId,
    InputId,
    ModuleError,
    ModuleId,
    ProportionalPlasticity,
    ReplayBuffer,
    Rng,
    Runtime,
    Weight,
};

fn learned(runtime: &Runtime<AgentSignal>, edge: &EdgeId) -> i16 {
    runtime
        .learned_weights()
        .into_iter()
        .find(|(candidate, _)| candidate == edge)
        .map_or(0, |(_, weight)| weight)
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
    runtime.add_module_route(executive, planner.clone(), Weight::new(10), OnAdvance)?;

    let to_good = EdgeId::new(EndpointId::Module(planner.clone()), good);
    let mut agent = LearningLoop::new(
        runtime,
        Rc::clone(&errors),
        Box::new(ProportionalPlasticity::default()),
        1.0,
        0.9,
    );

    // Two live episodes — record each trajectory and its graded error.
    let mut buffer = ReplayBuffer::new(16);
    for episode in 1..=2 {
        let report = agent.run_and_learn(&goal, AgentSignal::Goal)?;
        buffer.record(report.steps(), agent.last_error());
        println!("episode {episode}: error {:.2}", agent.last_error());
    }
    println!(
        "\nlearned weight to the good tool, awake: {}",
        learned(agent.runtime(), &to_good)
    );

    // Sleep: replay the stored experiences offline — no tools run.
    let plasticity = ProportionalPlasticity::default();
    let mut rng = Rng::seeded(7);
    buffer.replay(
        agent.runtime_mut(),
        &plasticity,
        1.0,
        0.9,
        20,
        &mut rng,
        &mut |_| {},
    );

    println!(
        "learned weight to the good tool, after sleep: {}",
        learned(agent.runtime(), &to_good)
    );
    println!("\nthe agent kept learning from memory alone — no tool was run during sleep.");
    Ok(())
}
