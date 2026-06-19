use std::error::Error;

use axon_core::{
    Allow,
    AsyncModule,
    AsyncRuntime,
    BoxFuture,
    InputId,
    ModuleError,
    ModuleId,
    ModuleOutput,
    RunStatus,
    Signal,
    StepLimit,
    StopToken,
    Weight,
};

/// A module whose future never resolves — a stand-in for a hung tool (a shell
/// command or network call that wedges).
#[derive(Debug)]
struct HungModule {
    id: ModuleId,
}

impl AsyncModule<u32> for HungModule {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(
        &mut self,
        _signal: Signal<u32>,
    ) -> BoxFuture<'_, Result<ModuleOutput<u32>, ModuleError>> {
        Box::pin(std::future::pending())
    }
}

#[tokio::test]
async fn stop_cancels_an_in_flight_async_module() -> Result<(), Box<dyn Error>> {
    // Given: an async runtime whose only module hangs forever, guarded by a stop
    // token.
    let stop = StopToken::new();
    let mut runtime: AsyncRuntime<u32> =
        AsyncRuntime::new(StepLimit::default()).with_stop_token(stop.clone());
    runtime.insert_module(HungModule {
        id: ModuleId::new("hang")?,
    })?;
    runtime.add_input_route(
        InputId::new("in")?,
        ModuleId::new("hang")?,
        Weight::new(1),
        Allow,
    )?;

    // When: the run starts awaiting the hung module and, concurrently, the token
    // is stopped.
    let trigger = stop.clone();
    let run = runtime.run_async(InputId::new("in")?, Signal::new(1));
    let stopper = async move {
        // Let the run reach and await the hung module first.
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        trigger.stop();
    };
    let (report, ()) = tokio::join!(run, stopper);

    // Then: the run is cancelled mid-future and halts, rather than hanging — the
    // in-flight future is dropped, not waited on.
    assert!(matches!(report?.status(), RunStatus::Halted { .. }));
    Ok(())
}
