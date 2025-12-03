+++
title = "wt switch"
weight = 10

[extra]
group = "Commands"
+++

Navigate between worktrees or create new ones. Switching to an existing worktree is just a directory change. With `--create`, a new branch and worktree are created, and hooks run.

## Examples

Switch to an existing worktree:

```bash
wt switch feature-auth
```

Create a new worktree for a fresh branch:

```bash
wt switch --create new-feature
```

Create from a specific base branch:

```bash
wt switch --create hotfix --base production
```

Switch to the previous worktree (like `cd -`):

```bash
wt switch -
```

## Creating Worktrees

The `--create` flag (or `-c`) creates a new branch from the default branch (or `--base`), sets up a worktree at `../{repo}.{branch}`, runs [post-create hooks](/hooks/#post-create) synchronously, then spawns [post-start hooks](/hooks/#post-start) in the background before switching to the new directory.

```bash
# Create from main (default)
wt switch --create api-refactor

# Create from a specific branch
wt switch --create emergency-fix --base release-2.0

# Create and open in editor
wt switch --create docs --execute "code ."

# Skip all hooks
wt switch --create temp --no-verify
```

## Shortcuts

Special symbols for common targets:

| Shortcut | Meaning |
|----------|---------|
| `-` | Previous worktree (like `cd -`) |
| `@` | Current branch's worktree |
| `^` | Default branch (main/master) |

```bash
wt switch -                              # Go back to previous worktree
wt switch ^                              # Switch to main worktree
wt switch --create bugfix --base=@       # Branch from current HEAD
```

## Hooks

When creating a worktree (`--create`), hooks run in this order:

1. **post-create** — Blocking, sequential. Typically: `npm install`, `cargo build`
2. **post-start** — Background, parallel. Typically: dev servers, file watchers

See [Hooks](/hooks/) for configuration details.

## How Arguments Are Resolved

Arguments resolve using **path-first lookup**:

1. Compute the expected path for the argument (using the configured path template)
2. If a worktree exists at that path, switch to it (regardless of what branch it's on)
3. Otherwise, treat the argument as a branch name

**Example**: If `repo.foo/` exists but is on branch `bar`:

- `wt switch foo` switches to `repo.foo/` (the `bar` branch worktree)
- `wt switch bar` also works (falls back to branch lookup)

---

## Command Reference

<!-- ⚠️ AUTO-GENERATED from `wt switch --help-page` — edit cli.rs to update -->

```bash
wt switch - Switch to a worktree
Usage: wt switch [OPTIONS] <BRANCH>

Arguments:
  <BRANCH>
          Branch or worktree name

          Shortcuts: '^' (main), '-' (previous), '@' (current)

Options:
  -c, --create
          Create a new branch

  -b, --base <BASE>
          Base branch

          Defaults to default branch.

  -x, --execute <EXECUTE>
          Command to run after switch

  -f, --force
          Skip approval prompts

      --no-verify
          Skip all project hooks

  -h, --help
          Print help (see a summary with '-h')

Global Options:
  -C <path>
          Working directory for this command

      --config <path>
          User config file path

  -v, --verbose
          Show commands and debug info
```
