use worktrunk::HookType;
use worktrunk::config::{ProjectConfig, WorktrunkConfig};
use worktrunk::git::{GitError, GitResultExt, Repository};
use worktrunk::styling::{
    AnstyleStyle, CYAN, CYAN_BOLD, HINT, HINT_EMOJI, WARNING, WARNING_EMOJI,
    format_bash_with_gutter, format_with_gutter,
};

use super::command_executor::{CommandContext, prepare_project_commands};
use super::worktree::{handle_push, handle_remove, parse_diff_shortstat};
use crate::output::execute_command_in_worktree;

/// Extract untracked files from git status --porcelain output
fn get_untracked_files(status_output: &str) -> Vec<String> {
    let mut untracked = Vec::new();

    for line in status_output.lines() {
        // Git status --porcelain format: XY filename
        // Untracked files have "??" status
        if let Some(filename) = line.strip_prefix("?? ") {
            untracked.push(filename.to_string());
        }
    }

    untracked
}

/// Warn about untracked files being auto-staged
fn show_untracked_warning(repo: &Repository) -> Result<(), GitError> {
    let status = repo
        .run_command(&["status", "--porcelain"])
        .git_context("Failed to get status")?;
    let untracked = get_untracked_files(&status);

    if untracked.is_empty() {
        return Ok(());
    }

    // Format file list (comma-separated)
    let file_list = untracked.join(", ");

    crate::output::progress(format!(
        "{WARNING_EMOJI} {WARNING}Auto-staging untracked files: {file_list}{WARNING:#}"
    ))?;

    Ok(())
}

pub fn handle_merge(
    target: Option<&str>,
    squash_enabled: bool,
    keep: bool,
    no_hooks: bool,
    force: bool,
    tracked_only: bool,
) -> Result<(), GitError> {
    let repo = Repository::current();

    // Get current branch
    let current_branch = repo.current_branch()?.ok_or(GitError::DetachedHead)?;

    // Get target branch (default to default branch if not provided)
    let target_branch = target.map_or_else(|| repo.default_branch(), |b| Ok(b.to_string()))?;

    // Check if already on target branch
    if current_branch == target_branch {
        use worktrunk::styling::{GREEN, SUCCESS_EMOJI};
        let green_bold = GREEN.bold();
        crate::output::success(format!(
            "{SUCCESS_EMOJI} {GREEN}Already on {green_bold}{target_branch}{green_bold:#}, nothing to merge{GREEN:#}"
        ))?;
        return Ok(());
    }

    // Load config for LLM integration
    let config = WorktrunkConfig::load().git_context("Failed to load config")?;

    // Handle uncommitted changes depending on whether we're squashing
    if repo.is_dirty()? {
        if squash_enabled {
            // Warn about untracked files before staging
            if !tracked_only {
                show_untracked_warning(&repo)?;
            }

            if tracked_only {
                repo.run_command(&["add", "-u"])
                    .git_context("Failed to stage tracked changes")?;
            } else {
                repo.run_command(&["add", "-A"])
                    .git_context("Failed to stage changes")?;
            }
        } else {
            // Commit immediately when not squashing
            handle_commit_changes(
                &config.commit_generation,
                &current_branch,
                no_hooks,
                force,
                tracked_only,
            )?;
        }
    }

    // Squash commits if enabled
    if squash_enabled {
        handle_squash(&target_branch, no_hooks, force)?;
    }

    // Rebase onto target (delegate to atomic dev command)
    super::dev::handle_dev_rebase(Some(&target_branch))?;

    // Run pre-merge checks unless --no-hooks was specified
    // Do this AFTER rebase to validate the final state that will be pushed
    if !no_hooks && let Ok(Some(project_config)) = ProjectConfig::load(&repo.worktree_root()?) {
        let worktree_path =
            std::env::current_dir().git_context("Failed to get current directory")?;
        run_pre_merge_commands(
            &project_config,
            &current_branch,
            &target_branch,
            &worktree_path,
            &repo,
            &config,
            force,
        )?;
    }

    // Fast-forward push to target branch (reuse handle_push logic)
    handle_push(Some(&target_branch), false, "Merged to")?;

    // Get primary worktree path before cleanup (while we can still run git commands)
    let primary_worktree_dir = repo.main_worktree_root()?;

    // Finish worktree unless --keep was specified
    if !keep {
        // STEP 1: Check for uncommitted changes before attempting cleanup
        // This prevents showing "Cleaning up worktree..." before failing
        repo.ensure_clean_working_tree()?;

        // STEP 2: Emit CD directive (just sets intent, doesn't actually change CWD)
        crate::output::change_directory(&primary_worktree_dir)?;

        // STEP 3: Switch to target branch in primary worktree (fails safely if there's an issue)
        let primary_repo = Repository::at(&primary_worktree_dir);
        let new_branch = primary_repo.current_branch()?;
        if new_branch.as_deref() != Some(&target_branch) {
            crate::output::progress(format!(
                "🔄 {CYAN}Switching to {CYAN:#}{CYAN_BOLD}{target_branch}{CYAN_BOLD:#}{CYAN}...{CYAN:#}"
            ))?;
            primary_repo
                .run_command(&["switch", &target_branch])
                .git_context(&format!("Failed to switch to '{}'", target_branch))?;
        }

        // STEP 4: Only NOW remove the worktree (after all checks passed)
        crate::output::progress(format!("🔄 {CYAN}Cleaning up worktree...{CYAN:#}"))?;
        handle_remove(None)?;

        // Print comprehensive summary
        crate::output::progress("")?;
        handle_merge_summary_output(Some(&primary_worktree_dir))?;
    } else {
        // Print comprehensive summary (worktree preserved)
        crate::output::progress("")?;
        handle_merge_summary_output(None)?;
    }

    // Execute post-merge commands in the main worktree
    // This runs after cleanup so the context is clear to the user
    // Create a fresh Repository instance at the primary worktree (the old repo may be invalid)
    let primary_repo = Repository::at(&primary_worktree_dir);
    execute_post_merge_commands(
        &primary_worktree_dir,
        &primary_repo,
        &config,
        &current_branch,
        &target_branch,
        force,
    )?;

    Ok(())
}

