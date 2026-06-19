use std::error::Error;

use axon_core::{
    AsyncModule,
    AsyncRuntime,
    BoxFuture,
    InputId,
    ModuleError,
    ModuleId,
    ModuleOutput,
    RunEvent,
    RunStatus,
    Runtime,
    Signal,
    Weight,
};

/// A module that "awaits I/O" (here: yields once) before doubling the payload
/// and stopping, exercising the async dispatch path end to end.
#[derive(Debug)]
struct AsyncDoubler {
    id: ModuleId,
}

impl AsyncModule<u32> for AsyncDoubler {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(
        &mut self,
        signal: Signal<u32>,
    ) -> BoxFuture<'_, Result<ModuleOutput<u32>, ModuleError>> {
        Box::pin(async move {
            tokio::task::yield_now().await;
            Ok(ModuleOutput::stop(signal.into_payload().saturating_mul(2)))
        })
    }
}

#[tokio::test]
async fn async_runtime_awaits_a_module_and_routes_to_a_stop() -> Result<(), Box<dyn Error>> {
    // Given: an async runtime with one awaiting module.
    let input = InputId::new("input")?;
    let worker = ModuleId::new("doubler")?;
    let mut runtime = AsyncRuntime::default();
    runtime.insert_module(AsyncDoubler { id: worker.clone() })?;
    runtime.add_input_route(input.clone(), worker, Weight::new(10), axon_core::Allow)?;

    // When: the run is awaited.
    let report = runtime.run_async(input, Signal::new(21u32)).await?;

    // Then: the awaited module's output is what stopped the run.
    match report.into_status() {
        RunStatus::Stopped(signal) => assert_eq!(*signal.payload(), 42),
        other => panic!("expected stopped run, got {other:?}"),
    }
    Ok(())
}

#[test]
fn run_observed_streams_one_event_per_transition() -> Result<(), Box<dyn Error>> {
    // Given: a two-module pipeline on the synchronous runtime.
    let input = InputId::new("input")?;
    let first = ModuleId::new("first")?;
    let second = ModuleId::new("second")?;
    let mut runtime = Runtime::default();
    runtime.insert_module(axon_core::FnModule::new(
        first.clone(),
        |signal: Signal<u8>| Ok(ModuleOutput::emit(signal.into_payload())),
    ))?;
    runtime.insert_module(axon_core::FnModule::new(
        second.clone(),
        |signal: Signal<u8>| Ok(ModuleOutput::stop(signal.into_payload())),
    ))?;
    runtime.add_input_route(
        input.clone(),
        first.clone(),
        Weight::new(10),
        axon_core::Allow,
    )?;
    runtime.add_module_route(first, second, Weight::new(10), axon_core::Allow)?;

    // When: the run is observed.
    let mut events = Vec::new();
    runtime.run_observed(input, Signal::new(7u8), &mut |event| {
        events.push(event.clone())
    })?;

    // Then: the event stream mirrors the trace: enter, emit, enter, stop.
    assert_eq!(events.len(), 4);
    assert!(matches!(events[0], RunEvent::Entered { .. }));
    assert!(matches!(events[1], RunEvent::Emitted { .. }));
    assert!(matches!(events[3], RunEvent::Stopped { .. }));
    Ok(())
}
