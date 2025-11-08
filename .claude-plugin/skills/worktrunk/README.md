# Worktrunk Configuration Skill

A unified skill for helping users configure Worktrunk, covering both personal settings and project-specific automation.

## Structure

```
worktrunk-config/
├── SKILL.md                              # Main skill file (loaded when skill activates)
├── README.md                             # This file
└── references/                           # Detailed reference documentation
    ├── user-config.md                    # User config (~/.config/worktrunk/config.toml)
    ├── project-config.md                 # Project config (.config/wt.toml)
    └── hook-types-reference.md           # Detailed hook behavior reference
```

## When This Skill Activates

Claude loads this skill when users ask to:
- Configure LLM integration ("set up LLM", "configure commit generation")
- Set up project hooks ("set up hooks", "automate npm install", "run tests before merge")
- Customize worktree paths
- Troubleshoot configuration issues

## Two Config Types

### User Config (`~/.config/worktrunk/config.toml`)
- **Scope**: Personal developer preferences
- **Git**: Never checked in
- **Permission model**: Conservative (always propose, get consent)
- **Contains**: LLM integration, worktree path templates, approved commands
- **Reference**: `references/user-config.md`

### Project Config (`.config/wt.toml`)
- **Scope**: Team-wide automation
- **Git**: Checked into repository
- **Permission model**: Proactive (create directly, easily reversible)
- **Contains**: Lifecycle hooks (post-create, pre-merge, etc.)
- **Reference**: `references/project-config.md`

## Progressive Disclosure Pattern

This skill follows the recommended pattern:

1. **SKILL.md** (~1-2k words)
   - High-level overview
   - Quick workflows for common tasks
   - When to use each config type
   - References to detailed docs

2. **references/*.md** (detailed)
   - Complete step-by-step procedures
   - All configuration options
   - Troubleshooting guides
   - Examples and patterns

Claude loads reference files only when needed, keeping context usage efficient.

## Key Workflows

### LLM Setup (User Config)
1. Check if LLM tool exists
2. Guide installation (user runs commands)
3. Propose config change
4. After approval, apply

### Project Hooks (Project Config)
1. Detect project type (npm, cargo, etc.)
2. Identify available commands
3. Design appropriate hooks
4. Validate commands exist
5. Create `.config/wt.toml` with comments

## Examples

**User asks**: "Help me set up LLM for commit messages"
→ Loads `reference/user-config.md`, follows LLM setup workflow

**User asks**: "Set up some hooks for this project"
→ Loads `reference/project-config.md`, detects project type, proposes hooks

**User asks**: "Why isn't my pre-merge hook running?"
→ Loads `reference/project-config.md#troubleshooting` or `reference/hook-types-reference.md`

## Distribution

Users install this skill by cloning the entire Worktrunk repository as a marketplace:

```bash
cd ~/.claude/plugins/marketplaces
git clone https://github.com/max-sixty/worktrunk.git worktrunk-skills
```

Then restart Claude Code. The skill will be available as `worktrunk-config@worktrunk-skills`.