/// Format the merge summary message (includes emoji and color for consistency)
fn format_merge_summary(primary_path: Option<&std::path::Path>) -> String {
    use worktrunk::styling::{GREEN, SUCCESS_EMOJI};
    let green_bold = GREEN.bold();

    // Show where we ended up
    if let Some(path) = primary_path {
        format!(
            "{SUCCESS_EMOJI} {GREEN}Returned to primary at {green_bold}{}{green_bold:#}{GREEN:#}",
            path.display()
        )
    } else {
        format!("{SUCCESS_EMOJI} {GREEN}Kept worktree (use 'wt remove' to clean up){GREEN:#}")
    }
}

/// Handle output for merge summary using global output context
fn handle_merge_summary_output(primary_path: Option<&std::path::Path>) -> Result<(), GitError> {
    let message = format_merge_summary(primary_path);

    // Show success message (formatting added by OutputContext)
    crate::output::success(message)?;

    // Flush output
    crate::output::flush()?;

    Ok(())
}

/// Format a commit message with the first line in bold, ready for gutter display
pub fn format_commit_message_for_display(message: &str) -> String {
    let bold = AnstyleStyle::new().bold();
    let lines: Vec<&str> = message.lines().collect();

    if lines.is_empty() {
        return String::new();
    }

    // Format first line in bold
    let mut result = format!("{bold}{}{bold:#}", lines[0]);

    // Add remaining lines without bold
    if lines.len() > 1 {
        for line in &lines[1..] {
            result.push('\n');
            result.push_str(line);
        }
    }

    result
}

/// Show hint if no LLM command is configured
pub fn show_llm_config_hint_if_needed(
    commit_generation_config: &worktrunk::config::CommitGenerationConfig,
) -> Result<(), GitError> {
    // Check if LLM is NOT configured (matching llm.rs logic)
    let is_configured = commit_generation_config
        .command
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    if !is_configured {
        crate::output::hint(format!(
            "{HINT_EMOJI} {HINT}Using fallback commit message. Run 'wt config help' to configure LLM-generated messages{HINT:#}"
        ))?;
    }
    Ok(())
}

/// Commit already-staged changes with an LLM-generated message
pub fn commit_with_generated_message(
    progress_msg: &str,
    commit_generation_config: &worktrunk::config::CommitGenerationConfig,
) -> Result<(), GitError> {
    let repo = Repository::current();

    // Get diff stats for staged changes
    let diff_shortstat = repo
        .run_command(&["diff", "--staged", "--shortstat"])
        .unwrap_or_default();
    let stats = parse_diff_shortstat(&diff_shortstat);
    let stats_parts = stats.format_summary();

    // Format progress message with stats
    // Don't nest styles - stats already contain ADDITION/DELETION colors
    let full_progress_msg = match stats_parts.is_empty() {
        true => format!("🔄 {CYAN}{progress_msg}{CYAN:#}"),
        false => format!(
            "🔄 {CYAN}{}{CYAN:#} ({})",
            progress_msg,
            stats_parts.join(", ")
        ),
    };

    crate::output::progress(full_progress_msg)?;
    crate::output::progress(format!("🔄 {CYAN}Generating commit message...{CYAN:#}"))?;

    show_llm_config_hint_if_needed(commit_generation_config)?;
    let commit_message = crate::llm::generate_commit_message(commit_generation_config)?;

    let formatted_message = format_commit_message_for_display(&commit_message);
    crate::output::progress(format_with_gutter(&formatted_message, "", None))?;

    repo.run_command(&["commit", "-m", &commit_message])
        .git_context("Failed to commit")?;

    // Get commit hash for display
    let commit_hash = repo
        .run_command(&["rev-parse", "--short", "HEAD"])?
        .trim()
        .to_string();

    use worktrunk::styling::{GREEN, HINT, SUCCESS_EMOJI};
    crate::output::success(format!(
        "{SUCCESS_EMOJI} {GREEN}Committed changes{GREEN:#} @ {HINT}{commit_hash}{HINT:#}"
    ))?;

    Ok(())
}

