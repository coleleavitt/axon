use std::error::Error;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;

use axon_core::{AsyncModule, BoxFuture, ModuleError, ModuleId, ModuleOutput, Signal};

use crate::ToolSignal;

/// Async counterpart to [`Tool`](crate::Tool): `call` returns a future, so a tool
/// that does real I/O — a shell command, a network fetch, a model call — can
/// await without blocking the single-threaded runtime, and several such tools
/// can make progress concurrently. Mirrors the sync trait for the
/// [`AsyncRuntime`](axon_core::AsyncRuntime) dispatch path.
pub trait AsyncTool {
    type Input;
    type Output;
    type Error: Error + Send + Sync + 'static;

    fn call(&mut self, input: Self::Input) -> BoxFuture<'_, Result<Self::Output, Self::Error>>;
}

/// Adapts an async tool closure into an [`AsyncModule`] over [`ToolSignal`] — the
/// async analogue of [`ToolModule`](crate::ToolModule). A `Call` awaits the tool
/// and emits a `Result`; a `Result` arriving here is terminal and dropped.
pub struct AsyncToolModule<I, O, F> {
    id: ModuleId,
    call: F,
    marker: PhantomData<fn(I) -> O>,
}

impl<I, O, F> AsyncToolModule<I, O, F> {
    pub fn new(id: ModuleId, call: F) -> Self {
        Self {
            id,
            call,
            marker: PhantomData,
        }
    }
}

impl<I, O, F> fmt::Debug for AsyncToolModule<I, O, F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AsyncToolModule")
            .field("id", &self.id)
            .finish()
    }
}

impl<I, O, F, Fut, E> AsyncModule<ToolSignal<I, O>> for AsyncToolModule<I, O, F>
where
    F: FnMut(I) -> Fut,
    Fut: Future<Output = Result<O, E>>,
    E: Error + Send + Sync + 'static,
    I: 'static,
    O: 'static,
{
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(
        &mut self,
        signal: Signal<ToolSignal<I, O>>,
    ) -> BoxFuture<'_, Result<ModuleOutput<ToolSignal<I, O>>, ModuleError>> {
        Box::pin(async move {
            match signal.into_payload() {
                ToolSignal::Call(input) => (self.call)(input)
                    .await
                    .map(ToolSignal::Result)
                    .map(ModuleOutput::emit)
                    .map_err(|source| ModuleError::with_source("async tool module failed", source)),
                ToolSignal::Result(_) => Ok(ModuleOutput::drop()),
            }
        })
    }
}
