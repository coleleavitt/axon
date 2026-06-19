use std::convert::Infallible;
use std::error::Error;
use std::time::Duration;

use axon_core::{Module, ModuleId, ModuleOutput, Signal};
use axon_tools::{
    CommandSpec,
    FsList,
    FsRead,
    GitStatus,
    ShellCommand,
    Tool,
    ToolModule,
    ToolSignal,
};

#[test]
fn fs_read_tool_reads_utf8_file_when_path_exists() -> Result<(), Box<dyn Error>> {
    // Given: a filesystem tool rooted at the crate manifest directory.
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut tool = FsRead::new(root);

    // When: a crate-local manifest file is read.
    let output = tool.call("Cargo.toml".to_owned())?;

    // Then: the text contains this crate's package name.
    assert!(output.contains("axon-tools"));
    Ok(())
}

#[test]
fn shell_command_runs_program_and_captures_stdout() -> Result<(), Box<dyn Error>> {
    // Given: a shell command tool with the current crate as working directory.
    let mut tool = ShellCommand::new(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")));

    // When: a simple command is executed.
    let output = tool.call(CommandSpec::new("printf").arg("sensorimotor"))?;

    // Then: stdout is captured without invoking an LLM loop.
    assert_eq!(output.stdout(), "sensorimotor");
    assert_eq!(output.status_code(), 0);
    Ok(())
}

#[test]
fn tool_module_maps_tool_result_into_core_module_output() -> Result<(), Box<dyn Error>> {
    // Given: a function-backed tool wrapped as an Axon module.
    let id = ModuleId::new("echo_tool")?;
    let mut module = ToolModule::new(id.clone(), |input: String| {
        Ok::<usize, Infallible>(input.len())
    });

    // When: the module receives a tool call signal.
    let output = module.handle(Signal::new(ToolSignal::Call("axon".to_owned())))?;

    // Then: the tool result is emitted as a typed core signal.
    match output {
        ModuleOutput::Emit(signal) => assert_eq!(signal.into_payload(), ToolSignal::Result(4)),
        ModuleOutput::Stop(signal) => panic!("unexpected stop: {signal:?}"),
        ModuleOutput::Drop => panic!("unexpected drop"),
    }
    assert_eq!(module.id(), &id);
    Ok(())
}

#[test]
fn git_status_reports_clean_or_dirty_without_failing() -> Result<(), Box<dyn Error>> {
    // Given: a git status tool rooted at the workspace.
    let mut tool =
        GitStatus::new(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));

    // When: status is requested.
    let status = tool.call(())?;

    // Then: the branch name and cleanliness flag are available as data.
    assert!(!status.branch().is_empty());
    let _ = status.is_clean();
    Ok(())
}

#[cfg(unix)]
#[test]
fn fs_read_rejects_symlink_escape_from_root() -> Result<(), Box<dyn Error>> {
    // Given: a root directory containing a symlink to a file outside that root.
    let root = temp_dir("axon-tools-root")?;
    let outside = temp_dir("axon-tools-outside")?;
    let secret = outside.join("secret.txt");
    std::fs::write(&secret, "secret-outside-root")?;
    std::os::unix::fs::symlink(&secret, root.join("link.txt"))?;
    let mut tool = FsRead::new(root.clone());

    // When: the symlink is read through the rooted tool.
    let result = tool.call("link.txt".to_owned());

    // Then: the read fails instead of escaping the root.
    assert!(result.is_err());
    std::fs::remove_dir_all(root)?;
    std::fs::remove_dir_all(outside)?;
    Ok(())
}

#[test]
fn fs_read_rejects_files_larger_than_configured_limit() -> Result<(), Box<dyn Error>> {
    // Given: a rooted file larger than the read limit.
    let root = temp_dir("axon-tools-large")?;
    std::fs::write(root.join("large.txt"), "abcdef")?;
    let mut tool = FsRead::new(root.clone()).with_max_bytes(3)?;

    // When: the oversized file is read.
    let result = tool.call("large.txt".to_owned());

    // Then: the tool refuses to buffer it.
    assert!(result.is_err());
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn shell_command_times_out_long_running_programs() -> Result<(), Box<dyn Error>> {
    // Given: a command tool with a short timeout.
    let mut tool = ShellCommand::new(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")))
        .with_timeout(Duration::from_millis(50));

    // When: a command exceeds the timeout.
    let result = tool.call(CommandSpec::new("sh").arg("-c").arg("sleep 1"));

    // Then: execution fails closed.
    assert!(result.is_err());
    Ok(())
}

#[test]
fn shell_command_rejects_output_larger_than_configured_limit() -> Result<(), Box<dyn Error>> {
    // Given: a command tool with a tiny output limit.
    let mut tool = ShellCommand::new(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")))
        .with_max_output_bytes(3)?;

    // When: command output exceeds the configured limit.
    let result = tool.call(CommandSpec::new("printf").arg("abcdef"));

    // Then: execution fails instead of buffering unbounded output.
    assert!(result.is_err());
    Ok(())
}

#[test]
fn git_status_fails_for_non_git_directory() -> Result<(), Box<dyn Error>> {
    // Given: a git status tool rooted at a non-repository directory.
    let root = temp_dir("axon-tools-nongit")?;
    let mut tool = GitStatus::new(root.clone());

    // When: status is requested.
    let result = tool.call(());

    // Then: it fails instead of reporting an empty clean branch.
    assert!(result.is_err());
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn fs_list_lists_sorted_source_entries() -> Result<(), Box<dyn Error>> {
    // Given: a list tool rooted at the crate manifest directory.
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut tool = FsList::new(root);

    // When: the crate source directory is listed.
    let entries = tool.call("src".to_owned())?;

    // Then: it returns the module files this crate is built from.
    assert!(entries.contains(&"fs.rs".to_owned()));
    assert!(entries.contains(&"lib.rs".to_owned()));
    Ok(())
}

#[test]
fn fs_list_lists_root_and_marks_subdirectories() -> Result<(), Box<dyn Error>> {
    // Given: a list tool rooted at the crate manifest directory.
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut tool = FsList::new(root);

    // When: the root itself is listed via an empty path.
    let entries = tool.call(String::new())?;

    // Then: files appear plain and directories carry a trailing slash.
    assert!(entries.contains(&"Cargo.toml".to_owned()));
    assert!(entries.contains(&"src/".to_owned()));
    Ok(())
}

#[test]
fn fs_list_rejects_parent_directory_escape() -> Result<(), Box<dyn Error>> {
    // Given: a list tool rooted at an isolated directory.
    let root = temp_dir("axon-tools-list")?;
    let mut tool = FsList::new(root.clone());

    // When: a parent-directory traversal is requested.
    let result = tool.call("..".to_owned());

    // Then: it is refused instead of escaping the root.
    assert!(result.is_err());
    std::fs::remove_dir_all(root)?;
    Ok(())
}

fn temp_dir(prefix: &str) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
    std::fs::create_dir(&dir)?;
    Ok(dir)
}
