use std::error::Error;
use std::fmt;
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepLimit(NonZeroUsize);

impl StepLimit {
    pub fn try_new(value: usize) -> Result<Self, StepLimitError> {
        match NonZeroUsize::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(StepLimitError),
        }
    }

    pub const fn get(self) -> usize {
        self.0.get()
    }
}

impl Default for StepLimit {
    fn default() -> Self {
        match NonZeroUsize::new(128) {
            Some(value) => Self(value),
            None => Self(NonZeroUsize::MIN),
        }
    }
}

impl fmt::Display for StepLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.get())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepLimitError;

impl fmt::Display for StepLimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("step limit must be non-zero")
    }
}

impl Error for StepLimitError {}
