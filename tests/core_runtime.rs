use std::error::Error;

use axon::{
    Allow,
    DropSignal,
    EndpointId,
    InputId,
    MinPriority,
    Module,
    ModuleError,
    ModuleId,
    ModuleOutput,
    Priority,
    RoutingError,
    RunReport,
    RunStatus,
    Runtime,
    RuntimeError,
    Signal,
    StepLimit,
    Weight,
};

#[derive(Debug, PartialEq, Eq)]
enum Payload {
    Plan,
    Action,
    Observation,
    Ignored,
}

#[derive(Debug)]
struct FixedModule {
    id: ModuleId,
    output: ModuleOutput<Payload>,
}

impl FixedModule {
    fn new(id: ModuleId, output: ModuleOutput<Payload>) -> Self {
        Self { id, output }
    }
}

impl Module<Payload> for FixedModule {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(&mut self, _signal: Signal<Payload>) -> Result<ModuleOutput<Payload>, ModuleError> {
        Ok(std::mem::replace(&mut self.output, ModuleOutput::drop()))
    }
}

#[derive(Debug)]
struct EchoModule {
    id: ModuleId,
}

impl EchoModule {
    fn new(id: ModuleId) -> Self {
        Self { id }
    }
}

impl Module<Payload> for EchoModule {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(&mut self, signal: Signal<Payload>) -> Result<ModuleOutput<Payload>, ModuleError> {
        Ok(ModuleOutput::emit_signal(signal))
    }
}

fn assert_no_route_payload(report: &RunReport<Payload>, expected: &Payload) {
    match report.status() {
        RunStatus::NoRoute { at: _, signal } => assert_eq!(signal.payload(), expected),
        RunStatus::Stopped(signal) => panic!("unexpected stop: {signal:?}"),
        RunStatus::Dropped { at } => panic!("unexpected drop at {at}"),
        RunStatus::Halted { at } => panic!("unexpected halt at {at}"),
    }
}

#[test]
fn run_stops_with_observation_when_plan_reaches_tool() -> Result<(), Box<dyn Error>> {
    // Given: a planner and tool connected by explicit default-open routes.
    let input = InputId::new("input")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let mut runtime = Runtime::new(StepLimit::try_new(8)?);
    runtime.insert_module(FixedModule::new(
        planner.clone(),
        ModuleOutput::emit(Payload::Action),
    ))?;
    runtime.insert_module(FixedModule::new(
        tool.clone(),
        ModuleOutput::stop(Payload::Observation),
    ))?;
    runtime.add_input_route(input.clone(), planner.clone(), Weight::new(10), Allow)?;
    runtime.add_module_route(planner, tool, Weight::new(10), Allow)?;

    // When: a plan signal enters the graph.
    let report = runtime.run(input, Signal::new(Payload::Plan))?;

    // Then: the runtime records both graph hops and returns the terminal observation.
    assert_eq!(report.steps().len(), 2);
    match report.status() {
        RunStatus::Stopped(signal) => assert_eq!(signal.payload(), &Payload::Observation),
        RunStatus::Dropped { at } => panic!("unexpected drop at {at}"),
        RunStatus::NoRoute { at, signal: _ } => panic!("unexpected missing route at {at}"),
        RunStatus::Halted { at } => panic!("unexpected halt at {at}"),
    }
    Ok(())
}

#[test]
fn run_returns_no_route_when_default_deny_blocks_signal() -> Result<(), Box<dyn Error>> {
    // Given: an empty graph with no route from the external input channel.
    let input = InputId::new("input")?;
    let mut runtime = Runtime::<Payload>::new(StepLimit::try_new(4)?);

    // When: a signal enters without an explicit admitted route.
    let report = runtime.run(input.clone(), Signal::new(Payload::Plan))?;

    // Then: default-deny routing returns the unrouted signal instead of guessing.
    assert!(report.steps().is_empty());
    match report.status() {
        RunStatus::NoRoute { at, signal: _ } => assert_eq!(at, &EndpointId::from(input)),
        RunStatus::Stopped(signal) => panic!("unexpected stop: {signal:?}"),
        RunStatus::Dropped { at } => panic!("unexpected drop at {at}"),
        RunStatus::Halted { at } => panic!("unexpected halt at {at}"),
    }
    assert_no_route_payload(&report, &Payload::Plan);
    Ok(())
}