/// Commit uncommitted changes with LLM-generated message
fn handle_commit_changes(
    commit_generation_config: &worktrunk::config::CommitGenerationConfig,
    current_branch: &str,
    no_hooks: bool,
    force: bool,
    tracked_only: bool,
) -> Result<(), GitError> {
    let repo = Repository::current();
    let config = WorktrunkConfig::load().git_context("Failed to load config")?;

    // Run pre-commit hook unless --no-hooks was specified
    if !no_hooks && let Ok(Some(project_config)) = ProjectConfig::load(&repo.worktree_root()?) {
        let worktree_path =
            std::env::current_dir().git_context("Failed to get current directory")?;
        run_pre_commit_commands(
            &project_config,
            current_branch,
            &worktree_path,
            &repo,
            &config,
            force,
        )?;
    }

    // Warn about untracked files before staging (only if using git add -A)
    if !tracked_only {
        show_untracked_warning(&repo)?;
    }

    // Stage changes
    if tracked_only {
        repo.run_command(&["add", "-u"])
            .git_context("Failed to stage tracked changes")?;
    } else {
        repo.run_command(&["add", "-A"])
            .git_context("Failed to stage changes")?;
    }

    commit_with_generated_message("Committing changes...", commit_generation_config)
}

fn handle_squash(
    target_branch: &str,
    no_hooks: bool,
    force: bool,
) -> Result<Option<usize>, GitError> {
    // Delegate to the atomic dev command
    super::dev::handle_dev_squash(Some(target_branch), force, no_hooks)?;
    Ok(None)
}

/// Run pre-merge commands sequentially (blocking, fail-fast)
pub fn run_pre_merge_commands(
    project_config: &ProjectConfig,
    current_branch: &str,
    target_branch: &str,
    worktree_path: &std::path::Path,
    repo: &Repository,
    config: &WorktrunkConfig,
    force: bool,
) -> Result<(), GitError> {
    let Some(pre_merge_config) = &project_config.pre_merge_command else {
        return Ok(());
    };

    let repo_root = repo.main_worktree_root()?;
    let ctx = CommandContext::new(
        repo,
        config,
        current_branch,
        worktree_path,
        &repo_root,
        force,
    );
    let commands = prepare_project_commands(
        pre_merge_config,
        &ctx,
        false,
        &[("target", target_branch)],
        "Pre-merge commands",
        |_name, command| {
            let dim = AnstyleStyle::new().dimmed();
            crate::output::progress(format!("{dim}Skipping pre-merge command: {command}{dim:#}"))
                .ok();
        },
    )?;
    for prepared in commands {
        let label = crate::commands::format_command_label("pre-merge", prepared.name.as_deref());
        crate::output::progress(format!("🔄 {CYAN}{label}{CYAN:#}"))?;
        crate::output::progress(format_bash_with_gutter(&prepared.expanded, ""))?;

        if let Err(e) = execute_command_in_worktree(worktree_path, &prepared.expanded) {
            return Err(GitError::HookCommandFailed {
                hook_type: HookType::PreMerge,
                command_name: prepared.name.clone(),
                error: e.to_string(),
            });
        }

        // No need to flush here - the redirect in execute_command_in_worktree ensures ordering
    }

    Ok(())
}

