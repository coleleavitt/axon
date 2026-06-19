use std::error::Error;

use axon_memory::{Consolidator, Episode, EpisodicStore, MemoryStore, RecallQuery};

#[test]
fn episodic_store_recalls_episodes_by_partial_cue() -> Result<(), Box<dyn Error>> {
    // Given: an episodic store with two separated memories.
    let mut store = EpisodicStore::new();
    let first = store.encode(Episode::new("read Cargo manifest").with_tags(["tool", "cargo"]));
    store.encode(Episode::new("verify prediction mismatch").with_tags(["predict"]));

    // When: recall is queried with a partial lexical cue and tag.
    let results = store.recall(&RecallQuery::new("manifest").with_tags(["cargo"]));

    // Then: the matching memory is returned with its stable id.
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].episode().id(), first);
    assert_eq!(results[0].score(), 2);
    Ok(())
}

#[test]
fn consolidator_promotes_recurring_tags_into_schema_memory() -> Result<(), Box<dyn Error>> {
    // Given: three episodes with a recurring tag.
    let mut store = EpisodicStore::new();
    store.encode(Episode::new("read file").with_tags(["tool"]));
    store.encode(Episode::new("write file").with_tags(["tool"]));
    store.encode(Episode::new("compare expected outcome").with_tags(["predict"]));

    // When: the consolidator runs with a recurrence threshold of two.
    let schemas = Consolidator::new(2).consolidate(&store);

    // Then: only recurring structure is promoted.
    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0].tag(), "tool");
    assert_eq!(schemas[0].support(), 2);
    Ok(())
}

#[test]
fn recall_is_content_addressable_from_a_multi_word_cue() -> Result<(), Box<dyn Error>> {
    // Given: a memory whose words only partly overlap the eventual cue.
    let mut store = EpisodicStore::new();
    store.encode(Episode::new("read the project Cargo manifest"));
    store.encode(Episode::new("run the formatter"));

    // When: recall is queried with a partial, reordered, mixed-case cue.
    let results = store.recall(&RecallQuery::new("CARGO manifest missing"));

    // Then: the overlapping memory wins, scored by how many cue words matched.
    assert_eq!(results.len(), 1);
    assert!(results[0].episode().text().contains("Cargo manifest"));
    assert_eq!(results[0].score(), 2);
    Ok(())
}

#[test]
fn encode_separates_duplicate_memories() -> Result<(), Box<dyn Error>> {
    // Given: a store that already holds a memory.
    let mut store = EpisodicStore::new();
    let first = store.encode(Episode::new("read Cargo manifest"));

    // When: the same memory (modulo case and surrounding space) is re-encoded.
    let again = store.encode(Episode::new("  Read Cargo Manifest  "));

    // Then: it is not stored twice; the original id is returned.
    assert_eq!(first, again);
    assert_eq!(store.episodes().len(), 1);
    Ok(())
}

#[test]
fn recall_breaks_ties_toward_recent_memories() -> Result<(), Box<dyn Error>> {
    // Given: two equally-relevant memories encoded oldest-first.
    let mut store = EpisodicStore::new();
    store.encode(Episode::new("alpha task"));
    store.encode(Episode::new("beta task"));

    // When: a cue matches both equally.
    let results = store.recall(&RecallQuery::new("task"));

    // Then: both return, most recent first.
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].episode().text(), "beta task");
    assert_eq!(results[1].episode().text(), "alpha task");
    Ok(())
}
