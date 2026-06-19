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

/// Lists directory entries within a sandbox root, using the same path-safety
/// model as [`FsRead`]: relative paths only, no `..`, and the resolved target
/// must stay inside the root.
#[derive(Debug)]
pub struct FsList {
    root: PathBuf,
}

impl FsList {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl Tool for FsList {
    /// A relative directory; empty or `"."` lists the root itself.
    type Input = String;
    /// Sorted entry names, directories suffixed with `/`.
    type Output = Vec<String>;
    type Error = ToolError;

    fn call(&mut self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        let root = self.root.canonicalize().map_err(ToolError::Io)?;
        let relative = input.trim();
        let target = if relative.is_empty() || relative == "." {
            root.clone()
        } else {
            safe_join(&root, Path::new(relative))?
        };
        let canonical = target.canonicalize().map_err(ToolError::Io)?;
        if !canonical.starts_with(&root) {
            return Err(ToolError::UnsafePath { path: canonical });
        }
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&canonical).map_err(ToolError::Io)? {
            let entry = entry.map_err(ToolError::Io)?;
            let suffix = if entry.file_type().map_err(ToolError::Io)?.is_dir() {
                "/"
            } else {
                ""
            };
            entries.push(format!("{}{suffix}", entry.file_name().to_string_lossy()));
        }
        entries.sort();
        Ok(entries)
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
