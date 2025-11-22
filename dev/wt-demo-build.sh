#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DEMO_DIR="$SCRIPT_DIR/wt-demo"
OUT_DIR="$DEMO_DIR/out"
DEMO_ROOT="$OUT_DIR/.demo"
DEMO_HOME="${DEMO_HOME:-$DEMO_ROOT}"
LOG="$OUT_DIR/record.log"
TAPE_TEMPLATE="$DEMO_DIR/demo.tape"
TAPE_RENDERED="$OUT_DIR/.rendered.tape"
STARSHIP_CONFIG_PATH="$OUT_DIR/starship.toml"
OUTPUT_GIF="$OUT_DIR/wt-demo.gif"
BARE_REMOTE=""
DEMO_REPO=""
DEMO_WORK_BASE=""

cleanup() {
  rm -f "$TAPE_RENDERED"
}

require_bin() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing dependency: $1" >&2
    exit 1
  fi
}

# Commit with a date offset (e.g., "7 days ago", "2 hours ago")
commit_dated() {
  local repo="$1"
  local message="$2"
  local date_offset="$3"
  local date_str
  date_str=$(date -v-"$date_offset" "+%Y-%m-%dT%H:%M:%S")
  GIT_AUTHOR_DATE="$date_str" GIT_COMMITTER_DATE="$date_str" \
    SKIP_DEMO_HOOK=1 git -C "$repo" commit -qm "$message"
}

write_starship_config() {
  mkdir -p "$(dirname "$STARSHIP_CONFIG_PATH")"
  cat >"$STARSHIP_CONFIG_PATH" <<'CFG'
format = "$directory$character"
palette = "gh_light"

[palettes.gh_light]
fg = "#1f2328"
bg = "#ffffff"
blue = "#0969da"
yellow = "#d29922"
green = "#2ea043"
red = "#d73a49"
muted = "#57606a"

[directory]
style = "bold fg:blue"
truncation_length = 3
truncate_to_repo = true
home_symbol = "~"

[git_branch]
style = "fg:muted"
symbol = " "
format = " [$symbol$branch]($style)"

[git_status]
style = "fg:red"
format = " [$all_status$ahead_behind]($style)"
conflicted = "⇕"
ahead = "⇡"
behind = "⇣"
staged = "+"
modified = "!"
untracked = "?"

[cmd_duration]
min_time = 500
# Keep duration but drop the timer icon to reduce prompt noise.
format = " [$duration]($style)"
style = "fg:muted"

[character]
success_symbol = "[❯](fg:green)"
error_symbol = "[❯](fg:red)"
vicmd_symbol = "[❮](fg:blue)"

[time]
disabled = true
CFG
}

prepare_repo() {
  # Clean previous temp repo; also clean legacy root-level .demo if it exists.
  rm -rf "$DEMO_ROOT"
  if [ -d "$REPO_ROOT/.demo" ] && [ "$REPO_ROOT/.demo" != "$DEMO_ROOT" ]; then
    rm -rf "$REPO_ROOT/.demo"
  fi
  mkdir -p "$DEMO_ROOT"
  export HOME="$DEMO_HOME"
  DEMO_WORK_BASE="$HOME/w"
  rm -rf "$DEMO_WORK_BASE"
  mkdir -p "$DEMO_WORK_BASE"
  DEMO_REPO="$DEMO_WORK_BASE/acme"
  mkdir -p "$DEMO_REPO"
  export DEMO_REPO

  BARE_REMOTE="$DEMO_ROOT/remote.git"
  git init --bare -q "$BARE_REMOTE"

  git -C "$DEMO_REPO" init -q
  git -C "$DEMO_REPO" config user.name "Worktrunk Demo"
  git -C "$DEMO_REPO" config user.email "demo@example.com"
  printf "# Worktrunk demo\n\nThis repo is generated automatically.\n" >"$DEMO_REPO/README.md"
  git -C "$DEMO_REPO" add README.md
  commit_dated "$DEMO_REPO" "Initial demo commit" "7d"
  git -C "$DEMO_REPO" branch -m main
  git -C "$DEMO_REPO" remote add origin "$BARE_REMOTE"
  git -C "$DEMO_REPO" push -u origin main -q

# Create a simple Rust project with tests
  cat >"$DEMO_REPO/Cargo.toml" <<'CARGO'
[package]
name = "acme"
version = "0.1.0"
edition = "2021"

[workspace]
CARGO
  cat >"$DEMO_REPO/rust-toolchain.toml" <<'TOOLCHAIN'
[toolchain]
channel = "stable"
TOOLCHAIN
  mkdir -p "$DEMO_REPO/src"
  cat >"$DEMO_REPO/src/lib.rs" <<'RUST'
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }

    #[test]
    fn test_add_negative() {
        assert_eq!(add(-1, 1), 0);
    }
}
RUST
  echo "/target" >"$DEMO_REPO/.gitignore"
  git -C "$DEMO_REPO" add .gitignore Cargo.toml rust-toolchain.toml src/
  commit_dated "$DEMO_REPO" "Add Rust project with tests" "6d"
  # Pre-build to create Cargo.lock and cache dependencies
  (cd "$DEMO_REPO" && cargo build --release -q 2>/dev/null)
  git -C "$DEMO_REPO" add Cargo.lock
  commit_dated "$DEMO_REPO" "Add Cargo.lock" "6d"
  git -C "$DEMO_REPO" push -q

  # Add worktrunk project hooks
  mkdir -p "$DEMO_REPO/.config"
  cat >"$DEMO_REPO/.config/wt.toml" <<'TOML'
