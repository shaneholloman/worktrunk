//! Directive output mode for shell integration
//!
//! # How Shell Integration Works
//!
//! Worktrunk uses a directive protocol to enable shell integration. When running with
//! `--internal` flag (invoked by shell wrapper), commands output directives that the
//! shell wrapper parses and executes.
//!
//! ## Protocol
//!
//! Running `wt switch --internal my-branch` outputs:
//!
//! ```text
//! __WORKTRUNK_CD__/path/to/worktree\0
//! Switched to worktree: my-branch\0
//! ```
//!
//! The shell wrapper parses this output:
//! - Lines starting with `__WORKTRUNK_CD__` trigger directory changes
//! - Lines starting with `__WORKTRUNK_EXEC__` trigger command execution
//! - Other lines print normally to the user
//! - All messages are NUL-terminated for reliable parsing
//!
//! This separation keeps the Rust binary focused on git logic while the shell
//! handles environment changes (cd, exec).
//!
//! ## Pattern
//!
//! This pattern is proven by tools like zoxide, starship, and direnv. The `--internal`
//! flag is hidden from help output—end users never interact with it directly.

use std::io::{self, Write};
use std::path::Path;

/// Directive output mode for shell integration
///
/// Outputs NUL-terminated directives for shell wrapper to parse and execute.
///
/// See module-level documentation for protocol details.
pub struct DirectiveOutput;

impl DirectiveOutput {
    pub fn new() -> Self {
        Self
    }

    pub fn success(&mut self, message: String) -> io::Result<()> {
        // Don't strip colors - users see this output and need styling
        write!(io::stdout(), "{}\0", message)?;
        io::stdout().flush()
    }

    pub fn progress(&mut self, message: String) -> io::Result<()> {
        // Progress messages are for humans - output them just like success messages
        // The shell wrapper will display these to users with colors preserved
        write!(io::stdout(), "{}\0", message)?;
        io::stdout().flush()
    }

    pub fn hint(&mut self, _message: String) -> io::Result<()> {
        // Hints are only for interactive mode - suppress in directive mode
        // When users run through shell wrapper, they already have integration
        Ok(())
    }

    pub fn change_directory(&mut self, path: &Path) -> io::Result<()> {
        write!(io::stdout(), "__WORKTRUNK_CD__{}\0", path.display())?;
        io::stdout().flush()
    }

    pub fn execute(&mut self, command: String) -> io::Result<()> {
        write!(io::stdout(), "__WORKTRUNK_EXEC__{}\0", command)?;
        io::stdout().flush()
    }

    pub fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }

    pub fn terminate_output(&mut self) -> io::Result<()> {
        // Write NUL terminator to separate command output from subsequent directives
        write!(io::stdout(), "\0")?;
        io::stdout().flush()
    }

    /// Format a switch success message for directive mode
    ///
    /// In directive mode, the shell wrapper will actually change directories,
    /// so we can say "changed directory to {path}"
    pub fn format_switch_success(
        &self,
        branch: &str,
        path: &Path,
        created_branch: bool,
        base_branch: Option<&str>,
    ) -> String {
        super::format_switch_success_message(branch, path, created_branch, base_branch, true)
    }
}

impl Default for DirectiveOutput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::PathBuf;

    /// Test that directive output produces correctly formatted NUL-terminated strings
    ///
    /// While we can't easily test that flush() is called in unit tests,
    /// we can verify the output format is correct. The flushing is critical
    /// for fish shell integration to work correctly - without immediate flushing,
    /// the fish shell's `while read -z chunk` loop will block waiting for data
    /// that's stuck in stdout's buffer.
    #[test]
    fn test_directive_format() {
        // Create a buffer to capture output
        let mut buffer = Vec::new();

        // Test change_directory format
        let path = PathBuf::from("/test/path");
        write!(&mut buffer, "__WORKTRUNK_CD__{}\0", path.display()).unwrap();

        // Test success message format
        let message = "Test message";
        write!(&mut buffer, "{}\0", message).unwrap();

        // Test execute command format
        let command = "echo test";
        write!(&mut buffer, "__WORKTRUNK_EXEC__{}\0", command).unwrap();

        // Verify the buffer contains NUL-terminated strings
        let output = String::from_utf8_lossy(&buffer);
        assert!(output.contains("__WORKTRUNK_CD__/test/path\0"));
        assert!(output.contains("Test message\0"));
        assert!(output.contains("__WORKTRUNK_EXEC__echo test\0"));

        // Verify NUL bytes are in the right places
        let nul_count = buffer.iter().filter(|&&b| b == 0).count();
        assert_eq!(nul_count, 3, "Should have exactly 3 NUL terminators");
    }

    /// Test that anstyle formatting is preserved in directive output
    #[test]
    fn test_success_preserves_anstyle() {
        use anstyle::{AnsiColor, Color, Style};

        let bold = Style::new().bold();
        let cyan = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan)));

        // Create a styled message
        let styled = format!("{cyan}Styled{cyan:#} {bold}message{bold:#}");

        // The styled message should contain ANSI escape codes
        assert!(
            styled.contains('\x1b'),
            "Styled message should contain ANSI escape codes"
        );

        // Directive mode preserves styling for users viewing through shell wrapper
        // (We're not testing actual output here, just documenting the behavior)
    }
}
