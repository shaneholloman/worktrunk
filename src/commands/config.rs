use anstyle::{AnsiColor, Color};
use etcetera::base_strategy::{BaseStrategy, choose_base_strategy};
use std::path::PathBuf;
use worktrunk::git::{GitError, Repository};
use worktrunk::styling::{AnstyleStyle, HINT, HINT_EMOJI, println};

/// Example configuration file content
const CONFIG_EXAMPLE: &str = include_str!("../../config.example.toml");

/// Handle the config init command
pub fn handle_config_init() -> Result<(), GitError> {
    let config_path = get_global_config_path().ok_or_else(|| {
        GitError::CommandFailed("Could not determine global config path".to_string())
    })?;

    // Check if file already exists
    if config_path.exists() {
        let bold = AnstyleStyle::new().bold();
        println!(
            "Global config already exists: {bold}{}{bold:#}",
            config_path.display()
        );
        println!();
        println!("{HINT_EMOJI} {HINT}Use 'wt config list' to view existing configuration{HINT:#}");
        return Ok(());
    }

    // Create parent directory if it doesn't exist
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            GitError::CommandFailed(format!("Failed to create config directory: {}", e))
        })?;
    }

    // Write the example config
    std::fs::write(&config_path, CONFIG_EXAMPLE)
        .map_err(|e| GitError::CommandFailed(format!("Failed to write config file: {}", e)))?;

    // Success message
    let green = AnstyleStyle::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
    let bold = AnstyleStyle::new().bold();
    println!("✅ {green}Created config file{green:#}");
    println!("   {bold}{}{bold:#}", config_path.display());
    println!();
    println!(
        "{HINT_EMOJI} {HINT}Edit this file to customize worktree paths and LLM settings{HINT:#}"
    );

    Ok(())
}

/// Handle the config list command
pub fn handle_config_list() -> Result<(), GitError> {
    // Display global config
    display_global_config()?;
    println!();

    // Display project config if in a git repository
    display_project_config()?;

    Ok(())
}

fn display_global_config() -> Result<(), GitError> {
    let bold = AnstyleStyle::new().bold();
    let dim = AnstyleStyle::new().dimmed();

    // Get config path
    let config_path = get_global_config_path().ok_or_else(|| {
        GitError::CommandFailed("Could not determine global config path".to_string())
    })?;

    println!("Global Config: {bold}{}{bold:#}", config_path.display());

    // Check if file exists
    if !config_path.exists() {
        println!("  {HINT_EMOJI} {HINT}Not found (using defaults){HINT:#}");
        println!("  {HINT_EMOJI} {HINT}Run 'wt config init' to create a config file{HINT:#}");
        println!();
        println!("  {dim}# Default configuration:{dim:#}");
        println!("  {dim}worktree-path = \"../{{repo}}.{{branch}}\"{dim:#}");
        return Ok(());
    }

    // Read and display the file contents
    let contents = std::fs::read_to_string(&config_path)
        .map_err(|e| GitError::CommandFailed(format!("Failed to read config file: {}", e)))?;

    if contents.trim().is_empty() {
        println!("  {HINT_EMOJI} {HINT}Empty file (using defaults){HINT:#}");
        return Ok(());
    }

    // Display each line with indentation
    for line in contents.lines() {
        if !line.trim().is_empty() {
            println!("  {dim}{line}{dim:#}");
        } else {
            println!();
        }
    }

    Ok(())
}

fn display_project_config() -> Result<(), GitError> {
    let bold = AnstyleStyle::new().bold();
    let dim = AnstyleStyle::new().dimmed();

    // Try to get current repository root
    let repo = Repository::current();
    let repo_root = match repo.worktree_root() {
        Ok(root) => root,
        Err(_) => {
            println!("Project Config: {dim}Not in a git repository{dim:#}");
            return Ok(());
        }
    };
    let config_path = repo_root.join(".config").join("wt.toml");

    println!("Project Config: {bold}{}{bold:#}", config_path.display());

    // Check if file exists
    if !config_path.exists() {
        println!("  {HINT_EMOJI} {HINT}Not found{HINT:#}");
        return Ok(());
    }

    // Read and display the file contents
    let contents = std::fs::read_to_string(&config_path)
        .map_err(|e| GitError::CommandFailed(format!("Failed to read config file: {}", e)))?;

    if contents.trim().is_empty() {
        println!("  {HINT_EMOJI} {HINT}Empty file{HINT:#}");
        return Ok(());
    }

    // Display each line with indentation
    for line in contents.lines() {
        if !line.trim().is_empty() {
            println!("  {dim}{line}{dim:#}");
        } else {
            println!();
        }
    }

    Ok(())
}

fn get_global_config_path() -> Option<PathBuf> {
    let strategy = choose_base_strategy().ok()?;
    Some(strategy.config_dir().join("worktrunk").join("config.toml"))
}
