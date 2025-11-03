// Benchmarks for `wt list` command
//
// This suite includes both synthetic and real-world repository benchmarks:
//
// 1. Synthetic benchmarks (fast, deterministic):
//    - bench_list_by_worktree_count: Varies worktree count (1-8)
//    - bench_list_by_repo_profile: Tests minimal/typical/large repo sizes
//    - bench_sequential_vs_parallel: Compares sequential vs parallel implementations
//
// 2. Real repository benchmarks (slower, more realistic):
//    - bench_list_real_repo: Uses rust-lang/rust repo (cloned to target/bench-repos/)
//
// Run all benchmarks:
//   cargo bench --bench list
//
// Run specific benchmark:
//   cargo bench --bench list bench_list_by_worktree_count
//   cargo bench --bench list bench_list_real_repo
//
// Note: Real repo benchmarks will clone rust-lang/rust on first run (~2-5 minutes).
// The clone is cached in target/bench-repos/ and reused across runs.
//
// CI benchmarks are not included because:
// - The expensive operations (GitHub/GitLab API calls) are network-dependent
// - CI detection (env var checking) is trivial (<1μs overhead)
// - Output formatting differences are minimal
// - Mocking APIs for reproducible benchmarks would be complex and not representative

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Benchmark configuration profiles representing different repo sizes
struct BenchmarkProfile {
    name: &'static str,
    commits: usize,
    files: usize,
    commits_ahead: usize,
    commits_behind: usize,
    uncommitted_files: usize,
}

const PROFILES: &[BenchmarkProfile] = &[
    BenchmarkProfile {
        name: "minimal",
        commits: 10,
        files: 10,
        commits_ahead: 0,
        commits_behind: 0, // Skip for now - causes git checkout issues
        uncommitted_files: 0,
    },
    BenchmarkProfile {
        name: "typical",
        commits: 500,
        files: 100,
        commits_ahead: 10,
        commits_behind: 0, // Skip for now - causes git checkout issues
        uncommitted_files: 3,
    },
    BenchmarkProfile {
        name: "large",
        commits: 1000,
        files: 200,
        commits_ahead: 50,
        commits_behind: 0, // Skip for now - causes git checkout issues
        uncommitted_files: 10,
    },
];

fn run_git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Git command failed: {:?}\nstderr: {}\nstdout: {}\npath: {}",
        args,
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
        path.display()
    );
}

/// Build release binary and return path.
/// This is idempotent - cargo skips rebuild if already up-to-date.
fn get_release_binary() -> PathBuf {
    let build_output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "Failed to build release binary: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    std::env::current_dir().unwrap().join("target/release/wt")
}

/// Create a realistic repository with actual commit history and file changes
fn create_realistic_repo(commits: usize, files: usize) -> TempDir {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path().join("main");
    std::fs::create_dir(&repo_path).unwrap();

    // Initialize repository
    run_git(&repo_path, &["init", "-b", "main"]);
    run_git(&repo_path, &["config", "user.name", "Benchmark"]);
    run_git(&repo_path, &["config", "user.email", "bench@test.com"]);

    // Create initial file structure
    for i in 0..files {
        let file_path = repo_path.join(format!("src/file_{}.rs", i));
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        let content = format!(
            "// File {}\n\
             pub struct Module{} {{\n\
                 data: Vec<String>,\n\
             }}\n\n\
             pub fn function_{}() -> i32 {{\n\
                 {}\n\
             }}\n",
            i,
            i,
            i,
            i * 42
        );
        std::fs::write(&file_path, content).unwrap();
    }

    run_git(&repo_path, &["add", "."]);
    run_git(&repo_path, &["commit", "-m", "Initial commit"]);

    // Build commit history with realistic diffs
    for i in 1..commits {
        // Modify 2-3 files per commit for realistic git operations
        let num_files_to_modify = 2 + (i % 2);
        for j in 0..num_files_to_modify {
            let file_idx = (i * 7 + j * 13) % files; // Pseudo-random file selection
            let file_path = repo_path.join(format!("src/file_{}.rs", file_idx));
            let mut content = std::fs::read_to_string(&file_path).unwrap();
            content.push_str(&format!(
                "\npub fn function_{}_{}() -> i32 {{\n    {}\n}}\n",
                file_idx,
                i,
                i * 100 + j
            ));
            std::fs::write(&file_path, content).unwrap();
        }

        run_git(&repo_path, &["add", "."]);
        run_git(&repo_path, &["commit", "-m", &format!("Commit {}", i)]);
    }

    temp_dir
}

