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

/// The lifecycle status of a goal on the [`GoalStack`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GoalStatus {
    Active,
    Blocked,
    Done,
}

/// A goal with a priority and a lifecycle status — richer than the single string
/// [`Goal`], so objectives can be maintained, prioritized, and tracked.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GoalItem {
    text: String,
    priority: Priority,
    status: GoalStatus,
}

impl GoalItem {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn priority(&self) -> Priority {
        self.priority
    }

    pub const fn status(&self) -> GoalStatus {
        self.status
    }
}

/// A prioritized goal stack with status tracking — the PFC goal-maintenance the
/// flat single-string `Goal` lacked. Long-horizon work pushes sub-goals; the
/// agent always pursues the highest-priority active one and marks goals
/// blocked/done as it progresses.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GoalStack {
    goals: Vec<GoalItem>,
}

impl GoalStack {
    pub const fn new() -> Self {
        Self { goals: Vec::new() }
    }

    /// Push a new active goal with the given priority.
    pub fn push(&mut self, text: impl Into<String>, priority: Priority) {
        self.goals.push(GoalItem {
            text: text.into(),
            priority,
            status: GoalStatus::Active,
        });
    }

    pub fn goals(&self) -> &[GoalItem] {
        &self.goals
    }

    pub fn len(&self) -> usize {
        self.goals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.goals.is_empty()
    }

    fn active_index(&self) -> Option<usize> {
        self.goals
            .iter()
            .enumerate()
            .filter(|(_, goal)| goal.status == GoalStatus::Active)
            .max_by(|(left_index, left), (right_index, right)| {
                left.priority
                    .cmp(&right.priority)
                    .then(left_index.cmp(right_index))
            })
            .map(|(index, _)| index)
    }

    /// The highest-priority active goal (most recent among ties), or `None`.
    pub fn active(&self) -> Option<&GoalItem> {
        self.active_index().map(|index| &self.goals[index])
    }

    fn set_active_status(&mut self, status: GoalStatus) -> bool {
        match self.active_index() {
            Some(index) => {
                self.goals[index].status = status;
                true
            }
            None => false,
        }
    }

    /// Mark the current active goal Done; returns whether one was active.
    pub fn complete_active(&mut self) -> bool {
        self.set_active_status(GoalStatus::Done)
    }

    /// Mark the current active goal Blocked; returns whether one was active.
    pub fn block_active(&mut self) -> bool {
        self.set_active_status(GoalStatus::Blocked)
    }

    /// Drop every Done goal.
    pub fn retire_done(&mut self) {
        self.goals.retain(|goal| goal.status != GoalStatus::Done);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Broadcast {
    kind: BroadcastKind,
    text: String,
    salience: Priority,
    /// How many distinct modules have integrated (consumed) this broadcast — the
    /// ignition reach metric (see [`Workspace::acknowledge`]).
    reach: u32,
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
            reach: 0,
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

    /// How many modules have integrated this broadcast — its ignition reach.
    pub const fn reach(&self) -> u32 {
        self.reach
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

    /// Record that a module integrated (consumed) the first broadcast matching
    /// `predicate`, bumping its reach. Returns whether a match was found.
    ///
    /// Reach is the practical, non-mystical reading of IIT/Φ for an agent: did a
    /// broadcast actually reach and change behavior in downstream modules, or did
    /// it ignite and fizzle? It is a counter to debug *why an agent ignored a
    /// warning* — not a control law.
    pub fn acknowledge(&mut self, predicate: impl Fn(&Broadcast) -> bool) -> bool {
        match self.broadcasts.iter_mut().find(|item| predicate(item)) {
            Some(item) => {
                item.reach = item.reach.saturating_add(1);
                true
            }
            None => false,
        }
    }

    /// How many held broadcasts reached at least `threshold` consumers — the
    /// count that genuinely *ignited* (vs were merely deposited).
    pub fn ignited(&self, threshold: u32) -> usize {
        self.broadcasts
            .iter()
            .filter(|item| item.reach >= threshold)
            .count()
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
