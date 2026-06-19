use axon_core::Priority;
use axon_memory::{Episode, EpisodicStore, MemoryStore};
use axon_workspace::Workspace;

/// The working-memory → long-term encode bridge.
///
/// Salient broadcasts would otherwise age out of the bounded workspace and be
/// lost. This transfers every broadcast whose salience is at or above
/// `min_salience` into episodic memory — tagged `"workspace"` and weighted by
/// its salience — so important conscious content is consolidated to long-term
/// store before eviction. Returns how many were encoded.
pub fn encode_salient(
    workspace: &Workspace,
    memory: &mut EpisodicStore,
    min_salience: Priority,
) -> usize {
    let mut encoded = 0;
    for broadcast in workspace.broadcasts() {
        if broadcast.salience() >= min_salience {
            memory.encode(
                Episode::new(broadcast.text())
                    .with_tags(["workspace"])
                    .with_importance(f32::from(broadcast.salience().get())),
            );
            encoded += 1;
        }
    }
    encoded
}
