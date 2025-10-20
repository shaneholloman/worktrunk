use crate::common::TestRepo;
use insta::Settings;
use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};
use std::process::Command;

/// Helper to create snapshot with normalized paths
fn snapshot_merge(test_name: &str, repo: &TestRepo, args: &[&str], cwd: Option<&std::path::Path>) {
    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");

    // Normalize paths
    settings.add_filter(repo.root_path().to_str().unwrap(), "[REPO]");
    for (name, path) in &repo.worktrees {
        settings.add_filter(
            path.to_str().unwrap(),
            &format!("[WORKTREE_{}]", name.to_uppercase().replace('-', "_")),
        );
    }

    // Normalize git SHAs
    settings.add_filter(r"\b[0-9a-f]{7,40}\b", "[SHA]");
    settings.add_filter(r"\\", "/");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("merge")
            .args(args)
            .current_dir(cwd.unwrap_or(repo.root_path()));

        assert_cmd_snapshot!(test_name, cmd);
    });
}

#[test]
fn test_merge_fast_forward() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    // Create a worktree for main
    let main_wt = repo.root_path().parent().unwrap().join("test-repo.main-wt");
    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["worktree", "add", main_wt.to_str().unwrap(), "main"])
        .current_dir(repo.root_path())
        .output()
        .expect("Failed to add worktree");

    // Create a feature worktree and make a commit
    let feature_wt = repo.add_worktree("feature", "feature");
    std::fs::write(feature_wt.join("feature.txt"), "feature content")
        .expect("Failed to write file");

    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["add", "feature.txt"])
        .current_dir(&feature_wt)
        .output()
        .expect("Failed to add file");

    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["commit", "-m", "Add feature file"])
        .current_dir(&feature_wt)
        .output()
        .expect("Failed to commit");

    // Merge feature into main
    snapshot_merge("merge_fast_forward", &repo, &["main"], Some(&feature_wt));
}

#[test]
fn test_merge_with_keep_flag() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    // Create a worktree for main
    let main_wt = repo.root_path().parent().unwrap().join("test-repo.main-wt");
    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["worktree", "add", main_wt.to_str().unwrap(), "main"])
        .current_dir(repo.root_path())
        .output()
        .expect("Failed to add worktree");

    // Create a feature worktree and make a commit
    let feature_wt = repo.add_worktree("feature", "feature");
    std::fs::write(feature_wt.join("feature.txt"), "feature content")
        .expect("Failed to write file");

    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["add", "feature.txt"])
        .current_dir(&feature_wt)
        .output()
        .expect("Failed to add file");

    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["commit", "-m", "Add feature file"])
        .current_dir(&feature_wt)
        .output()
        .expect("Failed to commit");

    // Merge with --keep flag (should not finish worktree)
    snapshot_merge(
        "merge_with_keep",
        &repo,
        &["main", "--keep"],
        Some(&feature_wt),
    );
}

#[test]
fn test_merge_already_on_target() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    // Already on main branch (repo root)
    snapshot_merge("merge_already_on_target", &repo, &[], None);
}

#[test]
fn test_merge_dirty_working_tree() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    // Create a feature worktree with uncommitted changes
    let feature_wt = repo.add_worktree("feature", "feature");
    std::fs::write(feature_wt.join("dirty.txt"), "uncommitted content")
        .expect("Failed to write file");

    // Try to merge (should fail due to dirty working tree)
    snapshot_merge(
        "merge_dirty_working_tree",
        &repo,
        &["main"],
        Some(&feature_wt),
    );
}

#[test]
fn test_merge_not_fast_forward() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    // Create commits in both branches
    // Add commit to main (repo root)
    std::fs::write(repo.root_path().join("main.txt"), "main content")
        .expect("Failed to write file");

    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["add", "main.txt"])
        .current_dir(repo.root_path())
        .output()
        .expect("Failed to add file");

    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["commit", "-m", "Add main file"])
        .current_dir(repo.root_path())
        .output()
        .expect("Failed to commit");

    // Create a feature worktree branching from before the main commit
    let feature_wt = repo.add_worktree("feature", "feature");
    std::fs::write(feature_wt.join("feature.txt"), "feature content")
        .expect("Failed to write file");

    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["add", "feature.txt"])
        .current_dir(&feature_wt)
        .output()
        .expect("Failed to add file");

    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["commit", "-m", "Add feature file"])
        .current_dir(&feature_wt)
        .output()
        .expect("Failed to commit");

    // Try to merge (should fail or require actual merge)
    snapshot_merge(
        "merge_not_fast_forward",
        &repo,
        &["main"],
        Some(&feature_wt),
    );
}

#[test]
fn test_merge_to_default_branch() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    // Create a worktree for main
    let main_wt = repo.root_path().parent().unwrap().join("test-repo.main-wt");
    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["worktree", "add", main_wt.to_str().unwrap(), "main"])
        .current_dir(repo.root_path())
        .output()
        .expect("Failed to add worktree");

    // Create a feature worktree and make a commit
    let feature_wt = repo.add_worktree("feature", "feature");
    std::fs::write(feature_wt.join("feature.txt"), "feature content")
        .expect("Failed to write file");

    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["add", "feature.txt"])
        .current_dir(&feature_wt)
        .output()
        .expect("Failed to add file");

    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["commit", "-m", "Add feature file"])
        .current_dir(&feature_wt)
        .output()
        .expect("Failed to commit");

    // Merge without specifying target (should use default branch)
    snapshot_merge("merge_to_default", &repo, &[], Some(&feature_wt));
}
