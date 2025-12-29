# Twitter Thread: Worktrunk Launch

<!--
STRUCTURE:
1. Intro - hook with announcement (link-free for max reach)
2. Context - Claude Code instances, why isolation matters
3. Git worktree UX problem
4. What Worktrunk is + GitHub link
5. Core command - wt switch
6. Other core commands - list, remove
7-12. Features - hooks, list + CI status, select, LLM commits, wt merge, status line
13. Omnibus demo - full workflow with parallel agents in Zellij
14. CTA - install, docs, feedback, star
15. Thanks to Claude Code team + RT request

DESIGN DECISIONS:
- Lead with "models improved → running more agents → worktrees → UX terrible → built this"
- 🧵 signals it's a thread
- Tweet 1 is link-free (hook needs max reach)
- GitHub link in tweet 4 (what Worktrunk is); GitHub star in tweet 14 (CTA)
- Tweet 2 explains isolation need; tweet 3 shows UX pain; tweet 4 introduces Worktrunk; tweet 5 shows wt switch
- Tweet 15 thanks team + RT request
- Core commands split across two tweets for focused demos
- Features are snappy, one per tweet, with media where applicable
- Monospace: Twitter doesn't support it; use screenshots or plain text (GIF shows commands)
- Social proof (PRQL, xarray, insta) cut for now to keep focus on the tool
-->

---

<!-- ============ PHASE 1: INTRO ============ -->
<!-- Goal: Hook, announce, explain what it is, set up the thread -->

<!-- TODO: Wordsmith tweets 1-2. Current version is functional but doesn't sing.
     Attempted combining them but lost clarity. Key requirements:
     - Tweet 1 must say what Worktrunk is (git worktree manager)
     - Need to convey the AI agent use case without being abstract
     - Lead with concrete pain, not marketing speak
     - Avoid slop: "fills the gap", "painful UX", "actually usable" -->

**1/** (190 chars)
Announcing Worktrunk! A git worktree manager, designed for running AI agents in parallel.

A few points on why I'm so excited about the project, and why I hope it becomes broadly adopted 🧵

[wt-demo.gif]

<!-- NOTE: Considered Zellij demo here but it's too complex for tweet 1's hook role.
     Placed omnibus demo at tweet 13 instead (before CTA). -->

<!-- ============ PHASE 2: CONTEXT ============ -->
<!-- Goal: Why isolation matters, then prove the UX problem -->

**2/** (202 chars)
As models have improved this year, I've been running more & more Claude Code instances in parallel.

Each needs its own isolated working directory, otherwise they get confused by each other's changes.

**3/** (222 chars)
Git worktrees solve this, but the UX is terrible!

To create & navigate to a new worktree:

git worktree add -b feat ../repo.feat && cd ../repo.feat

...even for a simple command, we need to type the name three times.

<!-- ============ PHASE 3: CORE COMMANDS ============ -->
<!-- Goal: Contrast with solution, then introduce core commands -->

**4/** (167 chars)
Worktrunk is a CLI, written in Rust, which makes git worktrees as easy as branches.

https://github.com/max-sixty/worktrunk

**5/** (99 chars)
In contrast to the git command, the Worktrunk command to create a new worktree is short (& aliasable):

wt switch --create api

[wt-switch.gif — creating and switching between worktrees]

**6/** (105 chars)
Worktrunk's other core commands:

wt list: see all worktrees with status
wt remove: delete a worktree

[wt-list-remove.gif — list then remove]

<!-- ============ PHASE 4: FEATURES ============ -->
<!-- Goal: List additional capabilities, one per tweet, snappy -->

**7/** (228 chars)
Beyond the core commands, features that make worktrees simpler:

Post-start hooks run after creating a worktree: install deps, copy caches, start dev servers, set up the environment. There's a hook for every stage of the worktree lifecycle.

[wt-hooks.gif — switch --create showing multiple post-start hooks running]

<!-- TODO: Consider cutting or merging tweets 8-9. Reviewers noted:
     - "50ms" is too technical / doesn't connect to AI workflows
     - Fuzzy picker isn't differentiated (every CLI has one)
     - Thread may be too long; these are weak candidates for cutting -->

**8/** (235 chars)
wt list renders in ~50ms, then fills in details (CI status, diff stats) as they become available. Can also list branches with wt list --branches.

wt list --full: CI status as clickable dots. Green/blue/red. Clicking opens the PR.

[wt-list.gif — showing progressive rendering]

**9/** (45 chars)
wt select: fuzzy picker across all branches.

[wt-select.gif]

**10/** (99 chars)
When running wt step commit or wt merge, worktrunk can have an LLM write the commit message, with a customizable template.

[wt-commit.gif — git diff then wt step commit]

**11/** (78 chars)
wt merge: squash, rebase, merge, remove worktree, delete branch, in one command.

[wt-merge.gif]

**12/** (83 chars)
@claudeai status line integration. See branch, diff stats, CI status at a glance.

[screenshot of Claude Code with worktrunk statusline]

**13/** (106 chars)
Putting it all together: parallel Claude Code agents in Zellij tabs, creating features, merging them back.

[wt-zellij-omnibus.gif]

<!-- ============ PHASE 5: CTA ============ -->
<!-- Goal: Install instructions, docs, invite feedback, star -->

**14/** (167 chars)
To install:

brew install max-sixty/worktrunk/wt
wt config shell install

Feedback welcome. Open an issue or reply here.

⭐ https://github.com/max-sixty/worktrunk

**15/** (230 chars)
Big thanks to @AnthropicAI and the @claudeai team, including @bcherny @\_catwu @alexalbert\_\_, for building Claude Code. Worktrunk wouldn't exist without it 🙏

If this was useful, liking & RT-ing the first tweet helps spread the word.

[TODO: paste link to tweet 1]

---

## Notes

- **Monospace in tweets**: Twitter doesn't support code formatting. Options:
  - Unicode monospace via [YayText](https://yaytext.com/monospace/): 𝚠𝚝 𝚜𝚠𝚒𝚝𝚌𝚑 -𝚌 𝚏𝚎𝚊𝚝
  - Screenshots
  - Plain text (the GIF shows commands anyway)
- **Social proof**: Cut for now, could add back in a later tweet