[pre-merge-command]
test = "cargo nextest run --no-fail-fast"
TOML
  git -C "$DEMO_REPO" add .config/wt.toml
  commit_dated "$DEMO_REPO" "Add project hooks" "5d"
  git -C "$DEMO_REPO" push -q

  # Create mock gh CLI for CI status
  mkdir -p "$DEMO_HOME/bin"
  cat >"$DEMO_HOME/bin/gh" <<'GH'
#!/usr/bin/env bash
# Mock gh CLI for demo

if [[ "$1" == "auth" && "$2" == "status" ]]; then
  exit 0
fi

if [[ "$1" == "pr" && "$2" == "list" ]]; then
  branch=""
  for arg in "$@"; do
    if [[ "$prev" == "--head" ]]; then
      branch="$arg"
    fi
    prev="$arg"
  done

  case "$branch" in
    alpha)
      echo '[{"state":"OPEN","headRefOid":"abc123","mergeStateStatus":"CLEAN","statusCheckRollup":[{"status":"COMPLETED","conclusion":"SUCCESS"}],"url":"https://github.com/acme/demo/pull/1"}]'
      ;;
    beta)
      echo '[{"state":"OPEN","headRefOid":"def456","mergeStateStatus":"CLEAN","statusCheckRollup":[{"status":"IN_PROGRESS","conclusion":null}],"url":"https://github.com/acme/demo/pull/2"}]'
      ;;
    hooks)
      echo '[{"state":"OPEN","headRefOid":"ghi789","mergeStateStatus":"CLEAN","statusCheckRollup":[{"status":"COMPLETED","conclusion":"FAILURE"}],"url":"https://github.com/acme/demo/pull/3"}]'
      ;;
    *)
      echo '[]'
      ;;
  esac
  exit 0
fi

if [[ "$1" == "run" && "$2" == "list" ]]; then
  branch=""
  for arg in "$@"; do
    if [[ "$prev" == "--branch" ]]; then
      branch="$arg"
    fi
    prev="$arg"
  done

  case "$branch" in
    main)
      echo '[{"status":"completed","conclusion":"success","headSha":"abc123"}]'
      ;;
    *)
      echo '[]'
      ;;
  esac
  exit 0
fi

exit 1
GH
  chmod +x "$DEMO_HOME/bin/gh"

  # Set up user config with LLM and pre-approved commands
  local project_id="${BARE_REMOTE%.git}"
  mkdir -p "$DEMO_HOME/.config/worktrunk"
  cat >"$DEMO_HOME/.config/worktrunk/config.toml" <<TOML
[commit-generation]
command = "llm"
args = ["-m", "claude-haiku-4.5"]

[projects."$project_id"]
approved-commands = ["cargo nextest run --no-fail-fast"]
TOML

  # Create two extra branches (no worktrees) for listing.
  git -C "$DEMO_REPO" branch docs/readme
  git -C "$DEMO_REPO" branch spike/search

  create_branch_alpha
  create_branch_beta

  # Add commit to main after beta, so beta is behind
  echo "# Development" >>"$DEMO_REPO/README.md"
  echo "See CONTRIBUTING.md for guidelines." >>"$DEMO_REPO/README.md"
  git -C "$DEMO_REPO" add README.md
  commit_dated "$DEMO_REPO" "docs: add development section" "1d"
  git -C "$DEMO_REPO" push -q

  create_branch_hooks
}

