use crate::display::{find_common_prefix, get_terminal_width};
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthStr;

use super::WorktreeInfo;

/// Helper: Check if a column is "dense" (appears in >50% of all rows)
fn is_dense_for_all_rows(count: usize, total: usize) -> bool {
    count * 2 > total
}

/// Helper: Check if a column is "dense" for non-primary rows (appears in >50% of non-primary rows)
fn is_dense_for_non_primary(count: usize, non_primary_count: usize) -> bool {
    non_primary_count > 0 && count * 2 > non_primary_count
}

/// Helper: Try to allocate space for a column. Returns the allocated width if successful.
/// Updates `remaining` by subtracting the allocated width + spacing.
fn try_allocate(remaining: &mut usize, ideal_width: usize, spacing: usize) -> usize {
    if ideal_width == 0 || *remaining < ideal_width + spacing {
        return 0;
    }
    *remaining = remaining.saturating_sub(ideal_width + spacing);
    ideal_width
}

pub struct ColumnWidths {
    pub branch: usize,
    pub time: usize,
    pub message: usize,
    pub ahead_behind: usize,
    pub working_diff: usize,
    pub branch_diff: usize,
    pub upstream: usize,
    pub states: usize,
}

pub struct LayoutConfig {
    pub widths: ColumnWidths,
    pub ideal_widths: ColumnWidths, // Maximum widths for padding sparse columns
    pub common_prefix: PathBuf,
    pub max_message_len: usize,
}

pub fn calculate_column_widths(infos: &[WorktreeInfo]) -> ColumnWidths {
    let mut max_branch = 0;
    let mut max_time = 0;
    let mut max_message = 0;
    let mut max_ahead_behind = 0;
    let mut max_working_diff = 0;
    let mut max_branch_diff = 0;
    let mut max_upstream = 0;
    let mut max_states = 0;

    for info in infos {
        // Branch name
        let branch_len = info.branch.as_deref().unwrap_or("(detached)").width();
        max_branch = max_branch.max(branch_len);

        // Time
        let time_str = crate::display::format_relative_time(info.timestamp);
        max_time = max_time.max(time_str.width());

        // Message (truncate to 50 chars max)
        let msg_len = info.commit_message.chars().take(50).count();
        max_message = max_message.max(msg_len);

        // Ahead/behind
        if !info.is_primary && (info.ahead > 0 || info.behind > 0) {
            let ahead_behind_len = format!("↑{} ↓{}", info.ahead, info.behind).width();
            max_ahead_behind = max_ahead_behind.max(ahead_behind_len);
        }

        // Working tree diff
        let (wt_added, wt_deleted) = info.working_tree_diff;
        if wt_added > 0 || wt_deleted > 0 {
            let working_diff_len = format!("+{} -{}", wt_added, wt_deleted).width();
            max_working_diff = max_working_diff.max(working_diff_len);
        }

        // Branch diff
        if !info.is_primary {
            let (br_added, br_deleted) = info.branch_diff;
            if br_added > 0 || br_deleted > 0 {
                let branch_diff_len = format!("+{} -{}", br_added, br_deleted).width();
                max_branch_diff = max_branch_diff.max(branch_diff_len);
            }
        }

        // Upstream tracking
        if info.upstream_ahead > 0 || info.upstream_behind > 0 {
            let remote_name = info.upstream_remote.as_deref().unwrap_or("origin");
            let upstream_len = format!(
                "{} ↑{} ↓{}",
                remote_name, info.upstream_ahead, info.upstream_behind
            )
            .width();
            max_upstream = max_upstream.max(upstream_len);
        }

        // States (including worktree_state)
        let states = super::render::format_all_states(info);
        if !states.is_empty() {
            max_states = max_states.max(states.width());
        }
    }

    ColumnWidths {
        branch: max_branch,
        time: max_time,
        message: max_message,
        ahead_behind: max_ahead_behind,
        working_diff: max_working_diff,
        branch_diff: max_branch_diff,
        upstream: max_upstream,
        states: max_states,
    }
}

