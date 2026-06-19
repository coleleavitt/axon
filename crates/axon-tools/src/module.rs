use std::error::Error;
use std::fmt;

use axon_core::{Module, ModuleError, ModuleId, ModuleOutput, Signal};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSignal<I, O> {
    Call(I),
    Result(O),
}

pub struct ToolModule<I, O, F> {
    id: ModuleId,
    call: F,
    marker: std::marker::PhantomData<fn(I) -> O>,
}

impl<I, O, F> ToolModule<I, O, F> {
    pub fn new(id: ModuleId, call: F) -> Self {
        Self {
            id,
            call,
            marker: std::marker::PhantomData,
        }
    }
}

impl<I, O, F> fmt::Debug for ToolModule<I, O, F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolModule")
            .field("id", &self.id)
            .finish()
    }
}

impl<I, O, F, E> Module<ToolSignal<I, O>> for ToolModule<I, O, F>
where
    F: FnMut(I) -> Result<O, E>,
    E: Error + Send + Sync + 'static,
{
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(
        &mut self,
        signal: Signal<ToolSignal<I, O>>,
    ) -> Result<ModuleOutput<ToolSignal<I, O>>, ModuleError> {
        match signal.into_payload() {
            ToolSignal::Call(input) => (self.call)(input)
                .map(ToolSignal::Result)
                .map(ModuleOutput::emit)
                .map_err(|source| ModuleError::with_source("tool module failed", source)),
            ToolSignal::Result(_) => Ok(ModuleOutput::drop()),
        }
    }
}