#[test]
fn run_rejects_ambiguous_routes_when_weights_tie() -> Result<(), Box<dyn Error>> {
    // Given: two admitted routes with the same source and same max weight.
    let input = InputId::new("input")?;
    let first = ModuleId::new("first")?;
    let second = ModuleId::new("second")?;
    let mut runtime = Runtime::new(StepLimit::try_new(4)?);
    runtime.insert_module(FixedModule::new(
        first.clone(),
        ModuleOutput::stop(Payload::Observation),
    ))?;
    runtime.insert_module(FixedModule::new(
        second.clone(),
        ModuleOutput::stop(Payload::Ignored),
    ))?;
    runtime.add_input_route(input.clone(), first, Weight::new(7), Allow)?;
    runtime.add_input_route(input.clone(), second, Weight::new(7), Allow)?;

    // When: routing attempts to choose a winner.
    let error = runtime
        .run(input.clone(), Signal::new(Payload::Plan))
        .err()
        .ok_or("expected ambiguous route error")?;

    // Then: insertion order is not used as a hidden tie-breaker.
    match error {
        RuntimeError::Routing(RoutingError::AmbiguousRoute { from, weight }) => {
            assert_eq!(from, EndpointId::from(input));
            assert_eq!(weight, Weight::new(7));
        }
        RuntimeError::MissingModule { id } => panic!("unexpected missing module {id}"),
        RuntimeError::DuplicateModule { id } => panic!("unexpected duplicate module {id}"),
        RuntimeError::Module { id, source } => panic!("unexpected module error {id}: {source}"),
        RuntimeError::StepLimitExceeded { limit, at } => {
            panic!("unexpected step limit {limit} at {at}")
        }
    }
    Ok(())
}

#[test]
fn run_stops_at_step_limit_when_graph_cycles() -> Result<(), Box<dyn Error>> {
    // Given: a self-looping module and a one-step runtime limit.
    let input = InputId::new("input")?;
    let echo = ModuleId::new("echo")?;
    let mut runtime = Runtime::new(StepLimit::try_new(1)?);
    runtime.insert_module(EchoModule::new(echo.clone()))?;
    runtime.add_input_route(input.clone(), echo.clone(), Weight::new(5), Allow)?;
    runtime.add_module_route(echo.clone(), echo.clone(), Weight::new(5), Allow)?;

    // When: the first step emits another signal.
    let error = runtime
        .run(input, Signal::new(Payload::Plan))
        .err()
        .ok_or("expected step limit error")?;

    // Then: the runtime refuses to spin forever.
    match error {
        RuntimeError::StepLimitExceeded { limit, at } => {
            assert_eq!(limit, StepLimit::try_new(1)?);
            assert_eq!(at, EndpointId::from(echo));
        }
        RuntimeError::Routing(source) => panic!("unexpected routing error: {source}"),
        RuntimeError::MissingModule { id } => panic!("unexpected missing module {id}"),
        RuntimeError::DuplicateModule { id } => panic!("unexpected duplicate module {id}"),
        RuntimeError::Module { id, source } => panic!("unexpected module error {id}: {source}"),
    }
    Ok(())
}

#[test]
fn min_priority_gate_blocks_low_priority_signals() -> Result<(), Box<dyn Error>> {
    // Given: a route that only admits high-priority signals.
    let input = InputId::new("input")?;
    let target = ModuleId::new("target")?;
    let mut runtime = Runtime::new(StepLimit::try_new(4)?);
    runtime.insert_module(FixedModule::new(
        target.clone(),
        ModuleOutput::stop(Payload::Observation),
    ))?;
    runtime.add_input_route(
        input.clone(),
        target,
        Weight::new(1),
        MinPriority::new(Priority::new(10)),
    )?;

    // When: the signal priority is below the gate threshold.
    let report = runtime.run(
        input,
        Signal::with_priority(Payload::Plan, Priority::new(3)),
    )?;

    // Then: the route is not admitted and the signal is returned as unrouted.
    assert!(report.steps().is_empty());
    assert_no_route_payload(&report, &Payload::Plan);
    Ok(())
}

#[test]
fn drop_signal_gate_always_blocks_route() -> Result<(), Box<dyn Error>> {
    // Given: an explicit route whose gate always denies admission.
    let input = InputId::new("input")?;
    let target = ModuleId::new("target")?;
    let mut runtime = Runtime::new(StepLimit::try_new(4)?);
    runtime.insert_module(FixedModule::new(
        target.clone(),
        ModuleOutput::stop(Payload::Observation),
    ))?;
    runtime.add_input_route(input.clone(), target, Weight::new(1), DropSignal)?;

    // When: the signal reaches the router.
    let report = runtime.run(input, Signal::new(Payload::Plan))?;

    // Then: even an explicit edge stays closed unless its gate admits the signal.
    assert!(report.steps().is_empty());
    assert_no_route_payload(&report, &Payload::Plan);
    Ok(())
}
