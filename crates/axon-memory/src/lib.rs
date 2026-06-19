use std::cmp::Ordering;
use std::collections::BTreeMap;
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
}

impl EpisodicStore {
    pub const fn new() -> Self {
        Self {
            episodes: Vec::new(),
            next_id: 1,
        }
    }

    pub fn episodes(&self) -> &[Episode] {
        &self.episodes
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
        // separable from one already held. A re-encoded duplicate returns the
        // existing id instead of accumulating redundant episodes.
        if let Some(existing) = self
            .episodes
            .iter()
            .find(|stored| same_memory(stored.text(), episode.text()))
        {
            return existing.id();
        }
        let id = MemoryId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        episode.assign(id);
        self.episodes.push(episode);
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

/// Two memories are the same when their text is equal after trimming and
/// case-folding — the orthogonalization rule applied on write.
fn same_memory(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryError;

impl fmt::Display for MemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("memory operation failed")
    }
}

impl Error for MemoryError {}
