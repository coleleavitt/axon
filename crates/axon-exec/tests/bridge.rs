use std::error::Error;

use axon_core::Priority;
use axon_exec::encode_salient;
use axon_memory::{EpisodicStore, MemoryStore, RecallQuery};
use axon_workspace::{Broadcast, Workspace};

#[test]
fn salient_broadcasts_cross_into_long_term_memory() -> Result<(), Box<dyn Error>> {
    // Given: a workspace holding an urgent alert and a routine observation.
    let mut workspace = Workspace::new(8)?;
    assert!(workspace.broadcast(Broadcast::alert("disk full")));
    assert!(workspace.broadcast(Broadcast::observation("tick")));
    let mut memory = EpisodicStore::new();

    // When: the encode bridge transfers items at or above salience 5.
    let moved = encode_salient(&workspace, &mut memory, Priority::new(5));

    // Then: only the salient alert is consolidated to long-term memory, and it is
    // recallable there — the routine observation does not cross the bridge.
    assert_eq!(moved, 1);
    assert_eq!(memory.episodes().len(), 1);
    assert_eq!(memory.recall(&RecallQuery::new("disk")).len(), 1);
    Ok(())
}