/// Add a worktree with diverged branch and uncommitted changes
fn add_worktree_with_divergence(
    temp_dir: &TempDir,
    repo_path: &Path,
    wt_num: usize,
    commits_ahead: usize,
    commits_behind: usize,
    uncommitted_files: usize,
) {
    let branch = format!("feature-{}", wt_num);
    let wt_path = temp_dir.path().join(format!("wt-{}", wt_num));

    // Get current HEAD to diverge from
    let head_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    let base_commit = String::from_utf8_lossy(&head_output.stdout)
        .trim()
        .to_string();

    // Create worktree at current HEAD
    run_git(
        repo_path,
        &[
            "worktree",
            "add",
            "-b",
            &branch,
            wt_path.to_str().unwrap(),
            &base_commit,
        ],
    );

    // Add diverging commits in worktree (creates "ahead" status)
    for i in 0..commits_ahead {
        let file_path = wt_path.join(format!("feature_{}_file_{}.txt", wt_num, i));
        let content = format!(
            "Feature {} content {}\n\
             This is a realistic file with multiple lines\n\
             to make git diff operations non-trivial.\n",
            wt_num, i
        );
        std::fs::write(&file_path, content).unwrap();
        run_git(&wt_path, &["add", "."]);
        run_git(
            &wt_path,
            &["commit", "-m", &format!("Feature {} commit {}", wt_num, i)],
        );
    }

    // Add uncommitted changes (exercises git diff HEAD)
    for i in 0..uncommitted_files {
        let file_path = wt_path.join(format!("uncommitted_{}.txt", i));
        std::fs::write(&file_path, "Uncommitted content\n").unwrap();
    }

    // Add commits to main branch (creates "behind" status for worktree)
    if commits_behind > 0 {
        // Ensure we're on the main branch
        run_git(repo_path, &["checkout", "main"]);

        for i in 0..commits_behind {
            let file_path = repo_path.join(format!("main_advance_{}.txt", i));
            std::fs::write(&file_path, format!("Main content {}\n", i)).unwrap();
            run_git(repo_path, &["add", "."]);
            run_git(repo_path, &["commit", "-m", &format!("Main advance {}", i)]);
        }
    }
}

