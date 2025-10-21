use config::{Config, ConfigError, File};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use toml;

/// Configuration for worktree path formatting and LLM integration.
///
/// The `worktree-path` template is relative to the repository root and supports:
/// - `{repo}` - Repository name
/// - `{branch}` - Branch name (slashes replaced with dashes)
///
/// # Examples
///
/// ```toml
/// # Default - parent directory siblings
/// worktree-path = "../{repo}.{branch}"
///
/// # Inside repo (bare repository style)
/// worktree-path = "{branch}"
///
/// # Organized in .worktrees subdirectory
/// worktree-path = ".worktrees/{branch}"
///
/// # Repository-namespaced shared directory (avoids conflicts)
/// worktree-path = "../worktrees/{repo}/{branch}"
///
/// # LLM configuration for commit message generation
/// [llm]
/// command = "llm"  # Command to invoke LLM (e.g., "llm", "claude")
/// args = ["-s"]    # Arguments to pass to the command
/// ```
///
/// Config file location:
/// - Linux: `~/.config/worktrunk/config.toml`
/// - macOS: `~/Library/Application Support/worktrunk/config.toml`
/// - Windows: `%APPDATA%\worktrunk\config.toml`
///
/// Environment variable: `WORKTRUNK_WORKTREE_PATH`
#[derive(Debug, Serialize, Deserialize)]
pub struct WorktrunkConfig {
    #[serde(rename = "worktree-path")]
    pub worktree_path: String,

    #[serde(default)]
    pub llm: LlmConfig,

    /// Commands that have been approved for automatic execution
    #[serde(default, rename = "approved-commands")]
    pub approved_commands: Vec<ApprovedCommand>,
}

/// Configuration for LLM integration
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LlmConfig {
    /// Command to invoke LLM (e.g., "llm", "claude")
    #[serde(default)]
    pub command: Option<String>,

    /// Arguments to pass to the LLM command
    #[serde(default)]
    pub args: Vec<String>,
}

/// Project-specific configuration (stored in .config/wt.toml within the project)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProjectConfig {
    /// Commands to execute after creating a new worktree
    #[serde(default, rename = "post-start-commands")]
    pub post_start_commands: Vec<String>,
}

/// Approved command for automatic execution
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ApprovedCommand {
    /// Project identifier (git remote URL or repo name)
    pub project: String,
    /// Command that was approved
    pub command: String,
}

impl Default for WorktrunkConfig {
    fn default() -> Self {
        Self {
            worktree_path: "../{repo}.{branch}".to_string(),
            llm: LlmConfig::default(),
            approved_commands: Vec::new(),
        }
    }
}

impl WorktrunkConfig {
    /// Load configuration from config file and environment variables.
    ///
    /// Configuration is loaded in the following order (later sources override earlier ones):
    /// 1. Default values
    /// 2. Config file (~/.config/worktrunk/config.toml on Linux/macOS)
    /// 3. Environment variables (WORKTRUNK_*)
    pub fn load() -> Result<Self, ConfigError> {
        let defaults = Self::default();

        let mut builder = Config::builder()
            .set_default("worktree-path", defaults.worktree_path)?
            .set_default("llm.command", defaults.llm.command.unwrap_or_default())?
            .set_default("llm.args", defaults.llm.args)?;

        // Add config file if it exists
        if let Some(config_path) = get_config_path()
            && config_path.exists()
        {
            builder = builder.add_source(File::from(config_path));
        }

        // Add environment variables with WORKTRUNK prefix
        builder = builder.add_source(config::Environment::with_prefix("WORKTRUNK").separator("_"));

        let config: Self = builder.build()?.try_deserialize()?;
        validate_worktree_path(&config.worktree_path)?;
        Ok(config)
    }

    /// Format a worktree path using this configuration's template.
    ///
    /// # Arguments
    /// * `repo` - Repository name (replaces {repo} in template)
    /// * `branch` - Branch name (replaces {branch} in template, slashes sanitized to dashes)
    ///
    /// # Examples
    /// ```
    /// use worktrunk::config::WorktrunkConfig;
    ///
    /// let config = WorktrunkConfig::default();
    /// let path = config.format_path("myproject", "feature/foo");
    /// assert_eq!(path, "../myproject.feature-foo");
    /// ```
    pub fn format_path(&self, repo: &str, branch: &str) -> String {
        // Sanitize branch name by replacing path separators to prevent directory traversal
        let safe_branch = branch.replace(['/', '\\'], "-");
        self.worktree_path
            .replace("{repo}", repo)
            .replace("{branch}", &safe_branch)
    }
}

