use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemoryId(u64);

impl MemoryId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

// `Episode` carries an `f32` importance, so it is `PartialEq` but not `Eq`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Episode {
    id: MemoryId,
    text: String,
    tags: Vec<String>,
    importance: f32,
}

impl Episode {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: MemoryId::default(),
            text: text.into(),
            tags: Vec::new(),
            importance: 1.0,
        }
    }

    pub fn with_tags<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.tags = collect_tags(tags);
        self
    }

    /// Set the salience/importance weight (default `1.0`). Typically derived
    /// from `Modulators::attention()` at encode time; recall ranks a more
    /// important memory above an equally-relevant but mundane one.
    #[must_use]
    pub fn with_importance(mut self, importance: f32) -> Self {
        self.importance = importance;
        self
    }

    pub const fn id(&self) -> MemoryId {
        self.id
    }

    pub const fn importance(&self) -> f32 {
        self.importance
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    fn assign(&mut self, id: MemoryId) {
        self.id = id;
    }
}

pub trait MemoryStore {
    fn encode(&mut self, episode: Episode) -> MemoryId;
    fn recall(&self, query: &RecallQuery) -> Vec<RecallResult<'_>>;
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EpisodicStore {
    episodes: Vec<Episode>,
    next_id: u64,
    capacity: Option<usize>,
    dedup_threshold: f32,
}

impl EpisodicStore {
    pub const fn new() -> Self {
        Self {
            episodes: Vec::new(),
            next_id: 1,
            capacity: None,
            // Collapse memories whose token sets are >= 80% similar.
            dedup_threshold: 0.8,
        }
    }

    /// Set the near-duplicate orthogonalization threshold in `[0.0, 1.0]`: on
    /// encode, a new memory whose token (Jaccard) similarity to an existing one
    /// meets this threshold is treated as the same memory (pattern separation).
    /// `1.0` requires exact token-set equality.
    #[must_use]
    pub const fn with_dedup_threshold(mut self, threshold: f32) -> Self {
        self.dedup_threshold = threshold;
        self
    }

    /// Bound the store to at most `capacity` episodes. Once full, each encode
    /// evicts the least valuable memory — lowest importance × recency — so the
    /// store actively forgets mundane, stale experience instead of growing
    /// without bound (NREM-style active forgetting).
    #[must_use]
    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.capacity = Some(capacity);
        self.enforce_capacity();
        self
    }

    pub fn episodes(&self) -> &[Episode] {
        &self.episodes
    }

    pub const fn capacity(&self) -> Option<usize> {
        self.capacity
    }

    /// Recall by embedding similarity rather than exact lexical overlap: rank
    /// episodes by cosine(query, episode) scaled by recency and importance.
    ///
    /// This is the content-addressable / pattern-completion path — with a real
    /// (`axon-provider`) embedder it recalls from paraphrased cues that the
    /// lexical [`recall`](MemoryStore::recall) path would miss. With the
    /// deterministic [`HashEmbedder`] it degrades gracefully to graded
    /// token-overlap similarity (still better than yes/no `contains`).
    pub fn rank_by_similarity<'a>(
        &'a self,
        query: &str,
        embedder: &dyn Embedder,
    ) -> Vec<SimilarHit<'a>> {
        let cue = embedder.embed(query);
        let newest = self.next_id.saturating_sub(1);
        let mut hits: Vec<SimilarHit<'a>> = self
            .episodes
            .iter()
            .filter_map(|episode| {
                let similarity = cosine(&cue, &embedder.embed(episode.text()));
                (similarity > 0.0).then_some(SimilarHit {
                    episode,
                    similarity,
                    recency: recency_weight(episode.id, newest),
                })
            })
            .collect();
        hits.sort_by(|left, right| {
            right
                .score()
                .partial_cmp(&left.score())
                .unwrap_or(Ordering::Equal)
                .then_with(|| right.episode.id.cmp(&left.episode.id))
        });
        hits
    }

    /// Evict lowest-value episodes until the store is within capacity.
    fn enforce_capacity(&mut self) {
        let Some(capacity) = self.capacity else {
            return;
        };
        while self.episodes.len() > capacity {
            let newest = self.next_id.saturating_sub(1);
            let victim = self
                .episodes
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| {
                    retention(left, newest)
                        .partial_cmp(&retention(right, newest))
                        .unwrap_or(Ordering::Equal)
                })
                .map(|(index, _)| index);
            match victim {
                Some(index) => {
                    self.episodes.remove(index);
                }
                None => break,
            }
        }
    }
}

