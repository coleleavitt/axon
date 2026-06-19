mod command;
mod error;
mod fs;
mod git;
mod module;

pub use command::{CommandOutput, CommandSpec, ShellCommand};
pub use error::ToolError;
pub use fs::FsRead;
pub use git::{GitStatus, GitStatusOutput};
pub use module::{ToolModule, ToolSignal};

pub trait Tool {
    type Input;
    type Output;
    type Error: std::error::Error + Send + Sync + 'static;

    fn call(&mut self, input: Self::Input) -> Result<Self::Output, Self::Error>;
}
