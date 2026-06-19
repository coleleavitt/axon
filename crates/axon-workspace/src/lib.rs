use std::error::Error;
use std::fmt;
use std::num::NonZeroUsize;

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
}

impl Broadcast {
    pub fn observation(text: impl Into<String>) -> Self {
        Self {
            kind: BroadcastKind::Observation,
            text: text.into(),
        }
    }

    pub fn decision(text: impl Into<String>) -> Self {
        Self {
            kind: BroadcastKind::Decision,
            text: text.into(),
        }
    }

    pub fn alert(text: impl Into<String>) -> Self {
        Self {
            kind: BroadcastKind::Alert,
            text: text.into(),
        }
    }

    pub const fn kind(&self) -> BroadcastKind {
        self.kind
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BroadcastKind {
    Observation,
    Decision,
    Alert,
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

    pub fn broadcast(&mut self, item: Broadcast) {
        if self.broadcasts.len() == self.capacity.get() {
            self.broadcasts.remove(0);
        }
        self.broadcasts.push(item);
    }

    pub fn broadcasts(&self) -> &[Broadcast] {
        &self.broadcasts
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
