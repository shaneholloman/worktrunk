use anyhow::Context;
use skim::prelude::*;
use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use worktrunk::config::WorktrunkConfig;
use worktrunk::git::Repository;

use super::list::collect;
use super::list::model::ListItem;
use super::worktree::handle_switch;
use crate::output::handle_switch_output;

/// Preview modes for the interactive selector
///
/// Each mode shows a different aspect of the worktree:
/// 1. WorkingTree: Uncommitted changes (git diff HEAD --stat)
/// 2. History: Commit history since diverging from main (git log with merge-base)
/// 3. BranchDiff: Line diffs in commits ahead of main (git diff --stat main…)
///
/// Loosely aligned with `wt list` columns, though not a perfect match:
/// - Mode 1 corresponds to "HEAD±" column
/// - Mode 2 shows commits (related to "main↕" counts)
/// - Mode 3 corresponds to "main…± (--full)" column
///
/// TODO: Consider adding mode 4 "remote±" showing diff vs upstream tracking branch
/// (unpushed commits). Would align with "Remote⇅" column in `wt list`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewMode {
    WorkingTree = 1,
    History = 2,
    BranchDiff = 3,
}

impl PreviewMode {
    fn from_u8(n: u8) -> Self {
        match n {
            2 => Self::History,
            3 => Self::BranchDiff,
            _ => Self::WorkingTree,
        }
    }

    fn read_from_state() -> Self {
        let state_path = Self::state_path();
        fs::read_to_string(&state_path)
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .map(Self::from_u8)
            .unwrap_or(Self::WorkingTree)
    }

    fn state_path() -> PathBuf {
        // Use per-process temp file to avoid race conditions when running multiple instances
        std::env::temp_dir().join(format!("wt-select-mode-{}", std::process::id()))
    }
}

/// RAII wrapper for preview state file lifecycle management
struct PreviewState {
    path: PathBuf,
}

impl PreviewState {
    fn new() -> Self {
        let path = PreviewMode::state_path();
        let _ = fs::write(&path, "1");
        Self { path }
    }
}

impl Drop for PreviewState {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Header item for column names (non-selectable)
struct HeaderSkimItem {
    display_text: String,
    display_text_with_ansi: String,
}

impl SkimItem for HeaderSkimItem {
    fn text(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.display_text)
    }

    fn display<'a>(&'a self, _context: skim::DisplayContext<'a>) -> skim::AnsiString<'a> {
        skim::AnsiString::parse(&self.display_text_with_ansi)
    }

    fn output(&self) -> Cow<'_, str> {
        Cow::Borrowed("") // Headers produce no output if selected
    }
}

/// Wrapper to implement SkimItem for ListItem
struct WorktreeSkimItem {
    display_text: String,
    display_text_with_ansi: String,
    branch_name: String,
    item: Arc<ListItem>,
}

impl SkimItem for WorktreeSkimItem {
    fn text(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.display_text)
    }

    fn display<'a>(&'a self, _context: skim::DisplayContext<'a>) -> skim::AnsiString<'a> {
        skim::AnsiString::parse(&self.display_text_with_ansi)
    }

    fn output(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.branch_name)
    }

    fn preview(&self, context: PreviewContext<'_>) -> ItemPreview {
        let mode = PreviewMode::read_from_state();

        // Build preview: tabs header + content
        let mut result = Self::render_preview_tabs(mode);
        result.push_str(&self.preview_for_mode(mode, context.width));

        ItemPreview::AnsiText(result)
    }
}

impl WorktreeSkimItem {
    /// Render the tab header for the preview window
    ///
    /// Shows all preview modes as tabs, with the current mode bolded
    /// and unselected modes dimmed. Controls are shown below in dimmed text.
    fn render_preview_tabs(mode: PreviewMode) -> String {
        /// Format a tab label with bold (active) or dimmed (inactive) styling
        fn format_tab(label: &str, is_active: bool) -> String {
            use anstyle::Style;
            let style = if is_active {
                Style::new().bold()
            } else {
                Style::new().dimmed()
            };
            format!("{}{}{}", style.render(), label, style.render_reset())
        }

        let tab1 = format_tab("1: HEAD±", mode == PreviewMode::WorkingTree);
        let tab2 = format_tab("2: history", mode == PreviewMode::History);
        let tab3 = format_tab("3: main…±", mode == PreviewMode::BranchDiff);
        let controls = format_tab("ctrl-u/d: scroll | ctrl-p: preview", false);

        format!("{} | {} | {}\n{}\n\n", tab1, tab2, tab3, controls)
    }

