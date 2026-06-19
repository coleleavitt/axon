use std::fmt;

use crate::gate::{Allow, Gate};
use crate::id::{EndpointId, ModuleId};
use crate::signal::Signal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Weight(i16);

impl Weight {
    pub const fn new(value: i16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i16 {
        self.0
    }
}

impl fmt::Display for Weight {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

pub struct Route<P> {
    from: EndpointId,
    to: ModuleId,
    weight: Weight,
    gate: Box<dyn Gate<P> + Send + Sync + 'static>,
}

impl<P> Route<P> {
    pub fn new<G>(from: EndpointId, to: ModuleId, weight: Weight, gate: G) -> Self
    where
        G: Gate<P> + Send + Sync + 'static,
    {
        Self {
            from,
            to,
            weight,
            gate: Box::new(gate),
        }
    }

    pub fn open(from: EndpointId, to: ModuleId, weight: Weight) -> Self {
        Self::new(from, to, weight, Allow)
    }

    pub const fn from(&self) -> &EndpointId {
        &self.from
    }

    pub const fn to(&self) -> &ModuleId {
        &self.to
    }

    pub const fn weight(&self) -> Weight {
        self.weight
    }

    pub fn admits(&self, signal: &Signal<P>) -> bool {
        self.gate.admits(signal)
    }
}

impl<P> fmt::Debug for Route<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Route")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("weight", &self.weight)
            .field("gate", &self.gate)
            .finish()
    }
}
