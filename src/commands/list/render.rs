use crate::display::{StyledLine, format_relative_time, shorten_path, truncate_at_word_boundary};
use worktrunk::theme::Theme;

use super::WorktreeInfo;
use super::layout::LayoutConfig;

pub fn format_all_states(info: &WorktreeInfo) -> String {
    let mut states = Vec::new();

    // Worktree state (merge/rebase/etc)
    if let Some(ref state) = info.worktree_state {
        states.push(format!("[{}]", state));
    }

    // Don't show detached state if branch is None (already shown in branch column)
    if info.detached && info.branch.is_some() {
        states.push("(detached)".to_string());
    }
    if info.bare {
        states.push("(bare)".to_string());
    }
    if let Some(ref reason) = info.locked {
        if reason.is_empty() {
            states.push("(locked)".to_string());
        } else {
            states.push(format!("(locked: {})", reason));
        }
    }
    if let Some(ref reason) = info.prunable {
        if reason.is_empty() {
            states.push("(prunable)".to_string());
        } else {
            states.push(format!("(prunable: {})", reason));
        }
    }

    states.join(" ")
}

pub fn format_header_line(layout: &LayoutConfig) {
    let widths = &layout.widths;
    let theme = Theme::new();
    let dim_style = theme.dim;

    let mut line = StyledLine::new();

    // Branch
    let header = format!("{:width$}", "Branch", width = widths.branch);
    line.push_styled(header, dim_style);
    line.push_raw("  ");

    // Age (Time)
    if widths.time > 0 {
        let header = format!("{:width$}", "Age", width = widths.time);
        line.push_styled(header, dim_style);
        line.push_raw("  ");
    }

    // Ahead/behind (commits)
    if layout.ideal_widths.ahead_behind > 0 {
        let header = format!(
            "{:width$}",
            "Cmts",
            width = layout.ideal_widths.ahead_behind
        );
        line.push_styled(header, dim_style);
        line.push_raw("  ");
    }

    // Branch diff (line diff in commits)
    if layout.ideal_widths.branch_diff > 0 {
        let header = format!(
            "{:width$}",
            "Cmt +/-",
            width = layout.ideal_widths.branch_diff
        );
        line.push_styled(header, dim_style);
        line.push_raw("  ");
    }

    // Working tree diff
    if layout.ideal_widths.working_diff > 0 {
        let header = format!(
            "{:width$}",
            "WT +/-",
            width = layout.ideal_widths.working_diff
        );
        line.push_styled(header, dim_style);
        line.push_raw("  ");
    }

    // Upstream
    if layout.ideal_widths.upstream > 0 {
        let header = format!("{:width$}", "Remote", width = layout.ideal_widths.upstream);
        line.push_styled(header, dim_style);
        line.push_raw("  ");
    }

    // Commit (fixed width: 8 chars)
    line.push_styled("Commit  ", dim_style);
    line.push_raw("  ");

    // Message
    if widths.message > 0 {
        let header = format!("{:width$}", "Message", width = widths.message);
        line.push_styled(header, dim_style);
        line.push_raw("  ");
    }

    // States
    if layout.ideal_widths.states > 0 {
        let header = format!("{:width$}", "State", width = layout.ideal_widths.states);
        line.push_styled(header, dim_style);
        line.push_raw("  ");
    }

    // Path
    line.push_styled("Path", dim_style);

    println!("{}", line.render());
}

