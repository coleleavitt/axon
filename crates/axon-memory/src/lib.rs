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

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Episode {
    id: MemoryId,
    text: String,
    tags: Vec<String>,
}

impl Episode {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: MemoryId::default(),
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

    pub const fn id(&self) -> MemoryId {
        self.id
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
        let mut results: Vec<_> = self
            .episodes
            .iter()
            .filter_map(|episode| RecallResult::from_episode(episode, query))
            .collect();
        // Rank by relevance, breaking ties toward more recent memories (higher
        // id) so recency biases retrieval the way episodic recall does.
        results.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
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
    score: u32,
}

impl<'a> RecallResult<'a> {
    fn from_episode(episode: &'a Episode, query: &RecallQuery) -> Option<Self> {
        let text_score = lexical_overlap(&query.text, &episode.text);
        let tag_score = query
            .tags
            .iter()
            .filter(|tag| episode.tags.iter().any(|candidate| candidate == *tag))
            .count()
            .try_into()
            .unwrap_or(u32::MAX);
        let score = text_score.saturating_add(tag_score);
        (score > 0).then_some(Self { episode, score })
    }

    pub const fn episode(&self) -> &Episode {
        self.episode
    }

    pub const fn score(&self) -> u32 {
        self.score
    }
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
