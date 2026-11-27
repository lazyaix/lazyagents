---
name: stacked-pr
description: Manage Sequential and Stacked Pull Requests safely under Squash-Only merge using Draft PRs, Depends-on links, and git rebase --update-refs --onto.
---

# Stacked PR Workflow (Squash-Only Safe)

1. **Prefer Sequential PRs when merging rapidly**: merge the base PR into `main`, run `git fetch origin --prune`, fast-forward `main`, then branch the next PR from updated `main`.
2. **When async review requires a stack**:
   - Keep only the bottom PR (`base: main`) Ready; open downstream PRs as **Draft** (`gh pr create --draft`) with `Depends on #<upstream-pr>`.
   - Propagate mid-review edits up the stack locally via `git rebase --update-refs` and `git push --force-with-lease`.
3. **After an upstream PR squash-merges (critical)**:
   - **Never** click GitHub's *Update branch* button or run plain `git rebase origin/main` (replays already-squashed parent commits and causes false conflicts).
   - Skip the old pre-squash parent commit tip and restack the remaining chain cleanly:
     `git fetch origin --prune && git rebase --update-refs --onto origin/main <old-parent-tip> <top-branch>`
   - Force-push updated child branches safely with `git push --force-with-lease`.
   - Confirm the newly promoted PR targets `main` with `MERGEABLE` status, then run `gh pr ready <pr>`.
