---
name: worktree-isolation
description: Create and clean up isolated git worktrees for parallel AI agent tasks without workspace or build-cache collisions.
---

# Git Worktree Isolation

1. Fetch the latest default branch (`git fetch origin --prune`).
2. Create an isolated sibling worktree:
   `git worktree add ../<repo>-wt-<task> -b <prefix>/<task> main`
3. Execute edits, builds, and tests strictly inside the isolated worktree.
4. After the PR merges, remove the worktree cleanly:
   `git worktree remove ../<repo>-wt-<task> && git worktree prune`
