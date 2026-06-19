#![cfg(feature = "serde")]

use std::error::Error;

use axon_memory::{Episode, EpisodicStore, MemoryStore};

#[test]
fn episodic_store_round_trips_through_json() -> Result<(), Box<dyn Error>> {
    // Given: a store with two distinct memories.
    let mut store = EpisodicStore::new();
    store.encode(Episode::new("read manifest").with_tags(["tool"]));
    store.encode(Episode::new("verify outcome").with_tags(["predict"]));

    // When: it is serialized and restored.
    let json = serde_json::to_string(&store)?;
    let mut restored: EpisodicStore = serde_json::from_str(&json)?;

    // Then: the episodes survive verbatim and the id sequence continues, so a
    // new memory does not collide with a restored one.
    assert_eq!(restored.episodes(), store.episodes());
    let next = restored.encode(Episode::new("new memory"));
    assert_eq!(next.get(), 3);
    Ok(())
}
