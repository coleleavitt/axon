use std::error::Error;
use std::fmt;

use axon::{
    Allow,
    EndpointId,
    InputId,
    Module,
    ModuleError,
    ModuleId,
    ModuleOutput,
    Runtime,
    RuntimeError,
    Signal,
    StepLimit,
    Weight,
};

#[derive(Debug)]
struct TestModule {
    id: ModuleId,
}

impl TestModule {
    fn new(id: ModuleId) -> Self {
        Self { id }
    }
}

impl Module<()> for TestModule {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(&mut self, _signal: Signal<()>) -> Result<ModuleOutput<()>, ModuleError> {
        Ok(ModuleOutput::stop(()))
    }
}

#[derive(Debug)]
struct SourceFailure;

impl fmt::Display for SourceFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("source failure")
    }
}

impl Error for SourceFailure {}

fn assert_missing_module(error: RuntimeError, expected: ModuleId) {
    match error {
        RuntimeError::MissingModule { id } => assert_eq!(id, expected),
        RuntimeError::DuplicateModule { id } => panic!("unexpected duplicate module {id}"),
        RuntimeError::Routing(source) => panic!("unexpected routing error: {source}"),
        RuntimeError::Module { id, source } => panic!("unexpected module error {id}: {source}"),
        RuntimeError::StepLimitExceeded { limit, at } => {
            panic!("unexpected step limit {limit} at {at}")
        }
    }
}

#[test]
fn insert_module_rejects_duplicate_module_ids() -> Result<(), Box<dyn Error>> {
    // Given: a runtime with one registered module.
    let module = ModuleId::new("module")?;
    let mut runtime = Runtime::new(StepLimit::try_new(4)?);
    runtime.insert_module(TestModule::new(module.clone()))?;

    // When: another module with the same id is inserted.
    let error = runtime
        .insert_module(TestModule::new(module.clone()))
        .err()
        .ok_or("expected duplicate module error")?;

    // Then: the duplicate is rejected before it can shadow the first module.
    match error {
        RuntimeError::DuplicateModule { id } => assert_eq!(id, module),
        RuntimeError::MissingModule { id } => panic!("unexpected missing module {id}"),
        RuntimeError::Routing(source) => panic!("unexpected routing error: {source}"),
        RuntimeError::Module { id, source } => panic!("unexpected module error {id}: {source}"),
        RuntimeError::StepLimitExceeded { limit, at } => {
            panic!("unexpected step limit {limit} at {at}")
        }
    }
    Ok(())
}

#[test]
fn add_input_route_rejects_missing_target_module() -> Result<(), Box<dyn Error>> {
    // Given: an input route whose target module has not been registered.
    let input = InputId::new("input")?;
    let missing = ModuleId::new("missing")?;
    let mut runtime = Runtime::<()>::new(StepLimit::try_new(4)?);

    // When: the route is added.
    let error = runtime
        .add_input_route(input, missing.clone(), Weight::new(1), Allow)
        .err()
        .ok_or("expected missing target module error")?;

    // Then: route creation fails at the boundary.
    assert_missing_module(error, missing);
    Ok(())
}

#[test]
fn add_module_route_rejects_missing_source_module() -> Result<(), Box<dyn Error>> {
    // Given: a registered target and a misspelled source module id.
    let source = ModuleId::new("misspelled-source")?;
    let target = ModuleId::new("target")?;
    let mut runtime = Runtime::new(StepLimit::try_new(4)?);
    runtime.insert_module(TestModule::new(target.clone()))?;

    // When: a module route is created from the missing source.
    let error = runtime
        .add_module_route(source.clone(), target, Weight::new(1), Allow)
        .err()
        .ok_or("expected missing source module error")?;

    // Then: the typo is rejected instead of becoming a silent no-route edge.
    assert_missing_module(error, source);
    Ok(())
}

#[test]
fn module_error_preserves_source_chain() -> Result<(), Box<dyn Error>> {
    // Given: a module error wrapping a typed source error.
    let error = ModuleError::with_source("module failed", SourceFailure);

    // When: callers inspect the standard error chain.
    let source = error.source().ok_or("expected module error source")?;

    // Then: the underlying source remains available across the SDK boundary.
    assert_eq!(error.to_string(), "module failed");
    assert_eq!(source.to_string(), "source failure");
    assert_eq!(
        EndpointId::from(InputId::new("input")?).to_string(),
        "input:input"
    );
    Ok(())
}
