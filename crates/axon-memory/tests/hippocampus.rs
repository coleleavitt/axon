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
    assert_eq!(results[0].relevance(), 2);
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
    assert_eq!(results[0].relevance(), 2);
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
    // And: the more recent memory carries the higher recency weight.
    assert!(results[0].recency() > results[1].recency());
    Ok(())
}

#[test]
fn importance_outranks_a_more_recent_but_mundane_memory() -> Result<(), Box<dyn Error>> {
    // Given: an old-but-important memory and a newer, mundane one of equal
    // lexical relevance to the cue.
    let mut store = EpisodicStore::new();
    store.encode(Episode::new("deploy production").with_importance(5.0));
    store.encode(Episode::new("deploy staging"));

    // When: a cue matches both equally on content.
    let results = store.recall(&RecallQuery::new("deploy"));

    // Then: importance overrides the recency advantage of the newer memory — the
    // salient one ranks first even though it is older.
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].episode().text(), "deploy production");
    assert!(results[0].score() > results[1].score());
    Ok(())
}

#[test]
fn relevance_recency_and_importance_compose_into_the_score() -> Result<(), Box<dyn Error>> {
    // Given: a single, newest memory matching one cue word.
    let mut store = EpisodicStore::new();
    store.encode(Episode::new("run the tests").with_importance(2.0));

    // When: recalled by a partial cue.
    let results = store.recall(&RecallQuery::new("tests"));

    // Then: score is the product of its three components.
    assert_eq!(results.len(), 1);
    let result = results[0];
    let expected = result.relevance() as f32 * result.recency() * result.importance();
    assert!((result.score() - expected).abs() < f32::EPSILON);
    // The newest memory has full recency, and importance is carried through.
    assert!((result.recency() - 1.0).abs() < f32::EPSILON);
    assert!((result.importance() - 2.0).abs() < f32::EPSILON);
    Ok(())
}
