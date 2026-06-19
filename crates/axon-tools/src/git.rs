use std::path::PathBuf;
use std::process::Command;

use crate::{Tool, ToolError};

#[derive(Debug)]
pub struct GitStatus {
    cwd: PathBuf,
}

impl GitStatus {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }
}

impl Tool for GitStatus {
    type Input = ();
    type Output = GitStatusOutput;
    type Error = ToolError;

    fn call(&mut self, (): Self::Input) -> Result<Self::Output, Self::Error> {
        let output = Command::new("git")
            .args(["status", "--short", "--branch"])
            .current_dir(&self.cwd)
            .output()
            .map_err(ToolError::Io)?;
        if !output.status.success() {
            return Err(ToolError::GitFailed {
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(GitStatusOutput::parse(&text))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatusOutput {
    branch: String,
    clean: bool,
}

impl GitStatusOutput {
    fn parse(text: &str) -> Self {
        let mut lines = text.lines();
        let branch = lines
            .next()
            .map(|line| line.trim_start_matches("## ").to_owned())
            .unwrap_or_default();
        Self {
            clean: !branch.is_empty() && lines.next().is_none(),
            branch,
        }
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub const fn is_clean(&self) -> bool {
        self.clean
    }
}