impl Default for EpisodicStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStore for EpisodicStore {
    fn encode(&mut self, mut episode: Episode) -> MemoryId {
        // Pattern separation (dentate gyrus): never store a memory that is not
        // separable from one already held. A near-duplicate (token similarity at
        // or above the threshold) returns the existing id instead of
        // accumulating redundant, overlapping traces.
        if let Some(existing) = self
            .episodes
            .iter()
            .find(|stored| jaccard(stored.text(), episode.text()) >= self.dedup_threshold)
        {
            return existing.id();
        }
        let id = MemoryId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        episode.assign(id);
        self.episodes.push(episode);
        self.enforce_capacity();
        id
    }

    fn recall(&self, query: &RecallQuery) -> Vec<RecallResult<'_>> {
        let newest = self.next_id.saturating_sub(1);
        let mut results: Vec<_> = self
            .episodes
            .iter()
            .filter_map(|episode| RecallResult::from_episode(episode, query, newest))
            .collect();
        // Rank by the combined relevance × recency × importance score, breaking
        // ties toward more recent memories (higher id).
        results.sort_by(|left, right| {
            right
                .score()
                .partial_cmp(&left.score())
                .unwrap_or(Ordering::Equal)
                .then_with(|| right.episode.id.cmp(&left.episode.id))
        });
        results
    }
}

/// Token (Jaccard) similarity in `[0.0, 1.0]` between two texts — the
/// orthogonalization measure applied on write. Two empty texts are identical.
fn jaccard(left: &str, right: &str) -> f32 {
    let left: BTreeSet<String> = tokenize(left).into_iter().collect();
    let right: BTreeSet<String> = tokenize(right).into_iter().collect();
    let union = left.union(&right).count();
    if union == 0 {
        return 1.0;
    }
    left.intersection(&right).count() as f32 / union as f32
}

/// Split text into lowercased alphanumeric tokens for content-addressable
/// matching from partial cues.
fn tokenize(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Count how many cue tokens are present in the episode text. More overlapping
/// words means a stronger partial-cue match, not just a yes/no `contains`.
fn lexical_overlap(query: &str, text: &str) -> u32 {
    let text_tokens = tokenize(text);
    tokenize(query)
        .into_iter()
        .filter(|cue| text_tokens.contains(cue))
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallQuery {
    text: String,
    tags: Vec<String>,
}

impl RecallQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tags: Vec::new(),
        }
    }

    pub fn with_tags<I, T>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.tags = collect_tags(tags);
        self
    }
}

fn collect_tags<I, T>(tags: I) -> Vec<String>
where
    I: IntoIterator<Item = T>,
    T: Into<String>,
{
    tags.into_iter().map(Into::into).collect()
}

#[derive(Debug, Clone, Copy)]
pub struct RecallResult<'a> {
    episode: &'a Episode,
    relevance: u32,
    recency: f32,
    importance: f32,
}

impl<'a> RecallResult<'a> {
    fn from_episode(episode: &'a Episode, query: &RecallQuery, newest: u64) -> Option<Self> {
        let text_score = lexical_overlap(&query.text, &episode.text);
        let tag_score = query
            .tags
            .iter()
            .filter(|tag| episode.tags.iter().any(|candidate| candidate == *tag))
            .count()
            .try_into()
            .unwrap_or(u32::MAX);
        let relevance = text_score.saturating_add(tag_score);
        if relevance == 0 {
            return None;
        }
        Some(Self {
            episode,
            relevance,
            recency: recency_weight(episode.id, newest),
            importance: episode.importance,
        })
    }

    pub const fn episode(&self) -> &Episode {
        self.episode
    }

    /// Content-match strength: lexical + tag overlap with the cue.
    pub const fn relevance(&self) -> u32 {
        self.relevance
    }

    /// Recency weight in `(0.0, 1.0]`; the newest memory weighs `1.0`.
    pub const fn recency(&self) -> f32 {
        self.recency
    }

    /// The episode's importance/salience weight.
    pub const fn importance(&self) -> f32 {
        self.importance
    }

    /// Combined retrieval score `relevance × recency × importance`: a memory must
    /// be on-topic, recent, *and* important to rank highly. This is the
    /// Generative-Agents weighting (relevance × recency-decay × importance).
    pub fn score(&self) -> f32 {
        self.relevance as f32 * self.recency * self.importance
    }
}

