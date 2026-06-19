use std::fmt;

use crate::id::{EndpointId, ModuleId};

/// The identity of a routing edge — a `(from, to)` pair.
///
/// This is the stable key plasticity addresses: reinforcement, decay, weight
/// snapshots, and the [`RunEvent::Reinforced`](crate::RunEvent::Reinforced)
/// event all name an edge by this id, independent of the route's
/// (non-serializable) gate. That decoupling is what lets learned weights be
/// persisted and restored while the gates are reconstructed from code.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EdgeId {
    from: EndpointId,
    to: ModuleId,
}

impl EdgeId {
    pub const fn new(from: EndpointId, to: ModuleId) -> Self {
        Self { from, to }
    }

    pub const fn from(&self) -> &EndpointId {
        &self.from
    }

    pub const fn to(&self) -> &ModuleId {
        &self.to
    }
}

impl fmt::Display for EdgeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} -> {}", self.from, self.to)
    }
}
