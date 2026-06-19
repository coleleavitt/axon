use std::error::Error;
use std::fmt;

use crate::id::{EndpointId, ModuleId};
use crate::limit::StepLimit;
use crate::module::ModuleError;
use crate::routing::RoutingError;

#[derive(Debug)]
pub enum RuntimeError {
    DuplicateModule { id: ModuleId },
    MissingModule { id: ModuleId },
    Routing(RoutingError),
    Module { id: ModuleId, source: ModuleError },
    StepLimitExceeded { limit: StepLimit, at: EndpointId },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateModule { id } => write!(formatter, "module {id} is already registered"),
            Self::MissingModule { id } => write!(formatter, "module {id} is not registered"),
            Self::Routing(source) => write!(formatter, "routing failed: {source}"),
            Self::Module { id, source } => write!(formatter, "module {id} failed: {source}"),
            Self::StepLimitExceeded { limit, at } => {
                write!(formatter, "step limit {limit} exceeded at {at}")
            }
        }
    }
}

impl Error for RuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DuplicateModule { id: _ }
            | Self::MissingModule { id: _ }
            | Self::StepLimitExceeded { limit: _, at: _ } => None,
            Self::Routing(source) => Some(source),
            Self::Module { id: _, source } => Some(source),
        }
    }
}

impl From<RoutingError> for RuntimeError {
    fn from(source: RoutingError) -> Self {
        Self::Routing(source)
    }
}
