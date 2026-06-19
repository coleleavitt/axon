use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::error::ensure_nonzero_usize;
use crate::{Tool, ToolError};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_OUTPUT_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    program: String,
    args: Vec<String>,
}

impl CommandSpec {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }
}

#[derive(Debug)]
pub struct ShellCommand {
    cwd: PathBuf,
    timeout: Duration,
    max_output_bytes: usize,
}

impl ShellCommand {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            timeout: DEFAULT_TIMEOUT,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }

    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_max_output_bytes(mut self, max_output_bytes: usize) -> Result<Self, ToolError> {
        self.max_output_bytes = ensure_nonzero_usize(max_output_bytes)?;
        Ok(self)
    }
}

impl Tool for ShellCommand {
    type Input = CommandSpec;
    type Output = CommandOutput;
    type Error = ToolError;

    fn call(&mut self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        let mut child = Command::new(&input.program)
            .args(&input.args)
            .current_dir(&self.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(ToolError::Io)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(ToolError::MissingPipe { stream: "stdout" })?;
        let stderr = child
            .stderr
            .take()
            .ok_or(ToolError::MissingPipe { stream: "stderr" })?;
        let stdout_reader = std::thread::spawn({
            let max = self.max_output_bytes;
            move || read_limited(stdout, "stdout", max)
        });
        let stderr_reader = std::thread::spawn({
            let max = self.max_output_bytes;
            move || read_limited(stderr, "stderr", max)
        });
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().map_err(ToolError::Io)? {
                break status;
            }
            if started.elapsed() >= self.timeout {
                let _ = child.kill();
                let _ = child.wait();
                join_reader(stdout_reader, "stdout")?;
                join_reader(stderr_reader, "stderr")?;
                return Err(ToolError::TimedOut {
                    timeout: self.timeout,
                });
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        Ok(CommandOutput {
            stdout: join_reader(stdout_reader, "stdout")?,
            stderr: join_reader(stderr_reader, "stderr")?,
            status_code: status.code().unwrap_or(-1),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    stdout: String,
    stderr: String,
    status_code: i32,
}

impl CommandOutput {
    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    pub const fn status_code(&self) -> i32 {
        self.status_code
    }
}

fn read_limited<R: Read>(
    mut reader: R,
    stream: &'static str,
    max: usize,
) -> Result<String, ToolError> {
    let mut output = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let count = reader.read(&mut chunk).map_err(ToolError::Io)?;
        if count == 0 {
            return Ok(String::from_utf8_lossy(&output).into_owned());
        }
        if output.len().saturating_add(count) > max {
            return Err(ToolError::OutputTooLarge {
                stream,
                max_bytes: max,
            });
        }
        output.extend_from_slice(&chunk[..count]);
    }
}

fn join_reader(
    handle: JoinHandle<Result<String, ToolError>>,
    stream: &'static str,
) -> Result<String, ToolError> {
    match handle.join() {
        Ok(result) => result,
        Err(_) => Err(ToolError::ThreadJoin { stream }),
    }
}