create_branch_alpha() {
  local branch="alpha"
  local path="$DEMO_WORK_BASE/acme.$branch"
  git -C "$DEMO_REPO" checkout -q -b "$branch" main
  # Modify README: small expansion (+12, -1)
  cat >"$DEMO_REPO/README.md" <<'MD'
# Worktrunk demo

A demo repository for showcasing worktrunk features.

## Features

- Fast worktree switching
- Integrated merge workflow
- Pre-merge test hooks
- LLM-generated commit messages

## Getting Started

Run `wt list` to see all worktrees.
MD
  git -C "$DEMO_REPO" add README.md
  commit_dated "$DEMO_REPO" "docs: expand README" "3d"
  # Add more commits to vary main↕
  echo "# Contributing" >>"$DEMO_REPO/README.md"
  echo "PRs welcome!" >>"$DEMO_REPO/README.md"
  git -C "$DEMO_REPO" add README.md
  commit_dated "$DEMO_REPO" "docs: add contributing section" "3d"
  echo "" >>"$DEMO_REPO/README.md"
  echo "# License" >>"$DEMO_REPO/README.md"
  echo "MIT" >>"$DEMO_REPO/README.md"
  git -C "$DEMO_REPO" add README.md
  commit_dated "$DEMO_REPO" "docs: add license" "3d"
  git -C "$DEMO_REPO" push -u origin "$branch" -q
  git -C "$DEMO_REPO" checkout -q main
  git -C "$DEMO_REPO" worktree add -q "$path" "$branch"
  # Add unpushed commit (shows ⇡ in Status)
  echo "# FAQ" >>"$path/README.md"
  git -C "$path" add README.md
  commit_dated "$path" "docs: add FAQ section" "3d"
  # Significant working tree changes
  # Modified file (!) - add lots of content for +100 diff
  cat >"$path/README.md" <<'MD'
# Worktrunk demo

A powerful demo for worktrunk.

## Quick Start

1. Clone the repo
2. Run `wt list`
3. Switch worktrees with `wt switch`

## Commands

- `wt list` - Show worktrees
- `wt switch` - Switch worktree
- `wt merge` - Merge and cleanup

## API Reference

### Core Functions

#### `list_worktrees()`

Returns all worktrees in the repository.

```rust
pub fn list_worktrees(repo: &Repository) -> Result<Vec<Worktree>> {
    let worktrees = repo.worktrees()?;
    worktrees.iter().map(|name| {
        let wt = repo.find_worktree(name)?;
        Ok(Worktree::from_git(wt))
    }).collect()
}
```

#### `switch_worktree()`

Switches to the specified worktree.

```rust
pub fn switch_worktree(name: &str) -> Result<()> {
    let path = find_worktree_path(name)?;
    std::env::set_current_dir(path)?;
    Ok(())
}
```

#### `create_worktree()`

Creates a new worktree for the given branch.

```rust
pub fn create_worktree(branch: &str, base: &str) -> Result<PathBuf> {
    let repo = Repository::open_from_env()?;
    let path = generate_worktree_path(&repo, branch)?;
    repo.worktree(branch, &path, Some(&base))?;
    Ok(path)
}
```

#### `merge_worktree()`

Merges the current branch into main and cleans up.

```rust
pub fn merge_worktree(opts: MergeOptions) -> Result<()> {
    let branch = current_branch()?;
    rebase_onto_main(&branch)?;
    fast_forward_main(&branch)?;
    if !opts.keep_worktree {
        remove_worktree(&branch)?;
    }
    Ok(())
}
```

### Helper Functions

#### `find_worktree_path()`

Resolves a worktree name to its filesystem path.

#### `generate_worktree_path()`

Generates a path for a new worktree based on naming conventions.

#### `current_branch()`

Returns the name of the currently checked out branch.

#### `rebase_onto_main()`

Rebases the given branch onto the main branch.

#### `fast_forward_main()`

Fast-forwards main to include the rebased commits.

#### `remove_worktree()`

Removes a worktree and optionally deletes its branch.

## Error Handling

All functions return `Result<T>` with detailed error types:

- `WorktreeNotFound` - The specified worktree doesn't exist
- `BranchInUse` - The branch is checked out in another worktree
- `MergeConflict` - Conflicts detected during rebase
- `DirtyWorkingTree` - Uncommitted changes present

## Performance Notes

- Listing uses parallel git operations for speed
- Diff calculations are cached per-session
- Remote fetches happen in background threads
MD
  # Untracked file (?)
  echo "// scratch" >"$path/scratch.rs"
}

create_branch_beta() {
  local branch="beta"
  local path="$DEMO_WORK_BASE/acme.$branch"
  git -C "$DEMO_REPO" checkout -q -b "$branch" main
  # No commits - same as main (↑0)
  git -C "$DEMO_REPO" push -u origin "$branch" -q
  git -C "$DEMO_REPO" checkout -q main
  git -C "$DEMO_REPO" worktree add -q "$path" "$branch"
  # Staged new file (+)
  echo "# TODO" >"$path/notes.txt"
  echo "- Add caching" >>"$path/notes.txt"
  git -C "$path" add notes.txt
}

