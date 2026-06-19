use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug)]
pub enum ToolError {
    Io(std::io::Error),
    UnsafePath {
        path: PathBuf,
    },
    FileTooLarge {
        path: PathBuf,
        max_bytes: u64,
        actual_bytes: u64,
    },
    OutputTooLarge {
        stream: &'static str,
        max_bytes: usize,
    },
    TimedOut {
        timeout: Duration,
    },
    MissingPipe {
        stream: &'static str,
    },
    GitFailed {
        stderr: String,
    },
    ThreadJoin {
        stream: &'static str,
    },
    InvalidLimit,
}

impl fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(formatter, "tool io failed: {source}"),
            Self::UnsafePath { path } => {
                write!(formatter, "path escapes tool root: {}", path.display())
            }
            Self::FileTooLarge {
                path,
                max_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "file {} is {actual_bytes} bytes, exceeding limit {max_bytes}",
                path.display()
            ),
            Self::OutputTooLarge { stream, max_bytes } => {
                write!(formatter, "{stream} exceeded output limit {max_bytes}")
            }
            Self::TimedOut { timeout } => write!(formatter, "command timed out after {timeout:?}"),
            Self::MissingPipe { stream } => write!(formatter, "missing {stream} pipe"),
            Self::GitFailed { stderr } => write!(formatter, "git status failed: {stderr}"),
            Self::ThreadJoin { stream } => write!(formatter, "{stream} reader thread panicked"),
            Self::InvalidLimit => formatter.write_str("tool limit must be non-zero"),
        }
    }
}

impl Error for ToolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::UnsafePath { path: _ }
            | Self::FileTooLarge {
                path: _,
                max_bytes: _,
                actual_bytes: _,
            }
            | Self::OutputTooLarge {
                stream: _,
                max_bytes: _,
            }
            | Self::TimedOut { timeout: _ }
            | Self::MissingPipe { stream: _ }
            | Self::GitFailed { stderr: _ }
            | Self::ThreadJoin { stream: _ }
            | Self::InvalidLimit => None,
        }
    }
}

pub(crate) fn ensure_nonzero_u64(value: u64) -> Result<u64, ToolError> {
    if value == 0 {
        Err(ToolError::InvalidLimit)
    } else {
        Ok(value)
    }
}

pub(crate) fn ensure_nonzero_usize(value: usize) -> Result<usize, ToolError> {
    if value == 0 {
        Err(ToolError::InvalidLimit)
    } else {
        Ok(value)
    }
}
