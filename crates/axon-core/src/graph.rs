use std::collections::{BTreeSet, VecDeque};

use crate::id::{EndpointId, ModuleId};

/// A read-only view of the routing topology — the module graph *is* the
/// connectome. Derived from the registered routes, it answers the structural
/// questions the flat routing table cannot: degree, reachability, and which
/// module is the hub (where failures concentrate and breakers/monitoring matter
/// most).
#[derive(Debug, Clone, Default)]
pub struct ModuleGraph {
    edges: Vec<(EndpointId, ModuleId)>,
}

impl ModuleGraph {
    pub const fn new() -> Self {
        Self { edges: Vec::new() }
    }

    /// Record a directed `from -> to` edge.
    pub fn insert(&mut self, from: EndpointId, to: ModuleId) {
        self.edges.push((from, to));
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Every distinct endpoint appearing as a source or target.
    pub fn nodes(&self) -> BTreeSet<EndpointId> {
        let mut nodes = BTreeSet::new();
        for (from, to) in &self.edges {
            nodes.insert(from.clone());
            nodes.insert(EndpointId::Module(to.clone()));
        }
        nodes
    }

    pub fn node_count(&self) -> usize {
        self.nodes().len()
    }

    /// Number of edges leaving `from`.
    pub fn out_degree(&self, from: &EndpointId) -> usize {
        self.edges
            .iter()
            .filter(|(source, _)| source == from)
            .count()
    }

    /// Number of edges arriving at module `to`.
    pub fn in_degree(&self, to: &ModuleId) -> usize {
        self.edges.iter().filter(|(_, target)| target == to).count()
    }

    /// The modules directly reachable in one hop from `from`.
    pub fn neighbors<'a>(&'a self, from: &'a EndpointId) -> impl Iterator<Item = &'a ModuleId> {
        self.edges
            .iter()
            .filter(move |(source, _)| source == from)
            .map(|(_, target)| target)
    }

    /// Every module reachable from `start` by following routes (breadth-first).
    pub fn reachable_from(&self, start: &EndpointId) -> BTreeSet<ModuleId> {
        let mut seen = BTreeSet::new();
        let mut frontier = VecDeque::new();
        frontier.push_back(start.clone());
        while let Some(node) = frontier.pop_front() {
            for next in self.neighbors(&node) {
                if seen.insert(next.clone()) {
                    frontier.push_back(EndpointId::Module(next.clone()));
                }
            }
        }
        seen
    }

    /// The module with the highest total degree (in + out) — the connectome hub
    /// where robustness measures pay off most. `None` if there are no modules.
    pub fn hub(&self) -> Option<ModuleId> {
        self.nodes()
            .into_iter()
            .filter_map(|node| match node {
                EndpointId::Module(id) => Some(id),
                EndpointId::Input(_) => None,
            })
            .max_by_key(|id| self.in_degree(id) + self.out_degree(&EndpointId::Module(id.clone())))
    }
}
