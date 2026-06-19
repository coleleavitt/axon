use std::error::Error;

use axon_memory::{
    Consolidator,
    Episode,
    EpisodicStore,
    Fact,
    HashEmbedder,
    MemoryStore,
    ProceduralStore,
    Procedure,
    RecallQuery,
    SchemaStore,
    SemanticStore,
};

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
fn reflection_abstracts_recurring_content_into_insights() -> Result<(), Box<dyn Error>> {
    // Given: episodes that share recurring content words.
    let mut store = EpisodicStore::new();
    store.encode(Episode::new("deploy the billing service"));
    store.encode(Episode::new("deploy the auth service"));
    store.encode(Episode::new("restart the cache"));

    // When: the consolidator reflects with a recurrence threshold of two.
    let insights = Consolidator::new(2).reflect(&store);

    // Then: recurring themes are abstracted with their support, while a one-off
    // token is not promoted.
    assert!(
        insights
            .iter()
            .any(|insight| insight.theme() == "deploy" && insight.support() == 2)
    );
    assert!(
        insights
            .iter()
            .any(|insight| insight.theme() == "service" && insight.support() == 2)
    );
    assert!(insights.iter().all(|insight| insight.theme() != "cache"));
    Ok(())
}

#[test]
fn schema_store_is_readable_after_consolidation() -> Result<(), Box<dyn Error>> {
    // Given: episodes consolidated into the slow schema store.
    let mut store = EpisodicStore::new();
    store.encode(Episode::new("read file").with_tags(["tool"]));
    store.encode(Episode::new("write file").with_tags(["tool"]));
    store.encode(Episode::new("predict outcome").with_tags(["predict"]));
    let mut schemas = SchemaStore::new();
    schemas.install(Consolidator::new(2).consolidate(&store));

    // When/Then: the consolidated schema is recallable (the slow store is no
    // longer write-only), while a below-threshold tag is absent.
    let tool = schemas.recall("tool");
    assert_eq!(tool.len(), 1);
    assert_eq!(tool[0].support(), 2);
    assert!(schemas.recall("predict").is_empty());
    Ok(())
}

#[test]
fn semantic_store_recalls_facts_by_subject() -> Result<(), Box<dyn Error>> {
    // Given: facts about two subjects, with one exact duplicate.
    let mut store = SemanticStore::new();
    store.assert(Fact::new("axon", "is", "a routing core"));
    store.assert(Fact::new("axon", "uses", "edition 2024"));
    store.assert(Fact::new("rust", "has", "ownership"));
    store.assert(Fact::new("axon", "is", "a routing core"));

    // Then: facts are recalled by subject and the duplicate was ignored.
    assert_eq!(store.about("axon").len(), 2);
    assert_eq!(store.about("rust").len(), 1);
    assert_eq!(store.facts().len(), 3);
    Ok(())
}

#[test]
fn procedural_store_recalls_a_known_how_to_by_goal() -> Result<(), Box<dyn Error>> {
    // Given: two learned procedures.
    let mut store = ProceduralStore::new();
    store.learn(Procedure::new(
        "run the tests",
        ["cargo build", "cargo test"],
    ));
    store.learn(Procedure::new("format the code", ["cargo fmt"]));

    // Then: an exact goal returns its steps...
    let Some(tests) = store.get("run the tests") else {
        panic!("expected a stored procedure");
    };
    assert_eq!(tests.steps().len(), 2);

    // ...a partial goal cue recalls the best match...
    let recalled = store.recall("run tests please");
    assert_eq!(recalled[0].goal(), "run the tests");

    // ...and re-learning the same goal replaces the procedure.
    store.learn(Procedure::new("run the tests", ["cargo nextest run"]));
    let Some(updated) = store.get("run the tests") else {
        panic!("expected the replaced procedure");
    };
    assert_eq!(updated.steps().len(), 1);
    Ok(())
}

#[test]
fn encode_collapses_near_duplicate_memories() -> Result<(), Box<dyn Error>> {
    // Given: a stored memory.
    let mut store = EpisodicStore::new();
    let first = store.encode(Episode::new("fix the auth bug"));

    // When: a reworded near-duplicate (token similarity 0.8) is encoded.
    let again = store.encode(Episode::new("fix the auth bug now"));

    // Then: it is orthogonalized to the existing memory, not stored separately.
    assert_eq!(first, again);
    assert_eq!(store.episodes().len(), 1);

    // And: a genuinely different memory is still separated.
    store.encode(Episode::new("brew some coffee"));
    assert_eq!(store.episodes().len(), 2);
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
fn similarity_recall_ranks_relevant_memory_and_drops_unrelated() -> Result<(), Box<dyn Error>> {
    // Given: one on-topic memory and one unrelated one.
    let mut store = EpisodicStore::new();
    store.encode(Episode::new("read the cargo manifest file"));
    store.encode(Episode::new("brew a fresh cup of coffee"));

    // When: recalled by embedding similarity to a paraphrased cue.
    let embedder = HashEmbedder::default();
    let hits = store.rank_by_similarity("inspect the cargo manifest", &embedder);

    // Then: the on-topic memory is the top hit with positive similarity, and the
    // unrelated one (no shared tokens) is filtered out entirely.
    assert_eq!(hits.len(), 1);
    assert!(hits[0].episode().text().contains("cargo manifest"));
    assert!(hits[0].similarity() > 0.0);
    Ok(())
}

#[test]
fn bounded_store_forgets_the_least_valuable_memory() -> Result<(), Box<dyn Error>> {
    // Given: a store bounded to two episodes.
    let mut store = EpisodicStore::new().with_capacity(2);
    store.encode(Episode::new("trivia").with_importance(0.1));
    store.encode(Episode::new("critical incident").with_importance(10.0));

    // When: a third memory is encoded, exceeding capacity.
    store.encode(Episode::new("note three"));

    // Then: the lowest importance × recency memory (mundane and now stale) is
    // forgotten, while the important one is retained.
    assert_eq!(store.episodes().len(), 2);
    assert!(
        store
            .episodes()
            .iter()
            .all(|episode| episode.text() != "trivia")
    );
    assert!(
        store
            .episodes()
            .iter()
            .any(|episode| episode.text().contains("critical"))
    );
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
