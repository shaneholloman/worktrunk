use anstyle::Style;
use clap::{CommandFactory, Parser, Subcommand};
use std::process;
use worktrunk::config::WorktrunkConfig;
use worktrunk::git::GitError;
use worktrunk::styling::println;

mod commands;
mod display;
mod llm;
mod output;

use commands::{
    ConfigAction, Shell, handle_complete, handle_completion, handle_config_init,
    handle_config_list, handle_configure_shell, handle_init, handle_list, handle_merge,
    handle_push, handle_remove, handle_switch,
};
use output::{handle_remove_output, handle_switch_output};

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table format
    Table,
    /// JSON format
    Json,
}

#[derive(Parser)]
#[command(name = "wt")]
#[command(about = "Git worktree management", long_about = None)]
#[command(version = env!("VERGEN_GIT_DESCRIBE"))]
#[command(disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Initialize global configuration file with examples
    Init,
    /// List all configuration files and their locations
    List,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate shell integration code
    Init {
        /// Shell to generate code for (bash, fish, zsh)
        shell: String,

        /// Command prefix (default: wt)
        #[arg(long, default_value = "wt")]
        cmd: String,
    },

    /// Configure shell by writing to config files
    ConfigureShell {
        /// Specific shell to configure (default: all shells with existing config files)
        #[arg(long, value_enum)]
        shell: Option<Shell>,

        /// Command prefix (default: wt)
        #[arg(long, default_value = "wt")]
        cmd: String,

        /// Skip confirmation prompt
        #[arg(long, short)]
        yes: bool,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigCommand,
    },

    /// List all worktrees
    List {
        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,

        /// Also display branches that don't have worktrees
        #[arg(long)]
        branches: bool,
    },

    /// Switch to a worktree
    Switch {
        /// Branch name or worktree path
        branch: String,

        /// Create a new branch
        #[arg(short = 'c', long)]
        create: bool,

        /// Base branch to create from (only with --create)
        #[arg(short = 'b', long)]
        base: Option<String>,

        /// Execute command after switching
        #[arg(short = 'x', long)]
        execute: Option<String>,

        /// Skip confirmation prompt
        #[arg(short = 'f', long)]
        force: bool,

        /// Skip executing post-start commands from project config
        #[arg(long)]
        no_config_commands: bool,

        /// Use internal mode (outputs directives for shell wrapper)
        #[arg(long, hide = true)]
        internal: bool,
    },

    /// Finish current worktree, returning to primary if current
    Remove {
        /// Use internal mode (outputs directives for shell wrapper)
        #[arg(long, hide = true)]
        internal: bool,
    },

    /// Push changes between worktrees
    Push {
        /// Target branch (defaults to default branch)
        target: Option<String>,

        /// Allow pushing merge commits (non-linear history)
        #[arg(long)]
        allow_merge_commits: bool,
    },

    /// Merge worktree into target branch
    Merge {
        /// Target branch to merge into (defaults to default branch)
        target: Option<String>,

        /// Squash all commits into one before merging
        #[arg(short, long)]
        squash: bool,

        /// Keep worktree after merging (don't remove)
        #[arg(short, long)]
        keep: bool,

        /// Custom instruction for commit message generation
        #[arg(short = 'm', long)]
        message: Option<String>,

        /// Use internal mode (outputs directives for shell wrapper)
        #[arg(long, hide = true)]
        internal: bool,
    },

    /// Generate shell completion script (deprecated - use init instead)
    #[command(hide = true)]
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Internal completion helper (hidden)
    #[command(hide = true)]
    Complete {
        /// Arguments to complete
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { shell, cmd } => {
            let mut cli_cmd = Cli::command();
            handle_init(&shell, &cmd, &mut cli_cmd).map_err(GitError::CommandFailed)
        }
        Commands::ConfigureShell { shell, cmd, yes } => {
            handle_configure_shell(shell, &cmd, yes)
                .map(|results| {
                    use anstyle::{AnsiColor, Color};

                    // Count actual changes (not AlreadyExists)
                    let changes_count = results
                        .iter()
                        .filter(|r| !matches!(r.action, ConfigAction::AlreadyExists))
                        .count();

                    if changes_count == 0 {
                        // All shells already configured
                        let green = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
                        println!("✅ {green}All shells already configured{green:#}");
                        return;
                    }

                    // Show what was done
                    for result in &results {
                        let bold = Style::new().bold();
                        let shell = result.shell;
                        let path = result.path.display();
                        println!(
                            "{} {bold}{shell}{bold:#} {path}",
                            result.action.description(),
                        );
                        // Indent each line of the config content with dim/gray color
                        for line in result.config_line.lines() {
                            let dim = Style::new().dimmed();
                            println!("  {dim}{line}{dim:#}");
                        }
                    }

                    // Success summary
                    println!();
                    let green = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
                    let plural = if changes_count == 1 { "" } else { "s" };
                    println!("✅ {green}Configured {changes_count} shell{plural}{green:#}");

                    // Show hint about restarting shell
                    println!();
                    use worktrunk::styling::{HINT, HINT_EMOJI};
                    println!(
                        "{HINT_EMOJI} {HINT}Restart your shell or run: source <config-file>{HINT:#}"
                    );
                })
                .map_err(GitError::CommandFailed)
        }
        Commands::Config { action } => match action {
            ConfigCommand::Init => handle_config_init(),
            ConfigCommand::List => handle_config_list(),
        },
        Commands::List { format, branches } => handle_list(format, branches),
        Commands::Switch {
            branch,
            create,
            base,
            execute,
            force,
            no_config_commands,
            internal,
        } => WorktrunkConfig::load()
            .map_err(|e| GitError::CommandFailed(format!("Failed to load config: {}", e)))
            .and_then(|config| {
                handle_switch(
                    &branch,
                    create,
                    base.as_deref(),
                    force,
                    no_config_commands,
                    &config,
                )
                .and_then(|result| {
                    handle_switch_output(&result, &branch, execute.as_deref(), internal)
                })
            }),
        Commands::Remove { internal } => {
            handle_remove().and_then(|result| handle_remove_output(&result, internal))
        }
        Commands::Push {
            target,
            allow_merge_commits,
        } => handle_push(target.as_deref(), allow_merge_commits),
        Commands::Merge {
            target,
            squash,
            keep,
            message,
            internal,
        } => handle_merge(
            target.as_deref(),
            squash,
            keep,
            message.as_deref(),
            internal,
        ),
        Commands::Completion { shell } => {
            let mut cli_cmd = Cli::command();
            handle_completion(shell, &mut cli_cmd);
            Ok(())
        }
        Commands::Complete { args } => handle_complete(args),
    };

    if let Err(e) = result {
        // Error messages are already formatted with emoji and colors
        eprintln!("{}", e);
        process::exit(1);
    }
}
// test change
