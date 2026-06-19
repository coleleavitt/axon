use std::error::Error;
use std::fmt;
use std::num::NonZeroUsize;

use axon_core::Priority;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Goal(String);

impl Goal {
    pub fn new(text: impl Into<String>) -> Result<Self, WorkspaceError> {
        let text = text.into();
        if text.is_empty() {
            Err(WorkspaceError::EmptyGoal)
        } else {
            Ok(Self(text))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Broadcast {
    kind: BroadcastKind,
    text: String,
    salience: Priority,
}

impl Broadcast {
    pub fn observation(text: impl Into<String>) -> Self {
        Self::of(BroadcastKind::Observation, text)
    }

    pub fn decision(text: impl Into<String>) -> Self {
        Self::of(BroadcastKind::Decision, text)
    }

    pub fn alert(text: impl Into<String>) -> Self {
        Self::of(BroadcastKind::Alert, text)
    }

    fn of(kind: BroadcastKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            salience: kind.default_salience(),
        }
    }

    /// Override the ignition salience (default is the kind's; see
    /// [`BroadcastKind::default_salience`]). Higher salience wins admission to the
    /// bounded workspace and resists eviction.
    #[must_use]
    pub fn with_salience(mut self, salience: Priority) -> Self {
        self.salience = salience;
        self
    }

    pub const fn kind(&self) -> BroadcastKind {
        self.kind
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn salience(&self) -> Priority {
        self.salience
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BroadcastKind {
    Observation,
    Decision,
    Alert,
}

impl BroadcastKind {
    /// Default ignition salience: alerts outrank decisions outrank observations,
    /// so a flood of routine observations cannot evict an urgent alert.
    pub const fn default_salience(self) -> Priority {
        match self {
            Self::Alert => Priority::new(10),
            Self::Decision => Priority::new(5),
            Self::Observation => Priority::new(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Workspace {
    capacity: NonZeroUsize,
    goal: Option<Goal>,
    broadcasts: Vec<Broadcast>,
}

impl Workspace {
    pub fn new(capacity: usize) -> Result<Self, WorkspaceError> {
        let capacity = NonZeroUsize::new(capacity).ok_or(WorkspaceError::ZeroCapacity)?;
        Ok(Self {
            capacity,
            goal: None,
            broadcasts: Vec::new(),
        })
    }

    pub fn with_goal(mut self, goal: Goal) -> Self {
        self.goal = Some(goal);
        self
    }

    pub fn set_goal(&mut self, goal: Goal) {
        self.goal = Some(goal);
    }

    pub const fn goal(&self) -> Option<&Goal> {
        self.goal.as_ref()
    }

    /// Admit `item` to the bounded workspace under salience competition (GWT
    /// ignition). Below capacity it is always admitted. When full, it is admitted
    /// only if it is at least as salient as the current weakest item — evicting
    /// that weakest, oldest-first among ties — and dropped otherwise. Returns
    /// whether it was admitted, so a routine observation can never push out an
    /// urgent alert.
    pub fn broadcast(&mut self, item: Broadcast) -> bool {
        if self.broadcasts.len() < self.capacity.get() {
            self.broadcasts.push(item);
            return true;
        }
        match self.weakest_index() {
            Some(index) if item.salience >= self.broadcasts[index].salience => {
                self.broadcasts.remove(index);
                self.broadcasts.push(item);
                true
            }
            _ => false,
        }
    }

    pub fn broadcasts(&self) -> &[Broadcast] {
        &self.broadcasts
    }

    /// The most salient item currently held (most recent among ties), or `None`.
    pub fn most_salient(&self) -> Option<&Broadcast> {
        self.broadcasts.iter().max_by_key(|item| item.salience)
    }

    /// Index of the weakest-salience item, oldest first among ties — the next to
    /// be evicted under admission pressure.
    fn weakest_index(&self) -> Option<usize> {
        self.broadcasts
            .iter()
            .enumerate()
            .min_by(|(left_index, left), (right_index, right)| {
                left.salience
                    .cmp(&right.salience)
                    .then(left_index.cmp(right_index))
            })
            .map(|(index, _)| index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceError {
    ZeroCapacity,
    EmptyGoal,
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCapacity => formatter.write_str("workspace capacity must be non-zero"),
            Self::EmptyGoal => formatter.write_str("workspace goal cannot be empty"),
        }
    }
}

impl Error for WorkspaceError {}
