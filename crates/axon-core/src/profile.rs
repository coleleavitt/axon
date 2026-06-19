use crate::edge::EdgeId;

/// A named, swappable set of per-edge weight biases — a *routing profile*.
///
/// Applied to a [`RoutingTable`](crate::RoutingTable), it shifts which routes win
/// without rewiring the graph, so one fixed structure yields many transient,
/// context-dependent routing configurations (Kuramoto/chimera metastability).
/// A caller typically derives a profile from the active `Mode` and swaps it in
/// when the mode changes.
#[derive(Debug, Clone, Default)]
pub struct RoutingProfile {
    biases: Vec<(EdgeId, i16)>,
}

impl RoutingProfile {
    pub const fn new() -> Self {
        Self { biases: Vec::new() }
    }

    /// Bias `edge`'s effective weight by `delta` under this profile (builder).
    #[must_use]
    pub fn bias(mut self, edge: EdgeId, delta: i16) -> Self {
        self.biases.push((edge, delta));
        self
    }

    /// The bias this profile assigns to `edge` (0 if unlisted).
    pub fn get(&self, edge: &EdgeId) -> i16 {
        self.biases
            .iter()
            .find(|(candidate, _)| candidate == edge)
            .map_or(0, |(_, delta)| *delta)
    }
}
