use crate::common::{TestRepo, make_snapshot_cmd, setup_snapshot_settings};
use insta_cmd::assert_cmd_snapshot;
use std::fs;

/// Helper to create snapshot with normalized paths and SHAs
fn snapshot_switch(test_name: &str, repo: &TestRepo, args: &[&str]) {
    let settings = setup_snapshot_settings(repo);
    settings.bind(|| {
        let mut cmd = make_snapshot_cmd(repo, "switch", args, None);
        assert_cmd_snapshot!(test_name, cmd);
    });
}

#[test]
fn test_post_start_commands_no_config() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Switch without project config should work normally
    snapshot_switch("post_start_no_config", &repo, &["--create", "feature"]);
}

#[test]
fn test_post_start_commands_empty_array() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create empty project config
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(config_dir.join("wt.toml"), "post-start-commands = []\n")
        .expect("Failed to write config");

    repo.commit("Add empty config");

    // Should work without prompting
    snapshot_switch("post_start_empty_array", &repo, &["--create", "feature"]);
}

#[test]
fn test_post_start_commands_with_approval() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create project config with a simple command
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-start-commands = ["echo 'Setup complete'"]"#,
    )
    .expect("Failed to write config");

    repo.commit("Add config");

    // Pre-approve the command by setting up the user config
    // This simulates the command being already approved
    let home_dir = std::env::var("HOME").unwrap();
    let user_config_dir =
        std::path::Path::new(&home_dir).join("Library/Application Support/worktrunk");
    fs::create_dir_all(&user_config_dir).ok();
    fs::write(
        user_config_dir.join("config.toml"),
        format!(
            r#"worktree-path = "../{{repo}}.{{branch}}"

[[approved-commands]]
project = "{}"
command = "echo 'Setup complete'"
"#,
            repo.root_path().file_name().unwrap().to_str().unwrap()
        ),
    )
    .ok();

    // Command should execute without prompting
    snapshot_switch("post_start_with_approval", &repo, &["--create", "feature"]);
}

#[test]
fn test_post_start_commands_invalid_toml() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create invalid TOML
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        "post-start-commands = [invalid syntax\n",
    )
    .expect("Failed to write config");

    repo.commit("Add invalid config");

    // Should continue without executing commands, showing warning
    snapshot_switch("post_start_invalid_toml", &repo, &["--create", "feature"]);
}

#[test]
fn test_post_start_commands_failing_command() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create project config with a command that will fail
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-start-commands = ["exit 1"]"#,
    )
    .expect("Failed to write config");

    repo.commit("Add config with failing command");

    // Pre-approve the command
    let home_dir = std::env::var("HOME").unwrap();
    let config_dir = std::path::Path::new(&home_dir).join("Library/Application Support/worktrunk");
    fs::create_dir_all(&config_dir).ok();
    fs::write(
        config_dir.join("config.toml"),
        format!(
            r#"worktree-path = "../{{repo}}.{{branch}}"

[[approved-commands]]
project = "{}"
command = "exit 1"
"#,
            repo.root_path().file_name().unwrap().to_str().unwrap()
        ),
    )
    .ok();

    // Should show warning but continue (worktree should still be created)
    snapshot_switch(
        "post_start_failing_command",
        &repo,
        &["--create", "feature"],
    );
}

#[test]
fn test_post_start_commands_multiple_commands() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create project config with multiple commands
    let config_dir = repo.root_path().join(".config");
    fs::create_dir_all(&config_dir).expect("Failed to create .config dir");
    fs::write(
        config_dir.join("wt.toml"),
        r#"post-start-commands = ["echo 'First'", "echo 'Second'"]"#,
    )
    .expect("Failed to write config");

    repo.commit("Add config with multiple commands");

    // Pre-approve both commands
    let home_dir = std::env::var("HOME").unwrap();
    let config_dir = std::path::Path::new(&home_dir).join("Library/Application Support/worktrunk");
    fs::create_dir_all(&config_dir).ok();
    fs::write(
        config_dir.join("config.toml"),
        format!(
            r#"worktree-path = "../{{repo}}.{{branch}}"

[[approved-commands]]
project = "{}"
command = "echo 'First'"

[[approved-commands]]
project = "{}"
command = "echo 'Second'"
"#,
            repo.root_path().file_name().unwrap().to_str().unwrap(),
            repo.root_path().file_name().unwrap().to_str().unwrap()
        ),
    )
    .ok();

    // Both commands should execute
    snapshot_switch(
        "post_start_multiple_commands",
        &repo,
        &["--create", "feature"],
    );
}
