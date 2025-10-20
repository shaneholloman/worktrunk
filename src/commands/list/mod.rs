mod layout;
mod render;

use rayon::prelude::*;
use worktrunk::git::{GitError, Repository};

use layout::calculate_responsive_layout;
use render::{format_header_line, format_worktree_line};

pub struct WorktreeInfo {
    pub path: std::path::PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub timestamp: i64,
    pub commit_message: String,
    pub ahead: usize,
    pub behind: usize,
    pub working_tree_diff: (usize, usize),
    pub branch_diff: (usize, usize),
    pub is_primary: bool,
    pub detached: bool,
    pub bare: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
    pub upstream_remote: Option<String>,
    pub upstream_ahead: usize,
    pub upstream_behind: usize,
    pub worktree_state: Option<String>,
}

impl WorktreeInfo {
    /// Create WorktreeInfo from a Worktree, enriching it with git metadata
    fn from_worktree(wt: &worktrunk::git::Worktree, primary: &worktrunk::git::Worktree) -> Self {
        let wt_repo = Repository::at(&wt.path);
        let is_primary = wt.path == primary.path;

        // Get commit timestamp
        let timestamp = wt_repo.commit_timestamp(&wt.head).unwrap_or(0);

        // Get commit message
        let commit_message = wt_repo.commit_message(&wt.head).unwrap_or_default();

        // Calculate ahead/behind relative to primary branch (only if primary has a branch)
        let (ahead, behind) = if is_primary {
            (0, 0)
        } else if let Some(pb) = primary.branch.as_deref() {
            wt_repo.ahead_behind(pb, &wt.head).unwrap_or((0, 0))
        } else {
            (0, 0)
        };
        let working_tree_diff = wt_repo.working_tree_diff_stats().unwrap_or((0, 0));

        // Get branch diff stats (downstream of primary, only if primary has a branch)
        let branch_diff = if is_primary {
            (0, 0)
        } else if let Some(pb) = primary.branch.as_deref() {
            wt_repo.branch_diff_stats(pb, &wt.head).unwrap_or((0, 0))
        } else {
            (0, 0)
        };

        // Get upstream tracking info
        let (upstream_remote, upstream_ahead, upstream_behind) = match wt
            .branch
            .as_ref()
            .and_then(|b| wt_repo.upstream_branch(b).ok().flatten())
        {
            Some(upstream_branch) => {
                // Extract remote name from "origin/main" -> "origin"
                let remote = upstream_branch
                    .split_once('/')
                    .map(|(remote, _)| remote)
                    .unwrap_or("origin")
                    .to_string();
                let (ahead, behind) = wt_repo
                    .ahead_behind(&upstream_branch, &wt.head)
                    .unwrap_or((0, 0));
                (Some(remote), ahead, behind)
            }
            None => (None, 0, 0),
        };

        // Get worktree state (merge/rebase/etc)
        let worktree_state = wt_repo.worktree_state().unwrap_or(None);

        WorktreeInfo {
            path: wt.path.clone(),
            head: wt.head.clone(),
            branch: wt.branch.clone(),
            timestamp,
            commit_message,
            ahead,
            behind,
            working_tree_diff,
            branch_diff,
            is_primary,
            detached: wt.detached,
            bare: wt.bare,
            locked: wt.locked.clone(),
            prunable: wt.prunable.clone(),
            upstream_remote,
            upstream_ahead,
            upstream_behind,
            worktree_state,
        }
    }
}

pub fn handle_list() -> Result<(), GitError> {
    let repo = Repository::current();
    let worktrees = repo.list_worktrees()?;

    if worktrees.is_empty() {
        return Ok(());
    }

    // First worktree is the primary
    let primary = &worktrees[0];

    // Get current worktree to identify active one
    let current_worktree_path = repo.worktree_root().ok();

    // Gather enhanced information for all worktrees in parallel
    //
    // Parallelization strategy: Use Rayon to process worktrees concurrently.
    // Each worktree requires ~5 git operations (timestamp, ahead/behind, diffs).
    //
    // Benchmark results: See benches/list.rs for sequential vs parallel comparison.
    //
    // Decision: Always use parallel for simplicity and 2+ worktree performance.
    // Rayon overhead (~1-2ms) is acceptable for single-worktree case.
    //
    // TODO: Could parallelize the 5 git commands within each worktree if needed,
    // but worktree-level parallelism provides the best cost/benefit tradeoff
    let mut infos: Vec<WorktreeInfo> = if std::env::var("WT_SEQUENTIAL").is_ok() {
        // Sequential iteration (for benchmarking)
        worktrees
            .iter()
            .map(|wt| WorktreeInfo::from_worktree(wt, primary))
            .collect()
    } else {
        // Parallel iteration (default)
        worktrees
            .par_iter()
            .map(|wt| WorktreeInfo::from_worktree(wt, primary))
            .collect()
    };

    // Sort by most recent commit (descending)
    infos.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    // Calculate responsive layout based on terminal width
    let layout = calculate_responsive_layout(&infos);

    // Display header
    format_header_line(&layout);

    // Display formatted output
    for info in &infos {
        format_worktree_line(info, &layout, current_worktree_path.as_ref());
    }

    Ok(())
}