/// Execute post-merge commands sequentially in the main worktree (blocking)
pub fn execute_post_merge_commands(
    main_worktree_path: &std::path::Path,
    repo: &Repository,
    config: &WorktrunkConfig,
    branch: &str,
    target_branch: &str,
    force: bool,
) -> Result<(), GitError> {
    use worktrunk::styling::WARNING;

    // Load project config from the main worktree path directly
    let project_config = match ProjectConfig::load(main_worktree_path)
        .git_context("Failed to load project config")?
    {
        Some(cfg) => cfg,
        None => return Ok(()),
    };

    let Some(post_merge_config) = &project_config.post_merge_command else {
        return Ok(());
    };

    let ctx = CommandContext::new(
        repo,
        config,
        branch,
        main_worktree_path,
        main_worktree_path,
        force,
    );
    let commands = prepare_project_commands(
        post_merge_config,
        &ctx,
        false,
        &[("target", target_branch)],
        "Post-merge commands",
        |_name, command| {
            let dim = AnstyleStyle::new().dimmed();
            crate::output::progress(format!("{dim}Skipping command: {command}{dim:#}")).ok();
        },
    )?;

    if commands.is_empty() {
        return Ok(());
    }

    // Execute each command sequentially in the main worktree
    for prepared in commands {
        let label = crate::commands::format_command_label("post-merge", prepared.name.as_deref());
        crate::output::progress(format!("🔄 {CYAN}{label}{CYAN:#}"))?;
        crate::output::progress(format_bash_with_gutter(&prepared.expanded, ""))?;

        if let Err(e) = execute_command_in_worktree(main_worktree_path, &prepared.expanded) {
            use worktrunk::styling::WARNING_EMOJI;
            let warning_bold = WARNING.bold();
            let message = match &prepared.name {
                Some(name) => format!(
                    "{WARNING_EMOJI} {WARNING}Command {warning_bold}{name}{warning_bold:#} failed: {e}{WARNING:#}"
                ),
                None => format!("{WARNING_EMOJI} {WARNING}Command failed: {e}{WARNING:#}"),
            };
            crate::output::progress(message)?;
            // Continue with other commands even if one fails
        }
    }

    crate::output::flush()?;

    Ok(())
}

/// Run pre-commit commands sequentially (blocking, fail-fast)
pub fn run_pre_commit_commands(
    project_config: &ProjectConfig,
    current_branch: &str,
    worktree_path: &std::path::Path,
    repo: &Repository,
    config: &WorktrunkConfig,
    force: bool,
) -> Result<(), GitError> {
    let Some(pre_commit_config) = &project_config.pre_commit_command else {
        return Ok(());
    };

    let repo_root = repo.main_worktree_root()?;
    let ctx = CommandContext::new(
        repo,
        config,
        current_branch,
        worktree_path,
        &repo_root,
        force,
    );
    let commands = prepare_project_commands(
        pre_commit_config,
        &ctx,
        false,
        &[],
        "Pre-commit commands",
        |_name, command| {
            let dim = AnstyleStyle::new().dimmed();
            crate::output::progress(format!("{dim}Skipping command: {command}{dim:#}")).ok();
        },
    )?;

    if commands.is_empty() {
        return Ok(());
    }

    // Execute each command sequentially
    for prepared in commands {
        let label = crate::commands::format_command_label("pre-commit", prepared.name.as_deref());
        crate::output::progress(format!("🔄 {CYAN}{label}{CYAN:#}"))?;
        crate::output::progress(format_bash_with_gutter(&prepared.expanded, ""))?;

        if let Err(e) = execute_command_in_worktree(worktree_path, &prepared.expanded) {
            return Err(GitError::HookCommandFailed {
                hook_type: HookType::PreCommit,
                command_name: prepared.name.clone(),
                error: e.to_string(),
            });
        }
    }

    crate::output::flush()?;

    Ok(())
}

/// Run pre-squash commands sequentially (blocking, fail-fast)
pub fn run_pre_squash_commands(
    project_config: &ProjectConfig,
    current_branch: &str,
    target_branch: &str,
    worktree_path: &std::path::Path,
    repo: &Repository,
    config: &WorktrunkConfig,
    force: bool,
) -> Result<(), GitError> {
    let Some(pre_squash_config) = &project_config.pre_squash_command else {
        return Ok(());
    };

    let repo_root = repo.main_worktree_root()?;
    let ctx = CommandContext::new(
        repo,
        config,
        current_branch,
        worktree_path,
        &repo_root,
        force,
    );
    let commands = prepare_project_commands(
        pre_squash_config,
        &ctx,
        false,
        &[("target", target_branch)],
        "Pre-squash commands",
        |_name, command| {
            let dim = AnstyleStyle::new().dimmed();
            crate::output::progress(format!("{dim}Skipping command: {command}{dim:#}")).ok();
        },
    )?;

    if commands.is_empty() {
        return Ok(());
    }

    // Execute each command sequentially
    for prepared in commands {
        let label = crate::commands::format_command_label("pre-squash", prepared.name.as_deref());
        crate::output::progress(format!("🔄 {CYAN}{label}{CYAN:#}"))?;
        crate::output::progress(format_bash_with_gutter(&prepared.expanded, ""))?;

        if let Err(e) = execute_command_in_worktree(worktree_path, &prepared.expanded) {
            return Err(GitError::HookCommandFailed {
                hook_type: HookType::PreSquash,
                command_name: prepared.name.clone(),
                error: e.to_string(),
            });
        }
    }

    crate::output::flush()?;

    Ok(())
}
