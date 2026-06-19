use std::convert::Infallible;
use std::error::Error;

use axon_core::{Allow, AsyncRuntime, InputId, ModuleId, RunStatus, Signal, StepLimit, Weight};
use axon_tools::{AsyncToolModule, ToolSignal};

#[tokio::test]
async fn async_tool_runs_through_the_async_runtime() -> Result<(), Box<dyn Error>> {
    // Given: an async tool wired into the async runtime. The closure returns a
    // future, modelling a tool that awaits real I/O before producing output.
    let tool = ModuleId::new("tool")?;
    let mut runtime: AsyncRuntime<ToolSignal<String, String>> =
        AsyncRuntime::new(StepLimit::default());
    runtime.insert_module(AsyncToolModule::new(
        tool.clone(),
        |input: String| async move { Ok::<_, Infallible>(format!("did {input}")) },
    ))?;
    runtime.add_input_route(InputId::new("in")?, tool, Weight::new(1), Allow)?;

    // When: a Call signal is driven through the async dispatch path.
    let report = runtime
        .run_async(
            InputId::new("in")?,
            Signal::new(ToolSignal::Call("ping".to_owned())),
        )
        .await?;

    // Then: the awaited tool output flows back out as a Result signal — async
    // tools integrate with AsyncRuntime exactly like sync tools do with Runtime.
    match report.into_status() {
        RunStatus::NoRoute { signal, .. } => {
            assert_eq!(
                signal.into_payload(),
                ToolSignal::Result("did ping".to_owned())
            );
        }
        other => panic!("expected the result to fall through with no onward route, got {other:?}"),
    }
    Ok(())
}
