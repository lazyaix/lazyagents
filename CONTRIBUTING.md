# Contributing to lazyagents

Thank you for your interest in contributing to `lazyaix/lazyagents`!

This project follows **Modern GitHub Flow (Trunk-Based Development)** with **Squash-only merges** and **Conventional Commits 1.0.0**. For the full platform governance and release architecture, see [docs/git-workflow.md](docs/git-workflow.md).

## Quick Start (Fork + Pull Request)

1. **Fork** [lazyaix/lazyagents](https://github.com/lazyaix/lazyagents) and clone your fork locally.
2. Create a short-lived branch from `main` using a descriptive prefix:
   ```bash
   git checkout -b feat/short-description
   ```
   Recommended prefixes: `feat/`, `fix/`, `refactor/`, `docs/`, `test/`, `ci/`, `chore/`, `agent/`.
3. Keep changes small, focused, and ideally mergeable within 1–2 days. For large features, split into sequential **Stacked PRs** and note `Depends on #<pr>` in the PR description.
4. Run local quality checks before opening or updating a PR:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace --all-features
   ```
   *(Local Git hooks are intentionally not forced; GitHub Actions CI is the single source of truth for merge gates.)*
5. Open a Pull Request targeting `main` and fill out the PR template.

## PR Title & Commit Convention

Because `main` enforces **Squash and Merge** using the **PR title** as the final commit subject:

- Intermediate commits on your PR branch do **not** have to follow a strict format.
- Your **PR title MUST strictly follow [Conventional Commits 1.0.0](https://www.conventionalcommits.org/)** (validated by the `PR Title` CI check):
  ```text
  <type>[optional (<scope>)][optional !]: <concise description>
  ```
- Allowed types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
- For breaking changes, append `!` after the type/scope (e.g., `feat(api)!: change runner trait`) and include a `BREAKING CHANGE:` section in the PR description.

## Required CI Checks

Every PR must be up to date with `main`, have all review conversations resolved, and pass all required GitHub Actions checks:

- `PR Title` — Conventional Commits 1.0.0 title validation
- `Format` — `cargo fmt --all -- --check`
- `Clippy` — `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `Test` — `cargo nextest` (plus doctests when library targets exist)
- `Cargo Deny` — RustSec advisories, license compliance, and source auditing (`deny.toml`)
- `Zizmor` — GitHub Actions workflow supply-chain security audit

## Recommended: SSH Commit Signing

Signed commits are **recommended** (not currently hard-blocked in Rulesets). The simplest way to sign commits on GitHub is using your existing Ed25519 SSH key:

```bash
git config --global gpg.format ssh
git config --global user.signingkey ~/.ssh/id_ed25519.pub
git config --global commit.gpgsign true
```

Then add `~/.ssh/id_ed25519.pub` under **GitHub Settings → SSH and GPG keys → New SSH key** with Key type set to **Signing Key**.

## Licensing

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in `lazyagents` is dual-licensed under **Apache-2.0 OR MIT** (as defined in [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT)), without any additional terms or conditions. No separate CLA or DCO trailer is required.
