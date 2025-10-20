use crate::common::TestRepo;
use insta_cmd::get_cargo_bin;
use rstest::rstest;
use std::process::Command;

/// Helper to check if a shell is available on the system
fn is_shell_available(shell: &str) -> bool {
    Command::new("which")
        .arg(shell)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Execute a shell script in the given shell and return stdout
fn execute_shell_script(repo: &TestRepo, shell: &str, script: &str) -> String {
    let mut cmd = Command::new(shell);
    repo.clean_cli_env(&mut cmd);

    // Additional shell-specific isolation to prevent user config interference
    cmd.env_remove("BASH_ENV");
    cmd.env_remove("ENV"); // for sh/dash
    cmd.env_remove("ZDOTDIR"); // for zsh

    // Prevent loading user config files
    if shell == "fish" {
        cmd.arg("--no-config");
    }

    let output = cmd
        .arg("-c")
        .arg(script)
        .current_dir(repo.root_path())
        .output()
        .unwrap_or_else(|e| panic!("Failed to execute {} script: {}", shell, e));

    if !output.status.success() {
        panic!(
            "Shell script failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout).expect("Invalid UTF-8 in output")
}

/// Generate shell integration code for the given shell
fn generate_init_code(repo: &TestRepo, shell: &str) -> String {
    let mut cmd = Command::new(get_cargo_bin("wt"));
    repo.clean_cli_env(&mut cmd);

    let output = cmd
        .args(["init", shell])
        .current_dir(repo.root_path())
        .output()
        .expect("Failed to generate init code");

    if !output.status.success() {
        panic!(
            "Failed to generate init code:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout).expect("Invalid UTF-8 in init code")
}

/// Generate shell-specific PATH export syntax
fn path_export_syntax(shell: &str, bin_path: &str) -> String {
    match shell {
        "fish" => format!(r#"set -x PATH {} $PATH"#, bin_path),
        _ => format!(r#"export PATH="{}:$PATH""#, bin_path),
    }
}

#[rstest]
#[case("bash")]
#[case("fish")]
#[case("zsh")]
fn test_e2e_switch_changes_directory(#[case] shell: &str) {
    if !is_shell_available(shell) {
        eprintln!("Skipping test: {} not available", shell);
        return;
    }

    let repo = TestRepo::new();
    repo.commit("Initial commit");

    let init_code = generate_init_code(&repo, shell);
    let bin_path = get_cargo_bin("wt")
        .parent()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let script = format!(
        r#"
        {}
        {}
        wt switch --create my-feature
        pwd
        "#,
        path_export_syntax(shell, &bin_path),
        init_code
    );

    let output = execute_shell_script(&repo, shell, &script);

    // Verify that pwd shows we're in a worktree directory containing "my-feature"
    assert!(
        output.contains("my-feature"),
        "Expected pwd to show my-feature worktree, got: {}",
        output
    );
}

#[rstest]
#[case("bash")]
#[case("fish")]
#[case("zsh")]
fn test_e2e_finish_returns_to_main(#[case] shell: &str) {
    if !is_shell_available(shell) {
        eprintln!("Skipping test: {} not available", shell);
        return;
    }

    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    let init_code = generate_init_code(&repo, shell);
    let repo_path = repo.root_path().to_string_lossy().to_string();
    let bin_path = get_cargo_bin("wt")
        .parent()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let script = format!(
        r#"
        {}
        {}
        wt switch --create my-feature
        wt finish
        pwd
        "#,
        path_export_syntax(shell, &bin_path),
        init_code
    );

    let output = execute_shell_script(&repo, shell, &script);

    // Verify that pwd shows we're back in the main repo directory
    assert!(
        output.trim().ends_with(&repo_path),
        "Expected pwd to show main repo at {}, got: {}",
        repo_path,
        output
    );
}

#[test]
fn test_bash_e2e_switch_preserves_output() {
    if !is_shell_available("bash") {
        eprintln!("Skipping test: bash not available");
        return;
    }

    let repo = TestRepo::new();
    repo.commit("Initial commit");

    let init_code = generate_init_code(&repo, "bash");

    let script = format!(
        r#"
        export PATH="{}:$PATH"
        {}
        wt switch --create test-branch 2>&1
        "#,
        get_cargo_bin("wt").parent().unwrap().to_string_lossy(),
        init_code
    );

    let output = execute_shell_script(&repo, "bash", &script);

    // Verify that user-facing output is preserved (not just directives)
    assert!(
        output.contains("test-branch") || output.contains("Created") || output.contains("Switched"),
        "Expected informative output, got: {}",
        output
    );
    // Verify directives are NOT shown to user
    assert!(
        !output.contains("__WORKTRUNK_CD__"),
        "Directives should not be visible to user, got: {}",
        output
    );
}

#[test]
fn test_bash_e2e_custom_prefix() {
    if !is_shell_available("bash") {
        eprintln!("Skipping test: bash not available");
        return;
    }

    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Generate init code with custom prefix
    let mut cmd = Command::new(get_cargo_bin("wt"));
    repo.clean_cli_env(&mut cmd);
    let output = cmd
        .args(["init", "bash", "--cmd", "custom"])
        .current_dir(repo.root_path())
        .output()
        .expect("Failed to generate init code");

    let init_code = String::from_utf8(output.stdout).expect("Invalid UTF-8 in init code");

    // Test that custom switch works
    let script = format!(
        r#"
        export PATH="{}:$PATH"
        {}
        custom switch --create my-feature
        pwd
        "#,
        get_cargo_bin("wt").parent().unwrap().to_string_lossy(),
        init_code
    );

    let output = execute_shell_script(&repo, "bash", &script);

    // Verify that pwd shows we're in a worktree directory
    assert!(
        output.contains("my-feature"),
        "Expected pwd to show my-feature worktree with custom prefix, got: {}",
        output
    );
}

#[test]
fn test_bash_e2e_error_handling() {
    if !is_shell_available("bash") {
        eprintln!("Skipping test: bash not available");
        return;
    }

    let repo = TestRepo::new();
    repo.commit("Initial commit");

    let init_code = generate_init_code(&repo, "bash");

    // Try to switch to a branch twice (should error on second attempt)
    let script = format!(
        r#"
        export PATH="{}:$PATH"
        {}
        wt switch --create test-feature
        wt switch --create test-feature 2>&1 || echo "ERROR_CAUGHT"
        "#,
        get_cargo_bin("wt").parent().unwrap().to_string_lossy(),
        init_code
    );

    let output = execute_shell_script(&repo, "bash", &script);

    // Verify that error is caught and handled
    assert!(
        output.contains("ERROR_CAUGHT")
            || output.contains("already exists")
            || output.contains("error"),
        "Expected error output when switching to same branch twice, got: {}",
        output
    );
}

#[rstest]
#[case("bash")]
#[case("fish")]
fn test_e2e_prompt_hook(#[case] shell: &str) {
    if !is_shell_available(shell) {
        eprintln!("Skipping test: {} not available", shell);
        return;
    }

    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Generate init code with prompt hook
    let mut cmd = Command::new(get_cargo_bin("wt"));
    repo.clean_cli_env(&mut cmd);
    let output = cmd
        .args(["init", shell, "--hook", "prompt"])
        .current_dir(repo.root_path())
        .output()
        .expect("Failed to generate init code");

    let init_code = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    let bin_path = get_cargo_bin("wt")
        .parent()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // Verify prompt hook function exists and can be called
    let script = format!(
        r#"
        {}
        {}
        type _wt_prompt_hook 2>&1
        _wt_prompt_hook 2>&1
        echo "HOOK_EXECUTED"
        "#,
        path_export_syntax(shell, &bin_path),
        init_code
    );

    let output = execute_shell_script(&repo, shell, &script);

    // Verify hook function exists
    assert!(
        output.contains("_wt_prompt_hook is a function") || output.contains("function"),
        "Expected prompt hook function to be defined, got: {}",
        output
    );

    // Verify hook executed successfully
    assert!(
        output.contains("HOOK_EXECUTED"),
        "Expected prompt hook to execute without error, got: {}",
        output
    );
}

#[test]
fn test_bash_e2e_switch_to_existing_worktree() {
    if !is_shell_available("bash") {
        eprintln!("Skipping test: bash not available");
        return;
    }

    let repo = TestRepo::new();
    repo.commit("Initial commit");

    let init_code = generate_init_code(&repo, "bash");

    // Create worktree, move away, then switch back to it (without --create)
    let script = format!(
        r#"
        export PATH="{}:$PATH"
        {}
        wt switch --create existing-branch
        pwd
        cd /tmp
        wt switch existing-branch
        pwd
        "#,
        get_cargo_bin("wt").parent().unwrap().to_string_lossy(),
        init_code
    );

    let output = execute_shell_script(&repo, "bash", &script);

    // Should show the existing-branch path twice (once after creation, once after switching back)
    let count = output.matches("existing-branch").count();
    assert!(
        count >= 2,
        "Expected to see existing-branch path at least twice, got: {}",
        output
    );
}

#[test]
fn test_bash_e2e_multiple_switches() {
    if !is_shell_available("bash") {
        eprintln!("Skipping test: bash not available");
        return;
    }

    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    let init_code = generate_init_code(&repo, "bash");

    // Test that multiple switches work
    let script = format!(
        r#"
        export PATH="{}:$PATH"
        {}
        wt switch --create test-branch
        pwd
        wt finish
        pwd
        "#,
        get_cargo_bin("wt").parent().unwrap().to_string_lossy(),
        init_code
    );

    let output = execute_shell_script(&repo, "bash", &script);

    // Should have switched to test-branch
    assert!(
        output.contains("test-branch"),
        "Expected wt switch to work, got: {}",
        output
    );

    // Should have returned to main (wt finish should work)
    assert!(
        output.contains("main"),
        "Expected wt finish to work, got: {}",
        output
    );
}
