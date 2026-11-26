# AI Agent Engineering & Git Workflow Guidelines

All AI coding agents working in `lazyaix/lazyagents` must follow these GitHub-first workflow rules. For full rationale and governance details, read [docs/git-workflow.md](docs/git-workflow.md).

## 1. Workspace & Branch Isolation

- **Never push directly to `main`** (blocked by active Rulesets for all users).
- **Use `git worktree`** whenever running concurrent agent sessions or parallel tasks so working trees and `target/` builds never collide:
  ```bash
  git worktree add ../lazyagents-wt-<task> -b agent/<task> main
  ```
- Use short-lived branches with descriptive prefixes: `feat/`, `fix/`, `refactor/`, `docs/`, `test/`, `ci/`, `chore/`, `agent/`.
- Keep each PR small and focused on one logical vertical slice. For large changes, create **Stacked PRs** and record `Depends on #<pr>` in the PR description.

## 2. Pull Requests & Conventional Commits

- Repository merges use **Squash and Merge only** with the **PR title** as the commit subject on `main`.
- Every **PR title MUST follow Conventional Commits 1.0.0**:
  `<type>(<optional-scope>)[!]: <concise summary>`
  Allowed types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
- If a PR includes breaking changes, append `!` to the PR title and add a `BREAKING CHANGE:` explanation in the PR body.

## 3. Mandatory Local Verification Before Pushing

Run and ensure all checks pass before pushing or updating a PR:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## 4. CI Self-Remediation & Security Hygiene

- After pushing a PR branch, monitor all required GitHub Actions checks (`PR Title`, `Format`, `Clippy`, `Test`, `Cargo Deny`, `Zizmor`). Fix any CI failures directly on the branch until all checks pass.
- When editing `.github/workflows/*.yml`:
  - Pin every `uses:` action to a **full 40-character commit SHA** with a `# vX.Y.Z` comment.
  - Keep top-level `permissions: {}` and declare minimal per-job permissions.
  - Set `persist-credentials: false` on `actions/checkout`.
  - Never use `pull_request_target` with untrusted PR code.
- Never commit secrets, tokens, or `.env` files.
