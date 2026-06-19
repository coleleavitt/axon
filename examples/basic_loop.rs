use std::error::Error;

use axon::{
    Allow,
    FnModule,
    InputId,
    Module,
    ModuleError,
    ModuleId,
    ModuleOutput,
    RunStatus,
    Runtime,
    Signal,
    Weight,
};

#[derive(Debug, PartialEq, Eq)]
enum Payload {
    Goal,
    Action,
    Observation,
}

#[derive(Debug)]
struct Planner {
    id: ModuleId,
}

impl Planner {
    fn new(id: ModuleId) -> Self {
        Self { id }
    }
}

impl Module<Payload> for Planner {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(&mut self, signal: Signal<Payload>) -> Result<ModuleOutput<Payload>, ModuleError> {
        match signal.into_payload() {
            Payload::Goal => Ok(ModuleOutput::emit(Payload::Action)),
            Payload::Action => Err(ModuleError::new("planner cannot plan from an action")),
            Payload::Observation => {
                Err(ModuleError::new("planner cannot plan from an observation"))
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let input = InputId::new("input")?;
    let planner = ModuleId::new("planner")?;
    let tool = ModuleId::new("tool")?;
    let mut runtime = Runtime::default();

    runtime.insert_module(Planner::new(planner.clone()))?;
    runtime.insert_module(FnModule::new(
        tool.clone(),
        |signal: Signal<Payload>| match signal.into_payload() {
            Payload::Goal => Err(ModuleError::new("tool cannot execute a goal directly")),
            Payload::Action => Ok(ModuleOutput::stop(Payload::Observation)),
            Payload::Observation => Err(ModuleError::new("tool cannot execute an observation")),
        },
    ))?;
    runtime.add_input_route(input.clone(), planner.clone(), Weight::new(10), Allow)?;
    runtime.add_module_route(planner, tool, Weight::new(10), Allow)?;

    let report = runtime.run(input, Signal::new(Payload::Goal))?;
    assert_eq!(report.steps().len(), 2);

    match report.into_status() {
        RunStatus::Stopped(signal) => assert_eq!(signal.into_payload(), Payload::Observation),
        RunStatus::Dropped { at } => panic!("unexpected drop at {at}"),
        RunStatus::NoRoute { at, signal: _ } => panic!("unexpected missing route at {at}"),
        RunStatus::Halted { at } => panic!("unexpected halt at {at}"),
    }

    Ok(())
}