    /// Render preview for the given mode with specified width
    fn preview_for_mode(&self, mode: PreviewMode, width: usize) -> String {
        match mode {
            PreviewMode::WorkingTree => self.render_working_tree_preview(width),
            PreviewMode::History => self.render_history_preview(width),
            PreviewMode::BranchDiff => self.render_branch_diff_preview(width),
        }
    }

    /// Common diff rendering pattern: check stat, show stat + full diff if non-empty
    fn render_diff_preview(&self, args: &[&str], no_changes_msg: &str, width: usize) -> String {
        let mut output = String::new();
        let repo = Repository::current();

        // Check stat output first
        let mut stat_args = args.to_vec();
        stat_args.push("--stat");
        stat_args.push("--color=always");
        let stat_width_arg = format!("--stat-width={}", width);
        stat_args.push(&stat_width_arg);

        if let Ok(stat) = repo.run_command(&stat_args)
            && !stat.trim().is_empty()
        {
            output.push_str(&stat);
            output.push_str("\n\n");

            // TODO: Re-enable user pager (delta, diff-so-fancy, etc.) for diff preview
            // Currently disabled because pagers can interfere with skim's TUI rendering.
            // When re-implementing, test with: GIT_PAGER=delta cargo run -- beta select
            // Consider: config option to disable pagers specifically for select preview
            let mut diff_args = args.to_vec();
            diff_args.push("--color=always");
            if let Ok(diff) = repo.run_command(&diff_args) {
                output.push_str(&diff);
            }
        } else {
            output.push_str(no_changes_msg);
            output.push('\n');
        }

        output
    }

    /// Render Mode 1: Working tree preview (uncommitted changes vs HEAD)
    /// Matches `wt list` "HEAD±" column
    fn render_working_tree_preview(&self, width: usize) -> String {
        let Some(wt_info) = self.item.worktree_data() else {
            // Branch without worktree - selecting will create one
            use worktrunk::styling::INFO_EMOJI;
            return format!("{INFO_EMOJI} Branch only — press Enter to create worktree\n");
        };

        let path = wt_info.path.display().to_string();
        self.render_diff_preview(
            &["-C", &path, "diff", "HEAD"],
            "No uncommitted changes",
            width,
        )
    }

    /// Render Mode 3: Branch diff preview (line diffs in commits ahead of main)
    /// Matches `wt list` "main…± (--full)" column
    fn render_branch_diff_preview(&self, width: usize) -> String {
        if self.item.counts().ahead == 0 {
            return "No commits ahead of main\n".to_string();
        }

        let merge_base = format!("main...{}", self.item.head());
        self.render_diff_preview(&["diff", &merge_base], "No changes vs main", width)
    }

    /// Render Mode 2: History preview
    fn render_history_preview(&self, _width: usize) -> String {
        const HISTORY_LIMIT: &str = "10";

        let mut output = String::new();
        let repo = Repository::current();
        let head = self.item.head();

        // Get merge-base with main
        //
        // Note on error handling: This code runs in an interactive preview pane that updates
        // on every keystroke. We intentionally use silent fallbacks rather than propagating
        // errors to avoid disruptive error messages during navigation. The preview is
        // supplementary - users can still select worktrees even if preview fails.
        //
        // Alternative: Check specific conditions (main branch exists, valid HEAD, etc.) before
        // running git commands. This would provide better diagnostics but adds latency to
        // every preview render. Trade-off: simplicity + speed vs. detailed error messages.
        let Ok(merge_base_output) = repo.run_command(&["merge-base", "main", head]) else {
            output.push_str("No commits\n");
            return output;
        };

        let merge_base = merge_base_output.trim();

        let branch = self.item.branch_name();
        let is_main = branch == "main" || branch == "master";

        if is_main {
            // Viewing main itself - show history without dimming
            if let Ok(log_output) = repo.run_command(&[
                "log",
                "--graph",
                "--decorate",
                "--oneline",
                "--color=always",
                "-n",
                HISTORY_LIMIT,
                head,
            ]) {
                output.push_str(&log_output);
            }
        } else {
            // Not on main - show bright commits not on main, dimmed commits on main

            // Part 1: Bright commits (merge-base..HEAD)
            let range = format!("{}..{}", merge_base, head);
            if let Ok(log_output) = repo.run_command(&[
                "log",
                "--graph",
                "--decorate",
                "--oneline",
                "--color=always",
                &range,
            ]) {
                output.push_str(&log_output);
            }

            // Part 2: Dimmed commits on main (history before merge-base)
            if let Ok(log_output) = repo.run_command(&[
                "log",
                "--graph",
                "--oneline",
                "--format=%C(dim)%h %s%C(reset)",
                "--color=always",
                "-n",
                HISTORY_LIMIT,
                merge_base,
            ]) {
                output.push_str(&log_output);
            }
        }

        output
    }
}