pub fn format_worktree_line(
    info: &WorktreeInfo,
    layout: &LayoutConfig,
    current_worktree_path: Option<&std::path::PathBuf>,
) {
    let widths = &layout.widths;
    let theme = Theme::new();
    let primary_style = theme.primary;
    let current_style = theme.current;
    let green_style = theme.addition;
    let red_style = theme.deletion;
    let yellow_style = theme.neutral;
    let dim_style = theme.dim;

    let branch_display = info.branch.as_deref().unwrap_or("(detached)");
    let short_head = &info.head[..8.min(info.head.len())];

    // Determine styles: current worktree is bold magenta, primary is cyan
    let is_current = current_worktree_path
        .map(|p| p == &info.path)
        .unwrap_or(false);
    let text_style = match (is_current, info.is_primary) {
        (true, _) => Some(current_style),
        (_, true) => Some(primary_style),
        _ => None,
    };

    // Start building the line
    let mut line = StyledLine::new();

    // Branch name
    let branch_text = format!("{:width$}", branch_display, width = widths.branch);
    if let Some(style) = text_style {
        line.push_styled(branch_text, style);
    } else {
        line.push_raw(branch_text);
    }
    line.push_raw("  ");

    // Age (Time)
    if widths.time > 0 {
        let time_str = format!(
            "{:width$}",
            format_relative_time(info.timestamp),
            width = widths.time
        );
        line.push_styled(time_str, dim_style);
        line.push_raw("  ");
    }

    // Ahead/behind (commits difference) - always reserve space if ANY row uses it
    if layout.ideal_widths.ahead_behind > 0 {
        if !info.is_primary && (info.ahead > 0 || info.behind > 0) {
            let ahead_behind_text = format!(
                "{:width$}",
                format!("↑{} ↓{}", info.ahead, info.behind),
                width = layout.ideal_widths.ahead_behind
            );
            line.push_styled(ahead_behind_text, yellow_style);
        } else {
            // No data for this row: pad with spaces
            line.push_raw(" ".repeat(layout.ideal_widths.ahead_behind));
        }
        line.push_raw("  ");
    }

    // Branch diff (line diff in commits) - always reserve space if ANY row uses it
    if layout.ideal_widths.branch_diff > 0 {
        if !info.is_primary {
            let (br_added, br_deleted) = info.branch_diff;
            if br_added > 0 || br_deleted > 0 {
                // Build the diff as a mini styled line
                let mut diff_segment = StyledLine::new();
                diff_segment.push_styled(format!("+{}", br_added), green_style);
                diff_segment.push_raw(" ");
                diff_segment.push_styled(format!("-{}", br_deleted), red_style);
                diff_segment.pad_to(layout.ideal_widths.branch_diff);
                // Append all segments from diff_segment to main line
                for segment in diff_segment.segments {
                    line.push(segment);
                }
            } else {
                // No data for this row: pad with spaces
                line.push_raw(" ".repeat(layout.ideal_widths.branch_diff));
            }
        } else {
            // Primary row: pad with spaces
            line.push_raw(" ".repeat(layout.ideal_widths.branch_diff));
        }
        line.push_raw("  ");
    }

    // Working tree diff (line diff in working tree) - always reserve space if ANY row uses it
    if layout.ideal_widths.working_diff > 0 {
        let (wt_added, wt_deleted) = info.working_tree_diff;
        if wt_added > 0 || wt_deleted > 0 {
            // Build the diff as a mini styled line
            let mut diff_segment = StyledLine::new();
            diff_segment.push_styled(format!("+{}", wt_added), green_style);
            diff_segment.push_raw(" ");
            diff_segment.push_styled(format!("-{}", wt_deleted), red_style);
            diff_segment.pad_to(layout.ideal_widths.working_diff);
            // Append all segments from diff_segment to main line
            for segment in diff_segment.segments {
                line.push(segment);
            }
        } else {
            // No data for this row: pad with spaces
            line.push_raw(" ".repeat(layout.ideal_widths.working_diff));
        }
        line.push_raw("  ");
    }

    // Upstream tracking - always reserve space if ANY row uses it
    if layout.ideal_widths.upstream > 0 {
        if info.upstream_ahead > 0 || info.upstream_behind > 0 {
            let remote_name = info.upstream_remote.as_deref().unwrap_or("origin");
            // Build the upstream as a mini styled line
            let mut upstream_segment = StyledLine::new();
            upstream_segment.push_styled(remote_name, dim_style);
            upstream_segment.push_raw(" ");
            upstream_segment.push_styled(format!("↑{}", info.upstream_ahead), green_style);
            upstream_segment.push_raw(" ");
            upstream_segment.push_styled(format!("↓{}", info.upstream_behind), red_style);
            upstream_segment.pad_to(layout.ideal_widths.upstream);
            // Append all segments from upstream_segment to main line
            for segment in upstream_segment.segments {
                line.push(segment);
            }
        } else {
            // No data for this row: pad with spaces
            line.push_raw(" ".repeat(layout.ideal_widths.upstream));
        }
        line.push_raw("  ");
    }

    // Commit (short HEAD, fixed width: 8 chars)
    if let Some(style) = text_style {
        line.push_styled(short_head, style);
    } else {
        line.push_raw(short_head);
    }
    line.push_raw("  ");

    // Message (left-aligned, truncated at word boundary)
    if widths.message > 0 {
        let msg = format!(
            "{:width$}",
            truncate_at_word_boundary(&info.commit_message, layout.max_message_len),
            width = widths.message
        );
        line.push_styled(msg, dim_style);
        line.push_raw("  ");
    }

    // States - always reserve space if ANY row uses it
    if layout.ideal_widths.states > 0 {
        let states = format_all_states(info);
        if !states.is_empty() {
            let states_text = format!("{:width$}", states, width = layout.ideal_widths.states);
            line.push_raw(states_text);
        } else {
            // No data for this row: pad with spaces
            line.push_raw(" ".repeat(layout.ideal_widths.states));
        }
        line.push_raw("  ");
    }

    // Path (no padding needed, it's the last column, use shortened path)
    let path_str = shorten_path(&info.path, &layout.common_prefix);
    if let Some(style) = text_style {
        line.push_styled(path_str, style);
    } else {
        line.push_raw(path_str);
    }

    println!("{}", line.render());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::list::WorktreeInfo;
    use crate::commands::list::layout::{ColumnWidths, LayoutConfig};
    use crate::display::{StyledLine, shorten_path};
    use std::path::PathBuf;

    #[test]
    fn test_column_alignment_with_all_columns() {
        // Create test data with all columns populated
        let info = WorktreeInfo {
            path: PathBuf::from("/test/path"),
            head: "abc12345".to_string(),
            branch: Some("test-branch".to_string()),
            timestamp: 0,
            commit_message: "Test message".to_string(),
            ahead: 3,
            behind: 2,
            working_tree_diff: (100, 50),
            branch_diff: (200, 30),
            is_primary: false,
            detached: false,
            bare: false,
            locked: Some("test lck".to_string()), // "(locked: test lck)" = 18 chars
            prunable: None,
            upstream_remote: Some("origin".to_string()),
            upstream_ahead: 4,
            upstream_behind: 0,
            worktree_state: None,
        };

        let layout = LayoutConfig {
            widths: ColumnWidths {
                branch: 11,
                time: 13,
                message: 12,
                ahead_behind: 5,
                working_diff: 8,
                branch_diff: 8,
                upstream: 12,
                states: 18,
            },
            ideal_widths: ColumnWidths {
                branch: 11,
                time: 13,
                message: 12,
                ahead_behind: 5,
                working_diff: 8,
                branch_diff: 8,
                upstream: 12,
                states: 18,
            },
            common_prefix: PathBuf::from("/test"),
            max_message_len: 12,
        };

        // Build header line manually (mimicking format_header_line logic)
        let mut header = StyledLine::new();
        header.push_raw(format!("{:width$}", "Branch", width = layout.widths.branch));
        header.push_raw("  ");
        header.push_raw(format!("{:width$}", "Age", width = layout.widths.time));
        header.push_raw("  ");
        header.push_raw(format!(
            "{:width$}",
            "Cmts",
            width = layout.ideal_widths.ahead_behind
        ));
        header.push_raw("  ");
        header.push_raw(format!(
            "{:width$}",
            "Cmt +/-",
            width = layout.ideal_widths.branch_diff
        ));
        header.push_raw("  ");
        header.push_raw(format!(
            "{:width$}",
            "WT +/-",
            width = layout.ideal_widths.working_diff
        ));
        header.push_raw("  ");
        header.push_raw(format!(
            "{:width$}",
            "Remote",
            width = layout.ideal_widths.upstream
        ));
        header.push_raw("  ");
        header.push_raw("Commit  ");
        header.push_raw("  ");
        header.push_raw(format!(
            "{:width$}",
            "Message",
            width = layout.widths.message
        ));
        header.push_raw("  ");
        header.push_raw(format!(
            "{:width$}",
            "State",
            width = layout.ideal_widths.states
        ));
        header.push_raw("  ");
        header.push_raw("Path");

        // Build data line manually (mimicking format_worktree_line logic)
        let mut data = StyledLine::new();
        data.push_raw(format!(
            "{:width$}",
            "test-branch",
            width = layout.widths.branch
        ));
        data.push_raw("  ");
        data.push_raw(format!(
            "{:width$}",
            "9 months ago",
            width = layout.widths.time
        ));
        data.push_raw("  ");
        // Ahead/behind
        let ahead_behind_text = format!(
            "{:width$}",
            "↑3 ↓2",
            width = layout.ideal_widths.ahead_behind
        );
        data.push_raw(ahead_behind_text);
        data.push_raw("  ");
        // Branch diff
        let mut branch_diff_segment = StyledLine::new();
        branch_diff_segment.push_raw("+200 -30");
        branch_diff_segment.pad_to(layout.ideal_widths.branch_diff);
        for seg in branch_diff_segment.segments {
            data.push(seg);
        }
        data.push_raw("  ");
        // Working diff
        let mut working_diff_segment = StyledLine::new();
        working_diff_segment.push_raw("+100 -50");
        working_diff_segment.pad_to(layout.ideal_widths.working_diff);
        for seg in working_diff_segment.segments {
            data.push(seg);
        }
        data.push_raw("  ");
        // Upstream
        let mut upstream_segment = StyledLine::new();
        upstream_segment.push_raw("origin ↑4 ↓0");
        upstream_segment.pad_to(layout.ideal_widths.upstream);
        for seg in upstream_segment.segments {
            data.push(seg);
        }
        data.push_raw("  ");
        // Commit (fixed 8 chars)
        data.push_raw("abc12345");
        data.push_raw("  ");
        // Message
        data.push_raw(format!(
            "{:width$}",
            "Test message",
            width = layout.widths.message
        ));
        data.push_raw("  ");
        // State
        let states = format_all_states(&info);
        data.push_raw(format!(
            "{:width$}",
            states,
            width = layout.ideal_widths.states
        ));
        data.push_raw("  ");
        // Path
        data.push_raw(shorten_path(&info.path, &layout.common_prefix));

        // Verify both lines have columns at the same positions
        // We'll check this by verifying specific column start positions
        let header_str = header.render();
        let data_str = data.render();

        // Remove ANSI codes for position checking (our test data doesn't have styles anyway)
        assert!(header_str.contains("Branch"));
        assert!(data_str.contains("test-branch"));

        // The key test: both lines should have the same visual width up to "Path" column
        // (Path is variable width, so we only check up to there)
        let header_width_without_path = header.width() - "Path".len();
        let data_width_without_path =
            data.width() - shorten_path(&info.path, &layout.common_prefix).len();

        assert_eq!(
            header_width_without_path, data_width_without_path,
            "Header and data rows should have same width before Path column"
        );
    }
}
