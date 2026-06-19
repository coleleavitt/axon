mod async_module;
mod command;
mod error;
mod fs;
mod git;
mod module;
mod outcome;

pub use async_module::{AsyncTool, AsyncToolModule};
pub use command::{CommandOutput, CommandSpec, ShellCommand};
pub use error::ToolError;
pub use fs::{FsList, FsRead};
pub use git::{GitStatus, GitStatusOutput};
pub use module::{ToolModule, ToolSignal};
pub use outcome::{ToolReport, ToolStatus};

pub trait Tool {
    type Input;
    type Output;
    type Error: std::error::Error + Send + Sync + 'static;

    fn call(&mut self, input: Self::Input) -> Result<Self::Output, Self::Error>;
}
