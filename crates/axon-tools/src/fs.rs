use std::path::{Component, Path, PathBuf};

use crate::error::ensure_nonzero_u64;
use crate::{Tool, ToolError};

const DEFAULT_MAX_BYTES: u64 = 1_048_576;

#[derive(Debug)]
pub struct FsRead {
    root: PathBuf,
    max_bytes: u64,
}

impl FsRead {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }

    pub fn with_max_bytes(mut self, max_bytes: u64) -> Result<Self, ToolError> {
        self.max_bytes = ensure_nonzero_u64(max_bytes)?;
        Ok(self)
    }
}

impl Tool for FsRead {
    type Input = String;
    type Output = String;
    type Error = ToolError;

    fn call(&mut self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        let root = self.root.canonicalize().map_err(ToolError::Io)?;
        let candidate = safe_join(&root, Path::new(&input))?;
        let canonical = candidate.canonicalize().map_err(ToolError::Io)?;
        if !canonical.starts_with(&root) {
            return Err(ToolError::UnsafePath { path: canonical });
        }
        let actual_bytes = std::fs::metadata(&canonical).map_err(ToolError::Io)?.len();
        if actual_bytes > self.max_bytes {
            return Err(ToolError::FileTooLarge {
                path: canonical,
                max_bytes: self.max_bytes,
                actual_bytes,
            });
        }
        std::fs::read_to_string(canonical).map_err(ToolError::Io)
    }
}

fn safe_join(root: &Path, rel: &Path) -> Result<PathBuf, ToolError> {
    if rel.is_absolute()
        || rel
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(ToolError::UnsafePath {
            path: rel.to_path_buf(),
        });
    }
    Ok(root.join(rel))
}