fn get_config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "worktrunk").map(|dirs| dirs.config_dir().join("config.toml"))
}

impl ProjectConfig {
    /// Load project configuration from .config/wt.toml in the repository root
    pub fn load(repo_root: &std::path::Path) -> Result<Option<Self>, ConfigError> {
        let config_path = repo_root.join(".config").join("wt.toml");

        if !config_path.exists() {
            return Ok(None);
        }

        let config = Config::builder()
            .add_source(File::from(config_path))
            .build()?;

        Ok(Some(config.try_deserialize()?))
    }
}

impl WorktrunkConfig {
    /// Check if a command is approved for the given project
    pub fn is_command_approved(&self, project: &str, command: &str) -> bool {
        self.approved_commands
            .iter()
            .any(|ac| ac.project == project && ac.command == command)
    }

    /// Add an approved command and save to config file
    pub fn approve_command(&mut self, project: String, command: String) -> Result<(), ConfigError> {
        // Don't add duplicates
        if self.is_command_approved(&project, &command) {
            return Ok(());
        }

        self.approved_commands
            .push(ApprovedCommand { project, command });
        self.save()
    }

    /// Save the current configuration to the config file
    fn save(&self) -> Result<(), ConfigError> {
        let config_path = get_config_path()
            .ok_or_else(|| ConfigError::Message("Could not determine config path".to_string()))?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ConfigError::Message(format!("Failed to create config directory: {}", e))
            })?;
        }

        let toml_string = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::Message(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(&config_path, toml_string)
            .map_err(|e| ConfigError::Message(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }
}

fn validate_worktree_path(template: &str) -> Result<(), ConfigError> {
    if template.is_empty() {
        return Err(ConfigError::Message(
            "worktree-path cannot be empty".to_string(),
        ));
    }

    // Reject absolute paths
    let path = std::path::Path::new(template);
    if path.is_absolute() {
        return Err(ConfigError::Message(
            "worktree-path must be relative, not absolute".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WorktrunkConfig::default();
        assert_eq!(config.worktree_path, "../{repo}.{branch}");
    }

    #[test]
    fn test_config_serialization() {
        let config = WorktrunkConfig::default();
        let toml = toml::to_string(&config).unwrap();
        assert!(toml.contains("worktree-path"));
        assert!(toml.contains("../{repo}.{branch}"));
    }

    #[test]
    fn test_load_config_defaults() {
        // Without a config file or env vars, should return defaults
        let config = WorktrunkConfig::load().unwrap();
        assert_eq!(config.worktree_path, "../{repo}.{branch}");
    }

    #[test]
    fn test_format_worktree_path() {
        let config = WorktrunkConfig {
            worktree_path: "{repo}.{branch}".to_string(),
            llm: LlmConfig::default(),
            approved_commands: Vec::new(),
        };
        assert_eq!(
            config.format_path("myproject", "feature-x"),
            "myproject.feature-x"
        );
    }

    #[test]
    fn test_format_worktree_path_custom_template() {
        let config = WorktrunkConfig {
            worktree_path: "{repo}-{branch}".to_string(),
            llm: LlmConfig::default(),
            approved_commands: Vec::new(),
        };
        assert_eq!(
            config.format_path("myproject", "feature-x"),
            "myproject-feature-x"
        );
    }

    #[test]
    fn test_format_worktree_path_only_branch() {
        let config = WorktrunkConfig {
            worktree_path: "{branch}".to_string(),
            llm: LlmConfig::default(),
            approved_commands: Vec::new(),
        };
        assert_eq!(config.format_path("myproject", "feature-x"), "feature-x");
    }

    #[test]
    fn test_format_worktree_path_with_slashes() {
        // Slashes should be replaced with dashes to prevent directory traversal
        let config = WorktrunkConfig {
            worktree_path: "{repo}.{branch}".to_string(),
            llm: LlmConfig::default(),
            approved_commands: Vec::new(),
        };
        assert_eq!(
            config.format_path("myproject", "feature/foo"),
            "myproject.feature-foo"
        );
    }

    #[test]
    fn test_format_worktree_path_with_multiple_slashes() {
        let config = WorktrunkConfig {
            worktree_path: "{branch}".to_string(),
            llm: LlmConfig::default(),
            approved_commands: Vec::new(),
        };
        assert_eq!(
            config.format_path("myproject", "feature/sub/task"),
            "feature-sub-task"
        );
    }

    #[test]
    fn test_format_worktree_path_with_backslashes() {
        // Windows-style path separators should also be sanitized
        let config = WorktrunkConfig {
            worktree_path: "{branch}".to_string(),
            llm: LlmConfig::default(),
            approved_commands: Vec::new(),
        };
        assert_eq!(
            config.format_path("myproject", "feature\\foo"),
            "feature-foo"
        );
    }

    #[test]
    fn test_validate_rejects_empty_path() {
        let result = validate_worktree_path("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_validate_rejects_absolute_path_unix() {
        let result = validate_worktree_path("/absolute/path/{branch}");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be relative"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_validate_rejects_absolute_path_windows() {
        let result = validate_worktree_path("C:\\absolute\\path\\{branch}");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be relative"));
    }

    #[test]
    fn test_validate_accepts_relative_path() {
        assert!(validate_worktree_path(".worktrees/{branch}").is_ok());
        assert!(validate_worktree_path("../{repo}.{branch}").is_ok());
        assert!(validate_worktree_path("../../shared/{branch}").is_ok());
    }

    #[test]
    fn test_project_config_default() {
        let config = ProjectConfig::default();
        assert!(config.post_start_commands.is_empty());
    }

    #[test]
    fn test_project_config_serialization() {
        let config = ProjectConfig {
            post_start_commands: vec!["npm install".to_string(), "npm test".to_string()],
        };
        let toml = toml::to_string(&config).unwrap();
        assert!(toml.contains("post-start-commands"));
        assert!(toml.contains("npm install"));
        assert!(toml.contains("npm test"));
    }

    #[test]
    fn test_project_config_deserialization() {
        let toml = r#"
            post-start-commands = ["npm install", "npm test"]
        "#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.post_start_commands.len(), 2);
        assert_eq!(config.post_start_commands[0], "npm install");
        assert_eq!(config.post_start_commands[1], "npm test");
    }

    #[test]
    fn test_approved_command_equality() {
        let cmd1 = ApprovedCommand {
            project: "github.com/user/repo".to_string(),
            command: "npm install".to_string(),
        };
        let cmd2 = ApprovedCommand {
            project: "github.com/user/repo".to_string(),
            command: "npm install".to_string(),
        };
        let cmd3 = ApprovedCommand {
            project: "github.com/user/repo".to_string(),
            command: "npm test".to_string(),
        };
        assert_eq!(cmd1, cmd2);
        assert_ne!(cmd1, cmd3);
    }

    #[test]
    fn test_is_command_approved() {
        let mut config = WorktrunkConfig::default();
        config.approved_commands.push(ApprovedCommand {
            project: "github.com/user/repo".to_string(),
            command: "npm install".to_string(),
        });

        assert!(config.is_command_approved("github.com/user/repo", "npm install"));
        assert!(!config.is_command_approved("github.com/user/repo", "npm test"));
        assert!(!config.is_command_approved("github.com/other/repo", "npm install"));
    }

    #[test]
    fn test_approve_command() {
        let mut config = WorktrunkConfig::default();

        // First approval
        assert!(!config.is_command_approved("github.com/user/repo", "npm install"));
        config
            .approve_command(
                "github.com/user/repo".to_string(),
                "npm install".to_string(),
            )
            .ok(); // Ignore save errors in tests
        assert!(config.is_command_approved("github.com/user/repo", "npm install"));

        // Duplicate approval shouldn't add twice
        let count_before = config.approved_commands.len();
        config
            .approve_command(
                "github.com/user/repo".to_string(),
                "npm install".to_string(),
            )
            .ok();
        assert_eq!(config.approved_commands.len(), count_before);
    }
}