/// Recency weighting: the newest memory (`id == newest`) weighs `1.0` and older
/// ones decay as `1 / (1 + age)`, so recall favors fresh experience.
fn recency_weight(id: MemoryId, newest: u64) -> f32 {
    let age = newest.saturating_sub(id.get());
    1.0 / (1.0 + age as f32)
}

/// A memory's retention value — importance scaled by recency. The lowest-value
/// episode is the first forgotten when the store is over capacity.
fn retention(episode: &Episode, newest: u64) -> f32 {
    episode.importance * recency_weight(episode.id, newest)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Consolidator {
    threshold: usize,
}

impl Consolidator {
    pub const fn new(threshold: usize) -> Self {
        Self { threshold }
    }

    pub fn consolidate(&self, store: &EpisodicStore) -> Vec<SchemaMemory> {
        let mut counts = BTreeMap::<String, usize>::new();
        for episode in store.episodes() {
            for tag in episode.tags() {
                counts
                    .entry(tag.clone())
                    .and_modify(|support| *support = support.saturating_add(1))
                    .or_insert(1);
            }
        }
        counts
            .into_iter()
            .filter(|(_, support)| *support >= self.threshold)
            .map(|(tag, support)| SchemaMemory { tag, support })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SchemaMemory {
    tag: String,
    support: usize,
}

impl SchemaMemory {
    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub const fn support(&self) -> usize {
        self.support
    }
}

/// Maps text to a dense vector so memories can be recalled by *similarity*, not
/// just exact token overlap. Implement against a real model (e.g. `axon-provider`
/// behind the `openai` feature) for semantic recall; the built-in
/// [`HashEmbedder`] is a deterministic, dependency-free stand-in for tests and
/// offline use.
pub trait Embedder {
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// A deterministic, dependency-free embedder: a hashed bag-of-tokens. Each token
/// increments one of `dims` buckets (FNV-1a hashed), so texts sharing tokens get
/// similar vectors. It captures lexical overlap, not true semantics, but gives a
/// graded, length-normalized similarity and a working [`Embedder`] seam.
#[derive(Debug, Clone, Copy)]
pub struct HashEmbedder {
    dims: usize,
}

impl HashEmbedder {
    pub const fn new(dims: usize) -> Self {
        Self { dims }
    }
}

impl Default for HashEmbedder {
    fn default() -> Self {
        Self { dims: 64 }
    }
}

impl Embedder for HashEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        let dims = self.dims.max(1);
        let mut vector = vec![0.0_f32; dims];
        for token in tokenize(text) {
            let bucket = (fnv1a(&token) % dims as u64) as usize;
            vector[bucket] += 1.0;
        }
        vector
    }
}

/// FNV-1a hash — a small, dependency-free, deterministic string hash.
fn fnv1a(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Cosine similarity of two vectors in `[0.0, 1.0]` for non-negative inputs;
/// `0.0` when either is the zero vector.
fn cosine(left: &[f32], right: &[f32]) -> f32 {
    let dot: f32 = left.iter().zip(right).map(|(a, b)| a * b).sum();
    let norm_left: f32 = left.iter().map(|a| a * a).sum();
    let norm_right: f32 = right.iter().map(|b| b * b).sum();
    if norm_left > 0.0 && norm_right > 0.0 {
        dot / (norm_left.sqrt() * norm_right.sqrt())
    } else {
        0.0
    }
}

/// A similarity-ranked recall hit: the matched episode plus its cosine
/// similarity and recency. See [`EpisodicStore::rank_by_similarity`].
#[derive(Debug, Clone, Copy)]
pub struct SimilarHit<'a> {
    episode: &'a Episode,
    similarity: f32,
    recency: f32,
}

impl<'a> SimilarHit<'a> {
    pub const fn episode(&self) -> &Episode {
        self.episode
    }

    /// Cosine similarity between the cue and the episode, in `[0.0, 1.0]`.
    pub const fn similarity(&self) -> f32 {
        self.similarity
    }

    /// Recency weight in `(0.0, 1.0]`.
    pub const fn recency(&self) -> f32 {
        self.recency
    }

    /// Combined score `similarity × recency × importance`.
    pub fn score(&self) -> f32 {
        self.similarity * self.recency * self.episode.importance()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryError;

impl fmt::Display for MemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("memory operation failed")
    }
}

impl Error for MemoryError {}