/// Calculate responsive layout based on terminal width
pub fn calculate_responsive_layout(infos: &[WorktreeInfo]) -> LayoutConfig {
    let terminal_width = get_terminal_width();
    let paths: Vec<&Path> = infos.iter().map(|info| info.path.as_path()).collect();
    let common_prefix = find_common_prefix(&paths);

    // Count how many rows have each sparse column
    let non_primary_count = infos.iter().filter(|i| !i.is_primary).count();
    let ahead_behind_count = infos
        .iter()
        .filter(|i| !i.is_primary && (i.ahead > 0 || i.behind > 0))
        .count();
    let working_diff_count = infos
        .iter()
        .filter(|i| {
            let (added, deleted) = i.working_tree_diff;
            added > 0 || deleted > 0
        })
        .count();
    let branch_diff_count = infos
        .iter()
        .filter(|i| {
            if i.is_primary {
                return false;
            }
            let (added, deleted) = i.branch_diff;
            added > 0 || deleted > 0
        })
        .count();
    let upstream_count = infos
        .iter()
        .filter(|i| i.upstream_ahead > 0 || i.upstream_behind > 0)
        .count();
    let states_count = infos
        .iter()
        .filter(|i| {
            i.worktree_state.is_some()
                || (i.detached && i.branch.is_some())
                || i.bare
                || i.locked.is_some()
                || i.prunable.is_some()
        })
        .count();

    // A column is "dense" if it appears in >50% of applicable rows
    // For ahead/behind and branch_diff, applicable = non-primary rows
    // For others, applicable = all rows
    let ahead_behind_is_dense = is_dense_for_non_primary(ahead_behind_count, non_primary_count);
    let working_diff_is_dense = is_dense_for_all_rows(working_diff_count, infos.len());
    let branch_diff_is_dense = is_dense_for_non_primary(branch_diff_count, non_primary_count);
    let upstream_is_dense = is_dense_for_all_rows(upstream_count, infos.len());
    let states_is_dense = is_dense_for_all_rows(states_count, infos.len());

    // Calculate ideal column widths
    let ideal_widths = calculate_column_widths(infos);

    // Essential columns (always shown):
    // - current indicator: 2 chars
    // - branch: variable
    // - short HEAD: 8 chars
    // - path: at least 20 chars (we'll use shortened paths)
    // - spacing: 2 chars between columns

    let spacing = 2;
    let current_indicator = 2;
    let short_head = 8;
    let min_path = 20;

    // Calculate base width needed
    let base_width =
        current_indicator + ideal_widths.branch + spacing + short_head + spacing + min_path;

    // Available width for optional columns
    let available = terminal_width.saturating_sub(base_width);

    // Priority order for columns (from high to low):
    // 1. time (15-20 chars)
    // 2. message (20-50 chars, flexible)
    // 3. ahead_behind - commits difference (if any worktree has it)
    // 4. branch_diff - line diff in commits (if any worktree has it)
    // 5. working_diff - line diff in working tree (if any worktree has it)
    // 6. upstream (if any worktree has it)
    // 7. states (if any worktree has it)

    let mut remaining = available;
    let mut widths = ColumnWidths {
        branch: ideal_widths.branch,
        time: 0,
        message: 0,
        ahead_behind: 0,
        working_diff: 0,
        branch_diff: 0,
        upstream: 0,
        states: 0,
    };

    // Time column (high priority, ~15 chars)
    widths.time = try_allocate(&mut remaining, ideal_widths.time, spacing);

    // Message column (flexible, 20-50 chars)
    let max_message_len = if remaining >= 50 + spacing {
        remaining = remaining.saturating_sub(50 + spacing);
        50
    } else if remaining >= 30 + spacing {
        let msg_len = remaining.saturating_sub(spacing).min(ideal_widths.message);
        remaining = remaining.saturating_sub(msg_len + spacing);
        msg_len
    } else if remaining >= 20 + spacing {
        let msg_len = 20;
        remaining = remaining.saturating_sub(msg_len + spacing);
        msg_len
    } else {
        0
    };

    if max_message_len > 0 {
        widths.message = max_message_len;
    }

    // Ahead/behind column (only if dense and fits)
    if ahead_behind_is_dense {
        widths.ahead_behind = try_allocate(&mut remaining, ideal_widths.ahead_behind, spacing);
    }

    // Working diff column (only if dense and fits)
    if working_diff_is_dense {
        widths.working_diff = try_allocate(&mut remaining, ideal_widths.working_diff, spacing);
    }

    // Branch diff column (only if dense and fits)
    if branch_diff_is_dense {
        widths.branch_diff = try_allocate(&mut remaining, ideal_widths.branch_diff, spacing);
    }

    // Upstream column (only if dense and fits)
    if upstream_is_dense {
        widths.upstream = try_allocate(&mut remaining, ideal_widths.upstream, spacing);
    }

    // States column (only if dense and fits)
    if states_is_dense {
        widths.states = try_allocate(&mut remaining, ideal_widths.states, spacing);
    }

    LayoutConfig {
        widths,
        ideal_widths,
        common_prefix,
        max_message_len,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_column_width_calculation_with_unicode() {
        use crate::commands::list::WorktreeInfo;

        let info1 = WorktreeInfo {
            path: PathBuf::from("/test"),
            head: "abc123".to_string(),
            branch: Some("main".to_string()),
            timestamp: 0,
            commit_message: "Test".to_string(),
            ahead: 3,
            behind: 2,
            working_tree_diff: (100, 50),
            branch_diff: (200, 30),
            is_primary: false,
            detached: false,
            bare: false,
            locked: None,
            prunable: None,
            upstream_remote: Some("origin".to_string()),
            upstream_ahead: 4,
            upstream_behind: 0,
            worktree_state: None,
        };

        let widths = calculate_column_widths(&[info1]);

        // "↑3 ↓2" has visual width 5 (not 9 bytes)
        assert_eq!(widths.ahead_behind, 5, "↑3 ↓2 should have width 5");

        // "+100 -50" has width 8
        assert_eq!(widths.working_diff, 8, "+100 -50 should have width 8");

        // "+200 -30" has width 8
        assert_eq!(widths.branch_diff, 8, "+200 -30 should have width 8");

        // "origin ↑4 ↓0" has visual width 12 (not more due to Unicode arrows)
        assert_eq!(widths.upstream, 12, "origin ↑4 ↓0 should have width 12");
    }
}