create_branch_hooks() {
  local branch="hooks"
  local path="$DEMO_WORK_BASE/acme.$branch"
  git -C "$DEMO_REPO" checkout -q -b "$branch" main
  # Refactor lib.rs: add multiply/subtract, remove both old tests (+12, -8)
  cat >"$DEMO_REPO/src/lib.rs" <<'RUST'
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operations() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(subtract(5, 3), 2);
        assert_eq!(multiply(3, 4), 12);
    }
}
RUST
  git -C "$DEMO_REPO" add src/lib.rs
  commit_dated "$DEMO_REPO" "feat: add math operations, consolidate tests" "2H"
  # No push - this branch has no upstream
  git -C "$DEMO_REPO" checkout -q main
  git -C "$DEMO_REPO" worktree add -q "$path" "$branch"
  # Staged change (+)
  echo "// Division coming soon" >>"$path/src/lib.rs"
  git -C "$path" add src/lib.rs
  # Modified file (!) - modify again after staging
  echo "// TODO: add division" >>"$path/src/lib.rs"
}

render_tape() {
  sed \
    -e "s|{{DEMO_REPO}}|$DEMO_REPO|g" \
    -e "s|{{DEMO_HOME}}|$DEMO_HOME|g" \
    -e "s|{{REAL_HOME}}|$HOME|g" \
    -e "s|{{STARSHIP_CONFIG}}|$STARSHIP_CONFIG_PATH|g" \
    -e "s|{{OUTPUT_GIF}}|$OUTPUT_GIF|g" \
    "$TAPE_TEMPLATE" >"$TAPE_RENDERED"
}

record_text() {
  mkdir -p "$OUT_DIR"
  DEMO_RAW="$OUT_DIR/run.raw.txt"
  local real_home="$HOME"

  # Extract commands from demo.tape
  local commands
  commands=$(grep -E '^Type ' "$TAPE_TEMPLATE" | sed 's/^Type //' | tr -d '"' | tr -d "'")

  env DEMO_REPO="$DEMO_REPO" RAW_PATH="$DEMO_RAW" COMMANDS="$commands" bash -lc '
    set -o pipefail
    export LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8
    export COLUMNS=160
    export RUSTUP_HOME="'"$real_home"'/.rustup"
    export CARGO_HOME="'"$real_home"'/.cargo"
    export HOME="'"$DEMO_HOME"'"
    export PATH="$HOME/bin:$PATH"
    export STARSHIP_CONFIG="'"$STARSHIP_CONFIG_PATH"'"
    export STARSHIP_CACHE="'"$DEMO_ROOT"'"/starship-cache
    mkdir -p "$STARSHIP_CACHE"
    export WT_PROGRESSIVE=false
    export NO_COLOR=1
    export CLICOLOR=0
    eval "$(starship init bash)" >/dev/null 2>&1
    eval "$(wt config shell init bash)" >/dev/null 2>&1
    cd "$DEMO_REPO"
    {
      while IFS= read -r cmd; do
        # Skip setup commands and exit
        case "$cmd" in
          "export "*|"eval "*|"cd "*|"clear"|"exit") continue ;;
        esac
        eval "$cmd"
      done <<< "$COMMANDS"
    } >"$RAW_PATH" 2>&1
  '
  RAW_PATH="$DEMO_RAW" OUT_DIR="$OUT_DIR" python3 - <<'PY'
import os, re, pathlib
raw = pathlib.Path(os.environ["RAW_PATH"]).read_text(errors="ignore")
# strip ANSI escape sequences and control chars
clean = re.sub(r"\x1B\[[0-9;?]*[A-Za-z]", "", raw)
clean = re.sub(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]", "", clean)
clean = clean.replace("^D", "")
clean = clean.lstrip()
pathlib.Path(os.environ["OUT_DIR"]).joinpath("run.txt").write_text(clean.strip() + "\n")
PY
}

record_vhs() {
  mkdir -p "$OUT_DIR"
  vhs "$TAPE_RENDERED" >"$LOG" 2>&1
}

main() {
  require_bin wt
  require_bin vhs
  require_bin starship
  trap cleanup EXIT

  mkdir -p "$OUT_DIR"
  write_starship_config
  prepare_repo
  record_text
  prepare_repo
  render_tape
  record_vhs

echo "GIF saved to $OUTPUT_GIF"
echo "Text log saved to $OUT_DIR/run.txt"
echo "Log: $LOG"
}

main "$@"