pub fn handle_select(is_directive_mode: bool) -> anyhow::Result<()> {
    let repo = Repository::current();

    // Initialize preview mode state file (auto-cleanup on drop)
    let _state = PreviewState::new();

    // Gather list data using simplified collection (buffered mode)
    let Some(list_data) = collect::collect(
        &repo, true,  // show_branches (include branches without worktrees)
        false, // show_remotes (local branches only, not remote branches)
        false, // show_full (no full layout needed)
        false, // fetch_ci (no CI with select command)
        false, // check_conflicts (no conflict checking with select command)
        false, // show_progress (no progress bars)
        false, // render_table (select renders its own UI)
    )?
    else {
        return Ok(());
    };

    // Get current worktree path for styling
    let _current_worktree_path = repo.worktree_root().ok();

    // Use the same layout system as `wt list` for proper column alignment
    // Skim uses ~50% of terminal width for the list (rest is preview), so calculate
    // layout based on available width to avoid truncation
    let terminal_width = super::list::layout::get_safe_list_width();
    let skim_list_width = terminal_width / 2;
    let layout = super::list::layout::calculate_layout_with_width(
        &list_data.items,
        false, // show_full
        false, // fetch_ci
        skim_list_width,
    );

    // Render header using layout system (need both plain and styled text for skim)
    let header_line = layout.render_header_line();
    let header_display_text = header_line.render();
    let header_plain_text = header_line.plain_text();

    // Convert to skim items using the layout system for rendering
    let mut items: Vec<Arc<dyn SkimItem>> = list_data
        .items
        .into_iter()
        .map(|item| {
            let branch_name = item.branch_name().to_string();

            // Use layout system to render the line - this handles all column alignment
            let rendered_line = layout.render_list_item_line(&item, None);
            let display_text_with_ansi = rendered_line.render();
            let display_text = rendered_line.plain_text();

            Arc::new(WorktreeSkimItem {
                display_text,
                display_text_with_ansi,
                branch_name,
                item: Arc::new(item),
            }) as Arc<dyn SkimItem>
        })
        .collect();

    // Insert header row at the beginning (will be non-selectable via header_lines option)
    items.insert(
        0,
        Arc::new(HeaderSkimItem {
            display_text: header_plain_text,
            display_text_with_ansi: header_display_text,
        }) as Arc<dyn SkimItem>,
    );

    // Get state path for key bindings
    let state_path_str = _state.path.display().to_string();

    // Configure skim options with Rust-based preview and mode switching keybindings
    let options = SkimOptionsBuilder::default()
        .height("90%".to_string())
        .layout("reverse".to_string())
        .header_lines(1) // Make first line (header) non-selectable
        .multi(false)
        .no_info(true) // Hide info line (matched/total counter)
        .preview(Some("".to_string())) // Enable preview (empty string means use SkimItem::preview())
        .preview_window("right:50%".to_string())
        .color(Some(
            "fg:-1,bg:-1,header:-1,matched:108,current:-1,current_bg:254,current_match:108"
                .to_string(),
        ))
        .bind(vec![
            // Mode switching
            format!(
                "1:execute-silent(echo 1 > {})+refresh-preview",
                state_path_str
            ),
            format!(
                "2:execute-silent(echo 2 > {})+refresh-preview",
                state_path_str
            ),
            format!(
                "3:execute-silent(echo 3 > {})+refresh-preview",
                state_path_str
            ),
            // Preview scrolling
            "ctrl-u:preview-page-up".to_string(),
            "ctrl-d:preview-page-down".to_string(),
            // Preview toggle (ctrl-p is universally supported, unlike ctrl-/)
            "ctrl-p:toggle-preview".to_string(),
        ])
        // Legend/controls moved to preview window tabs (render_preview_tabs)
        .no_clear(true) // Prevent skim from clearing screen, we'll do it manually
        .build()
        .map_err(|e| anyhow::anyhow!(format!("Failed to build skim options: {}", e)))?;

    // Create item receiver
    let (tx, rx): (SkimItemSender, SkimItemReceiver) = unbounded();
    for item in items {
        tx.send(item)
            .map_err(|e| anyhow::anyhow!(format!("Failed to send item to skim: {}", e)))?;
    }
    drop(tx);

    // Run skim
    let output = Skim::run_with(&options, Some(rx));

    // Handle selection
    if let Some(out) = output
        && !out.is_abort
        && let Some(selected) = out.selected_items.first()
    {
        // Get branch name or worktree path from selected item
        // (output() returns the worktree path for existing worktrees, branch name otherwise)
        let identifier = selected.output().to_string();

        // Load config
        let config = WorktrunkConfig::load().context("Failed to load config")?;

        // Switch to the selected worktree
        // handle_switch can handle both branch names and worktree paths
        let (result, resolved_branch) =
            handle_switch(&identifier, false, None, false, false, &config)?;

        // Clear the terminal screen after skim exits to prevent artifacts
        // Use stderr for terminal control sequences - in directive mode, stdout goes to a FIFO
        // for directive parsing, so terminal control must go through stderr to reach the TTY
        use crossterm::{execute, terminal};
        use std::io::stderr;
        execute!(stderr(), terminal::Clear(terminal::ClearType::All))?;
        execute!(stderr(), crossterm::cursor::MoveTo(0, 0))?;

        // Show success message; emit cd directive if in directive mode
        handle_switch_output(&result, &resolved_branch, false, is_directive_mode)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_mode_from_u8() {
        assert_eq!(PreviewMode::from_u8(1), PreviewMode::WorkingTree);
        assert_eq!(PreviewMode::from_u8(2), PreviewMode::History);
        assert_eq!(PreviewMode::from_u8(3), PreviewMode::BranchDiff);
        // Invalid values default to WorkingTree
        assert_eq!(PreviewMode::from_u8(0), PreviewMode::WorkingTree);
        assert_eq!(PreviewMode::from_u8(99), PreviewMode::WorkingTree);
    }

    #[test]
    fn test_preview_mode_state_file_read_default() {
        // When state file doesn't exist or is invalid, default to WorkingTree
        let state_path = PreviewMode::state_path();
        // Clean up any existing state
        let _ = fs::remove_file(&state_path);

        assert_eq!(PreviewMode::read_from_state(), PreviewMode::WorkingTree);
    }

    #[test]
    fn test_preview_mode_state_file_roundtrip() {
        // Use a unique test file to avoid conflicts with concurrent tests
        let test_state_path =
            std::env::temp_dir().join(format!("wt-select-mode-test-{}", std::process::id()));

        // Write mode 1 (WorkingTree)
        fs::write(&test_state_path, "1").unwrap();
        let mode = fs::read_to_string(&test_state_path)
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .map(PreviewMode::from_u8)
            .unwrap_or(PreviewMode::WorkingTree);
        assert_eq!(mode, PreviewMode::WorkingTree);

        // Write mode 2 (History)
        fs::write(&test_state_path, "2").unwrap();
        let mode = fs::read_to_string(&test_state_path)
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .map(PreviewMode::from_u8)
            .unwrap_or(PreviewMode::WorkingTree);
        assert_eq!(mode, PreviewMode::History);

        // Write mode 3 (BranchDiff)
        fs::write(&test_state_path, "3").unwrap();
        let mode = fs::read_to_string(&test_state_path)
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .map(PreviewMode::from_u8)
            .unwrap_or(PreviewMode::WorkingTree);
        assert_eq!(mode, PreviewMode::BranchDiff);

        // Cleanup
        let _ = fs::remove_file(&test_state_path);
    }
}
