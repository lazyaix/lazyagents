---
name: pr-author
description: Draft and verify a Pull Request with a Conventional Commits 1.0.0 title, local verification checks, and repository PR template.
---

# Pull Request Authoring Workflow

1. Confirm you are on a short-lived topic branch (never the default branch) and inspect `git diff --stat`.
2. Run every required local verification command declared in `AGENTS.md`.
3. Compose a Conventional Commits 1.0.0 PR title (`<type>(<scope>): <concise summary>`).
4. Fill out the repository PR template completely, including verification checkboxes and `Depends on #<pr>` for stacked PRs.
5. Open the PR with `gh pr create` and monitor required checks.