fn bench_list_by_worktree_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_by_worktree_count");

    let binary = get_release_binary();

    // Use "typical" profile for this benchmark
    let profile = &PROFILES[1];

    // Test with different worktree counts to find crossover point
    for num_worktrees in [1, 2, 3, 4, 6, 8] {
        // Setup repo ONCE per worktree count (wt list is read-only, so reuse is safe)
        let temp = create_realistic_repo(profile.commits, profile.files);
        let repo_path = temp.path().join("main");

        // Add worktrees with divergence
        for i in 1..num_worktrees {
            add_worktree_with_divergence(
                &temp,
                &repo_path,
                i,
                profile.commits_ahead,
                profile.commits_behind,
                profile.uncommitted_files,
            );
        }

        // Warm up git's internal caches
        run_git(&repo_path, &["status"]);

        group.bench_with_input(
            BenchmarkId::from_parameter(num_worktrees),
            &num_worktrees,
            |b, _| {
                b.iter(|| {
                    Command::new(&binary)
                        .arg("list")
                        .current_dir(&repo_path)
                        .output()
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_list_by_repo_profile(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_by_profile");

    let binary = get_release_binary();

    // Fixed worktree count to isolate repo size impact
    let num_worktrees = 4;

    for profile in PROFILES {
        // Setup repo ONCE per profile (wt list is read-only)
        let temp = create_realistic_repo(profile.commits, profile.files);
        let repo_path = temp.path().join("main");

        for i in 1..num_worktrees {
            add_worktree_with_divergence(
                &temp,
                &repo_path,
                i,
                profile.commits_ahead,
                profile.commits_behind,
                profile.uncommitted_files,
            );
        }

        run_git(&repo_path, &["status"]);

        group.bench_with_input(
            BenchmarkId::from_parameter(profile.name),
            profile,
            |b, _profile| {
                b.iter(|| {
                    Command::new(&binary)
                        .arg("list")
                        .current_dir(&repo_path)
                        .output()
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_sequential_vs_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_vs_parallel");

    let binary = get_release_binary();

    let profile = &PROFILES[1]; // typical profile

    // Test both sequential and parallel implementations across different worktree counts
    for num_worktrees in [1, 2, 3, 4, 6, 8] {
        let temp = create_realistic_repo(profile.commits, profile.files);
        let repo_path = temp.path().join("main");

        for i in 1..num_worktrees {
            add_worktree_with_divergence(
                &temp,
                &repo_path,
                i,
                profile.commits_ahead,
                profile.commits_behind,
                profile.uncommitted_files,
            );
        }

        run_git(&repo_path, &["status"]);

        // Benchmark parallel implementation (default)
        group.bench_with_input(
            BenchmarkId::new("parallel", num_worktrees),
            &num_worktrees,
            |b, _| {
                b.iter(|| {
                    Command::new(&binary)
                        .arg("list")
                        .current_dir(&repo_path)
                        .output()
                        .unwrap();
                });
            },
        );

        // Benchmark sequential implementation (via WT_SEQUENTIAL env var)
        group.bench_with_input(
            BenchmarkId::new("sequential", num_worktrees),
            &num_worktrees,
            |b, _| {
                b.iter(|| {
                    Command::new(&binary)
                        .arg("list")
                        .env("WT_SEQUENTIAL", "1")
                        .current_dir(&repo_path)
                        .output()
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Get or clone the rust-lang/rust repository to a cached location.
/// This persists across benchmark runs but is cleaned by `cargo clean`.
fn get_or_clone_rust_repo() -> PathBuf {
    let cache_dir = std::env::current_dir().unwrap().join("target/bench-repos");
    let rust_repo = cache_dir.join("rust");

    if rust_repo.exists() {
        // Verify it's a valid git repo
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&rust_repo)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                println!("Using cached rust repo at {}", rust_repo.display());
                return rust_repo;
            }
            Ok(out) => {
                panic!(
                    "Git rev-parse failed in cached repo: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
            Err(e) => {
                panic!("Failed to execute git command: {}", e);
            }
        }
    }

    // Clone the repo
    std::fs::create_dir_all(&cache_dir).unwrap();
    println!(
        "Cloning rust-lang/rust to {} (this will take several minutes)...",
        rust_repo.display()
    );

    // Do a full clone to ensure we have all history. This takes longer on first run
    // (~5-10 minutes) but is cached in target/bench-repos/ for subsequent runs.
    let clone_output = Command::new("git")
        .args([
            "clone",
            "https://github.com/rust-lang/rust.git",
            rust_repo.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        clone_output.status.success(),
        "Failed to clone rust repo: {}",
        String::from_utf8_lossy(&clone_output.stderr)
    );

    println!("Rust repo cloned successfully");
    rust_repo
}

fn bench_list_real_repo(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_real_repo");

    let binary = get_release_binary();

    // Get or clone the rust repo (cached across runs)
    let rust_repo = get_or_clone_rust_repo();

    // Test with different worktree counts
    for num_worktrees in [1, 2, 4, 6, 8] {
        // Create a temporary workspace for this benchmark run
        let temp = tempfile::tempdir().unwrap();
        let workspace_main = temp.path().join("main");

        // Copy the rust repo to the temp location (git worktree needs the original)
        // Use git clone --local for fast copy with shared objects
        let clone_output = Command::new("git")
            .args([
                "clone",
                "--local",
                rust_repo.to_str().unwrap(),
                workspace_main.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            clone_output.status.success(),
            "Failed to clone rust repo to workspace: {}",
            String::from_utf8_lossy(&clone_output.stderr)
        );

        run_git(&workspace_main, &["config", "user.name", "Benchmark"]);
        run_git(&workspace_main, &["config", "user.email", "bench@test.com"]);

        // Add worktrees with realistic changes
        for i in 1..num_worktrees {
            add_worktree_with_divergence(
                &temp,
                &workspace_main,
                i,
                10, // commits ahead
                0,  // commits behind (skip for now)
                3,  // uncommitted files
            );
        }

        // Warm up git's internal caches
        run_git(&workspace_main, &["status"]);

        group.bench_with_input(
            BenchmarkId::new("rust_repo", num_worktrees),
            &num_worktrees,
            |b, _| {
                b.iter(|| {
                    Command::new(&binary)
                        .arg("list")
                        .current_dir(&workspace_main)
                        .output()
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(30)
        .measurement_time(std::time::Duration::from_secs(15))
        .warm_up_time(std::time::Duration::from_secs(3));
    targets = bench_list_by_worktree_count, bench_list_by_repo_profile, bench_sequential_vs_parallel, bench_list_real_repo
}
criterion_main!(benches);
