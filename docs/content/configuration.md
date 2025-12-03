+++
title = "Configuration"
weight = 20

[extra]
group = "Reference"
+++

Worktrunk uses two configuration files:

| File | Location | Purpose |
|------|----------|---------|
| **User config** | `~/.config/worktrunk/config.toml` | Personal settings, command defaults, approved project commands |
| **Project config** | `.config/wt.toml` | Lifecycle hooks, checked into version control |

## User Config

The user config stores personal preferences that apply across all repositories. Create it with:

```bash
wt config create
```

This creates `~/.config/worktrunk/config.toml` with documented examples. View the current configuration with `wt config show`.

### Worktree Path Template

Controls where new worktrees are created. The template is relative to the repository root:

```toml
# Default — siblings in parent directory
worktree-path = "../{{ main_worktree }}.{{ branch }}"

# Inside the repository
worktree-path = ".worktrees/{{ branch }}"

# Namespaced (useful when multiple repos share a parent directory)
worktree-path = "../worktrees/{{ main_worktree }}/{{ branch }}"
```

**Available variables:**
- `{{ main_worktree }}` — main worktree directory name
- `{{ branch }}` — branch name (slashes replaced with dashes)

### Command Defaults

Override default flag behavior for commands. Settings here apply unless explicitly overridden on the command line.

**`wt list` defaults:**

```toml
[list]
full = true      # --full (default: false)
branches = true  # --branches (default: false)
remotes = true   # --remotes (default: false)
```

**`wt step commit` and `wt merge` staging:**

```toml
[commit]
stage = "all"    # "all" (default), "tracked", or "none"
```

**`wt merge` defaults:**

```toml
[merge]
# All options default to true
squash = false  # --no-squash: preserve individual commits
commit = false  # --no-commit: skip committing uncommitted changes
remove = false  # --no-remove: keep worktree after merge
verify = false  # --no-verify: skip project hooks
```

### LLM Commit Messages

Configure automatic commit message generation. Requires an external tool like [llm](https://llm.datasette.io/):

```toml
[commit-generation]
command = "llm"
args = ["-m", "claude-haiku-4.5"]
```

See [LLM Commit Messages](/llm-commits/) for setup details and template customization.

### Approved Commands

When project hooks run for the first time, Worktrunk prompts for approval. Approved commands are saved here automatically:

```toml
[projects."my-project"]
approved-commands = [
    "post-create.install = npm ci",
    "pre-merge.test = npm test",
]
```

Manage approvals with `wt config approvals list` and `wt config approvals clear <repo>`.

## Project Config

The project config defines lifecycle hooks — commands that run at specific points during worktree operations. This file is checked into version control and shared across the team.

Create `.config/wt.toml` in the repository root:

```toml
[post-create]
install = "npm ci"

[pre-merge]
test = "npm test"
lint = "npm run lint"
```

See [Hooks](/hooks/) for complete documentation on hook types, execution order, and template variables.

## Shell Integration

Worktrunk needs shell integration to change directories when switching worktrees. Install with:

```bash
wt config shell install
```

Or manually add to the shell config:

```bash
# bash/zsh
eval "$(wt config shell init bash)"

# fish
wt config shell init fish | source
```

Without shell integration, `wt switch` prints the target directory but cannot `cd` into it.

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `WORKTRUNK_CONFIG_PATH` | Override user config location |
| `NO_COLOR` | Disable colored output ([standard](https://no-color.org/)) |
| `CLICOLOR_FORCE` | Force colored output even when not a TTY |
