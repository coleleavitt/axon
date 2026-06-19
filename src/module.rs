use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;

use crate::id::ModuleId;
use crate::signal::Signal;

pub trait Module<P>: fmt::Debug {
    fn id(&self) -> &ModuleId;

    fn handle(&mut self, signal: Signal<P>) -> Result<ModuleOutput<P>, ModuleError>;
}

pub struct FnModule<P, F> {
    id: ModuleId,
    handle: F,
    payload: PhantomData<fn(P) -> P>,
}

impl<P, F> FnModule<P, F> {
    pub fn new(id: ModuleId, handle: F) -> Self {
        Self {
            id,
            handle,
            payload: PhantomData,
        }
    }
}

impl<P, F> fmt::Debug for FnModule<P, F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FnModule")
            .field("id", &self.id)
            .finish()
    }
}

impl<P, F> Module<P> for FnModule<P, F>
where
    F: FnMut(Signal<P>) -> Result<ModuleOutput<P>, ModuleError>,
{
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn handle(&mut self, signal: Signal<P>) -> Result<ModuleOutput<P>, ModuleError> {
        (self.handle)(signal)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleOutput<P> {
    Emit(Signal<P>),
    Stop(Signal<P>),
    Drop,
}

impl<P> ModuleOutput<P> {
    pub fn emit(payload: P) -> Self {
        Self::Emit(Signal::new(payload))
    }

    pub const fn emit_signal(signal: Signal<P>) -> Self {
        Self::Emit(signal)
    }

    pub fn stop(payload: P) -> Self {
        Self::Stop(Signal::new(payload))
    }

    pub const fn stop_signal(signal: Signal<P>) -> Self {
        Self::Stop(signal)
    }

    pub const fn drop() -> Self {
        Self::Drop
    }
}

#[derive(Debug)]
pub struct ModuleError {
    message: Cow<'static, str>,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl ModuleError {
    pub fn new(message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(
        message: impl Into<Cow<'static, str>>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ModuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message())
    }
}

impl Error for ModuleError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}
