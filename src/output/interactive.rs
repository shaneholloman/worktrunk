//! Interactive output mode for human users

use std::io::{self, Write};
use std::path::Path;
use worktrunk::styling::{GREEN, SUCCESS_EMOJI, println};

/// Interactive output mode for human users
///
/// Formats messages with colors, emojis, and formatting.
/// Executes commands directly instead of emitting directives.
pub struct InteractiveOutput {
    /// Target directory for command execution (set by change_directory)
    target_dir: Option<std::path::PathBuf>,
}

impl InteractiveOutput {
    pub fn new() -> Self {
        Self { target_dir: None }
    }

    pub fn success(&mut self, message: String) -> io::Result<()> {
        println!("{SUCCESS_EMOJI} {GREEN}{message}{GREEN:#}");
        Ok(())
    }

    pub fn progress(&mut self, message: String) -> io::Result<()> {
        println!("{message}");
        io::stdout().flush()?;
        Ok(())
    }

    pub fn change_directory(&mut self, path: &Path) -> io::Result<()> {
        // In interactive mode, we can't actually change directory
        // Just store the target for execute commands
        self.target_dir = Some(path.to_path_buf());
        Ok(())
    }

    pub fn execute(&mut self, command: String) -> io::Result<()> {
        use std::process::Command;

        // Execute command in the target directory
        let exec_dir = self.target_dir.as_deref().unwrap_or_else(|| Path::new("."));

        let output = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(exec_dir)
            .output()
            .map_err(|e| io::Error::other(format!("Failed to execute command: {}", e)))?;

        if !output.status.success() {
            return Err(io::Error::other(format!(
                "Command failed with exit code: {}",
                output.status
            )));
        }

        // Print output directly (we're already inside the output framework)
        if !output.stdout.is_empty() {
            println!("{}", String::from_utf8_lossy(&output.stdout).trim_end());
        }
        if !output.stderr.is_empty() {
            use worktrunk::styling::eprintln;
            eprintln!("{}", String::from_utf8_lossy(&output.stderr).trim_end());
        }

        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()?;
        io::stderr().flush()?;
        Ok(())
    }

    pub fn terminate_output(&mut self) -> io::Result<()> {
        // No-op in interactive mode - no NUL terminators needed
        Ok(())
    }
}

impl Default for InteractiveOutput {
    fn default() -> Self {
        Self::new()
    }
}
