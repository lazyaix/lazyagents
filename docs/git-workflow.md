# GitHub Workflow Specification (`lazyaix/lazyagents`)

This document is the authoritative specification for Git collaboration, GitHub Repository Rulesets, CI/CD quality gates, AI agent parallel development, and automated releases for **`lazyaix/lazyagents`**.

---

## 1. Repository Identity & Core Principles

| Dimension | Standard |
| :--- | :--- |
| **Platform** | GitHub only (`lazyaix/lazyagents`, Public OSS under GitHub Enterprise org `lazyaix`) |
| **License** | `Apache-2.0 OR MIT` dual license (GitHub ToS inbound=outbound; no DCO/CLA friction) |
| **Default Branch** | `main` (sole permanent branch, always buildable, tested, and releasable) |
| **Branching Model** | Modern GitHub Flow / Trunk-Based Development (TBD) |
| **Merge Strategy** | **Squash and Merge only** (linear history, `PR_TITLE` as commit subject, `PR_BODY` as commit body, auto-delete head branch) |
| **Commit Standard** | [Conventional Commits 1.0.0](https://www.conventionalcommits.org/) enforced on PR titles |
| **Release Engine** | **Manual Git Tag (`vX.Y.Z`)** → `.github/workflows/release.yml` (5-platform prebuilt archives + `checksums.txt` + SLSA provenance + `lazyagents-installer.sh` + Homebrew Formula + Immutable GitHub Release) |

---

## 2. Branching Model & Parallel Development

### 2.1 Branch Topology

- **`main`**: The single long-lived branch. Direct pushes, deletions, force pushes, and merge commits are strictly blocked for everyone (including Admins).
- **Short-lived working branches**:
  - **External contributors**: Fork `lazyaix/lazyagents` and open PRs from short-lived branches in their fork.
  - **Maintainers & AI agents**: Branch off `main`, keep PRs small (aim for 1–2 days lifespan, < 300–400 LOC where practical), merge via PR, and auto-delete the branch on merge.
- **Maintenance branches (`release/vX.Y`)**: **On-demand only.** There is no `develop` branch. Normal bug fixes flow forward into `main`. Only if a post-`v1.0` minor release requires backporting a critical security patch, cut a temporary `release/vX.Y` branch from its release tag and cherry-pick the fix from `main`.

### 2.2 Recommended Branch Naming Convention

*(Documented convention — not hard-blocked by Rulesets so external Fork branch names are never rejected.)*

| Prefix | Usage |
| :--- | :--- |
| `feat/<topic>` | New feature or user-facing capability |
| `fix/<topic>` | Bug fix |
| `refactor/<topic>` | Code restructuring with no behavior change |
| `docs/<topic>` | Documentation updates |
| `test/<topic>` | Adding or improving tests |
| `ci/<topic>` | GitHub Actions, Rulesets, or build tooling |
| `chore/<topic>` | Dependency updates, maintenance, housekeeping |
| `agent/<topic>` | AI coding agent autonomous task branches |
| `release/v<X.Y>` | Reserved for on-demand release maintenance branches |

### 2.3 Concurrent Work & AI Agents (`git worktree` + Stacked PRs)

When working on multiple tasks or running multiple AI coding agents concurrently, **isolate every task in its own `git worktree`** rather than sharing one working directory:

```bash
# Create an isolated worktree outside the main checkout
git worktree add ../lazyagents-wt-feat-foo -b feat/foo main

# Remove the worktree once its PR is merged
git worktree remove ../lazyagents-wt-feat-foo
git worktree prune
```

For large multi-layer changes, decompose work into small vertical slices (e.g., `Layer 1: types/schema` → `Layer 2: core logic` → `Layer 3: CLI/integration`) and choose the delivery cadence that matches your review flow:

| Mode | When to Use | Behavior Under Squash-Only `main` |
| :--- | :--- | :--- |
| **Sequential PRs (Default)** | Solo maintainer, fast review loop, or AI agent continuous delivery | Merge PR 1 → fast-forward `main` → branch PR 2 from new `main`. Zero Squash-SHA conflicts; every PR runs full CI immediately. |
| **Stacked PRs (Async Review)** | Multi-day human review where downstream slices must be written before upstream merges | Only the bottom PR (`base: main`) is **Ready**; open all downstream PRs as **Draft** (`gh pr create --draft`) with `Depends on #<pr>`. |

#### Squash-Only Stacked PR Restack Protocol

Because `main` uses **Squash and Merge only**, merging a parent PR creates a brand-new commit SHA on `main` and deletes the parent branch (causing GitHub to auto-retarget the child PR to `main`). Any child branch whose history still contains the parent's pre-squash commit SHA will immediately turn `CONFLICTING` (`DIRTY`) on overlapping files (`Cargo.toml`, `Cargo.lock`, `lib.rs`) because Git's three-way merge falls back to the older merge-base.

To prevent false conflicts:
1. **Keep downstream stacked PRs in Draft** (`gh pr create --draft`) until their upstream dependency merges and is restacked. (CI workflows intentionally target `pull_request: branches: [main]` so Draft downstream PRs do not waste duplicate runner minutes before restacking.)
2. **Never use GitHub's "Update branch" button or plain `git rebase origin/main`** on a stacked child PR whose parent was squash-merged—both replay the already-squashed parent commits against `main`.
3. **Always restack with `--onto` + `--update-refs`** right after the upstream PR squash-merges, skipping the old parent tip commit:
   ```bash
   git fetch origin --prune
   git rebase --update-refs --onto origin/main <old-parent-tip> <top-branch>
   git push --force-with-lease origin <child-branch-1> [<child-branch-2> ...]
   ```
4. Once the promoted PR targets `main` and shows `MERGEABLE`, mark it ready (`gh pr ready <pr>`) so all required CI checks run cleanly.
5. Recommended one-time local Git settings for smoother stacking:
   ```bash
   git config rebase.updateRefs true
   git config rerere.enabled true
   ```

---

## 3. Merge Strategy & Commit Hygiene

### 3.1 Repository Merge Settings

Configured on `lazyaix/lazyagents`:
- `allow_squash_merge = true`
- `allow_merge_commit = false`
- `allow_rebase_merge = false`
- `squash_merge_commit_title = "PR_TITLE"`
- `squash_merge_commit_message = "PR_BODY"`
- `delete_branch_on_merge = true`
- `allow_auto_merge = true`
- `allow_update_branch = true`

### 3.2 Conventional Commits 1.0.0

Every commit that lands on `main` originates from a Squash Merge whose title is taken from the PR title. Therefore:
- Intermediate WIP commits inside a PR branch do **not** need to follow Conventional Commits.
- Every **PR title** must satisfy:
  ```text
  <type>[optional (<scope>)][optional !]: <concise summary>
  ```
  - Allowed types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
  - Breaking changes: add `!` before `:` and describe migration impact under `BREAKING CHANGE:` in the PR body (preserved automatically into `main` via `squash_merge_commit_message = "PR_BODY"`).

> **Design Note on GitHub `commit_message_pattern` Ruleset**: GitHub's native `commit_message_pattern` Ruleset evaluates *every individual commit on the PR source branch* even under Squash Merge. To preserve frictionless WIP commits for external contributors and AI agents while keeping `main` 100% Conventional-Commit clean, enforcement is performed by the mandatory `PR Title` GitHub Actions check (`integration_id: 15368`) paired with Squash-only `PR_TITLE` and zero direct pushes to `main`.

---

## 4. GitHub Repository Rulesets & Governance

Legacy Branch Protection is not used; all governance is enforced via **GitHub Repository Rulesets**.

### 4.1 `Protect main` (Target: `~DEFAULT_BRANCH`, Status: `active`)

- **Bypass actors**: None (`[]`) — even Repository Admins cannot push directly to `main`.
- **Rules enabled**:
  - Block deletion (`deletion`)
  - Block force pushes (`non_fast_forward`)
  - Require linear history (`required_linear_history`)
  - Require pull request before merging (`pull_request`):
    - `required_approving_review_count`: `0` *(solo-maintainer bootstrap; external contributors still cannot merge because they lack Write permission)*
    - `dismiss_stale_reviews_on_push`: `true`
    - `required_review_thread_resolution`: `true`
    - `require_code_owner_review`: `false` *(enabled alongside review count `1` once a 2nd human maintainer joins)*
  - Require status checks to pass (`required_status_checks`):
    - `strict_required_status_checks_policy`: `true` (PR branch must be up to date with `main`)
    - Required GitHub Actions checks (`integration_id: 15368`):
      1. `PR Title`
      2. `Format`
      3. `Clippy`
      4. `Test`
      5. `Cargo Deny`
      6. `Zizmor`

### 4.2 `Protect release tags` (Target: `refs/tags/v*`, Status: `active`)

- **Bypass actors**: None (`[]`)
- **Rules enabled**:
  - Block tag updates (`update`)
  - Block tag deletions (`deletion`)
  - Block force pushes (`non_fast_forward`)
- **Creation**: Allowed so automated releases can create new `vX.Y.Z` tags; once created, tags are permanently immutable.

### 4.3 Security & Release Controls

- **Immutable Releases**: Enabled via `PUT /repos/lazyaix/lazyagents/immutable-releases` — locks published release tags and assets against post-publication modification or deletion.
- **Dependabot**: Vulnerability alerts, automated security updates, and `.github/dependabot.yml` weekly grouped PRs for `cargo` and `github-actions` with a 7-day supply-chain `cooldown` (`default-days: 7`, preserving 40-char SHA pinning).
- **GitHub Actions Supply Chain**:
  - All `uses:` references pinned to full 40-character commit SHAs with `# vX.Y.Z` version comments.
  - Top-level `permissions: {}` on every workflow; least-privilege per job.
  - `persist-credentials: false` on every `actions/checkout`.
  - Audited on every PR via `zizmor`.
- **Commit Signing**: Recommended via SSH signing keys (documented in `CONTRIBUTING.md`), not currently enforced as a hard Ruleset blocker.

### 4.4 GitHub Token Handling & Stateless Installation Tokens

GitHub App installation access tokens (`ghs_*`, including GitHub Actions `GITHUB_TOKEN` and tokens minted via `actions/create-github-app-token`) use a **stateless JWT format (~520 characters containing `.`)** rather than the legacy 40-character opaque format (the temporary `X-GitHub-Stateless-S2S-Token` per-request override header is retired on November 30, 2026). All workflows, tooling, and `lazyagents` code must follow these rules:

- **Opaque variable-length storage**: Always store and pass GitHub tokens as variable-length opaque strings (never cap columns/buffers at 40 or 255 characters, and never parse, decode, or rely on internal JWT claims).
- **Secret redaction in logs & agent traces**: Do not use legacy fixed-length patterns such as `ghs_[A-Za-z0-9]{36}`. Match the full stateless token character set and variable length (e.g., `ghs_[A-Za-z0-9._-]{36,}`) so installation tokens are never leaked into logs, traces, or LLM context.
- **No legacy override header**: Never send `X-GitHub-Stateless-S2S-Token` when requesting installation tokens.

---

## 5. Manual Tag-Only Multi-Platform Releases (`.github/workflows/release.yml`)

Version upgrades and releases happen **strictly when a human maintainer manually creates and pushes a semantic Git tag (`vX.Y.Z`)** — CI never automatically bumps `Cargo.toml` versions or opens release PRs on pushes to `main`.

1. **Tag Push Trigger (`on: push: tags: ["v*"]`)**:
   ```bash
   git checkout main && git pull --ff-only
   git tag -a v0.1.1 -m "Release v0.1.1"
   git push origin v0.1.1
   ```
2. **5-Platform Build & Provenance**: `.github/workflows/release.yml` compiles optimized stripped binaries (`--profile dist --locked`) with `LAZYAGENTS_RELEASE_VERSION="${TAG_NAME#v}"` across `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, and `x86_64-pc-windows-msvc`, generates `checksums.txt`, `lazyagents-installer.sh`, and a multi-platform Homebrew Formula (`lazyagents.rb`), signs SLSA build provenance (`actions/attest-build-provenance`), and publishes an **Immutable GitHub Release** atomically.
3. **Optional Automatic Homebrew Tap Sync (`lazyaix/homebrew-tap`)**:
   - Create `lazyaix/homebrew-tap` under `lazyaix` and install a repo-scoped GitHub App (Webhook **Inactive**; Repository permissions: **Contents: Read & Write**).
   - Configure repository Actions variable `HOMEBREW_TAP_APP_CLIENT_ID` and secret `HOMEBREW_TAP_APP_PRIVATE_KEY` on `lazyaix/lazyagents`.
   - On every `v*` tag release, `publish-homebrew-tap` commits `Formula/lazyagents.rb` directly to `lazyaix/homebrew-tap` so `brew install lazyaix/tap/lazyagents` updates immediately. Until configured, `publish-homebrew-tap` skips cleanly while `lazyagents.rb` remains attached to the GitHub Release assets.
4. **Optional Manual `release-plz`**: `.github/workflows/release-plz.yml` is retained strictly as manual `workflow_dispatch` only (`git_release_enable = false`, `git_tag_enable = false` in `release-plz.toml`).

---

## 6. Upgrade Playbooks (Zero-Friction Evolution)

### A. Scaling to 2+ Human Maintainers
Update `.github/CODEOWNERS` with the new maintainers/team, then update the `Protect main` Ruleset:
- Set `required_approving_review_count` from `0` → `1`
- Set `require_code_owner_review` from `false` → `true`
- *(Optional)* Add Repository Admin bypass with `bypass_mode: "pull_request"` if emergency hotfix override is desired while still forbidding direct pushes to `main`.

### B. Enabling GitHub Merge Queue (High-Concurrency Multi-Developer / Multi-Agent Phase)
All CI workflows (`ci.yml`, `security.yml`, `pr-title.yml`) already declare the `merge_group` trigger. To activate Merge Queue:
- Add a `merge_queue` rule (`merge_method: "SQUASH"`, `grouping_strategy: "ALLGREEN"`) to the `Protect main` Ruleset.

### C. Enabling `crates.io` Publishing (OIDC Trusted Publishing)
1. Link `lazyaix/lazyagents` (`.github/workflows/release-plz.yml`) in `crates.io` Trusted Publishing settings (`id-token: write` is already declared on `release-plz-release`).
2. Set `git_only = false` and `publish = true` in `release-plz.toml` (and `semver_check = true` once library APIs exist).

### D. Adding Multi-Platform Binary Distribution (`cargo-dist`)
1. Run `cargo dist init` and pin all generated workflow actions to 40-char SHAs.
2. Set `git_release_enable = false` in `release-plz.toml` so `release-plz` creates the `vX.Y.Z` tag and `cargo-dist` creates the **Immutable GitHub Release** atomically with cross-platform binaries and SLSA artifact attestations.
