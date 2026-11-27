# AI Agent Engineering & Git Workflow Guidelines

All AI coding agents working in `lazyaix/lazyagents` must follow these GitHub-first workflow and Rust/TUI engineering rules. This file is compiled deterministically from `.lazyagents/lazyagents.toml` (run `lazyagents generate` after SSOT edits; never edit generated outputs manually). For full details, read:
- [docs/architecture.md](docs/architecture.md) — Product positioning, Hybrid SSOT, Antigravity-first compiler, engineering governance, and M0–M4 roadmap
- [docs/git-workflow.md](docs/git-workflow.md) — Git collaboration, Rulesets, CI, and release governance
- [docs/rust-standards.md](docs/rust-standards.md) — Rust 1.100 engineering standards, four-layer workspace, lints, patterns, and testing
- [docs/tui-architecture.md](docs/tui-architecture.md) — L4 Cyber Flagship Ratatui 0.30 + TachyonFX TUI Config Studio architecture and terminal protocols

## Git Workflow & Branch Isolation

- **Never push directly to `main`**; use short-lived branches (`feat/, fix/, refactor/, docs/, test/, ci/, chore/, agent/`).
- **Use `git worktree`** for concurrent agent tasks (`../lazyagents-wt-{task}`) so working trees and build caches never collide.
- **Merge Strategy**: `squash-only` with Conventional Commits (`pr-title`).
- Keep PRs focused on one vertical slice. Under `squash-only` merge, prefer **Sequential PRs** (merge PR 1 → sync `main` → branch PR 2); when stacking for async review, open downstream PRs as **Draft** with `Depends on #<pr>`, never use plain `git rebase` or GitHub *Update branch*, and restack after upstream squash-merges via `git rebase --update-refs --onto origin/main <old-parent-tip> <top-branch>` + `git push --force-with-lease`.
- **Protected Paths** (require explicit human review): `.github/**`, `deny.toml`, `release-plz.toml`, `SECURITY.md`, `.lazyagents/**`.

## Rust & TUI Engineering Rules (Non-Negotiable)

- **Toolchain & Edition**: Rust `1.100.0` (`rust-toolchain.toml`, `edition = "2024"`, `resolver = "3"`). Prefer native Rust 1.85–1.100 idioms (`!`, `DropGuard`, `assert_matches!`, `cfg_select!`, `if let` match guards, let-chains, Trait Upcasting, `std::io::pipe`) over legacy boilerplate or micro-crates.
- **Four-Layer Unidirectional Workspace**:
  - `la-core` (`crates/la-core`): **Pure Sans-I/O** (`M-IMPL-IO`). Zero network, filesystem, PTY, Tokio, or Ratatui dependencies.
  - `la-runtime` (`crates/la-runtime`): Depends **only** on `la-core`.
  - `la-tui` (`crates/la-tui`): Depends **only** on `la-core`. **Never** cross-depend between `la-runtime` and `la-tui`.
  - `lazyagents` (`crates/la-cli`): Binary entrypoint wiring `la-core`, `la-runtime`, and `la-tui`.
- **Module Layout, `lib.rs` Facade & `.rs` Splitting**:
  - Use modern `foo.rs` + `foo/bar.rs` layout; **never create `mod.rs`** (`clippy::mod_module_files = "warn"`) and avoid `foo/foo.rs` (`module_inception`). Keep nesting ≤ 3 levels per crate.
  - **`lib.rs` is a Strict Facade**: put only `//!` crate docs, `mod`/`pub mod` declarations, and curated explicit `pub use` re-exports (no domain structs, no business logic, no `pub use foo::*`, no `prelude::*`). Keep `crates/la-cli/src/main.rs` a thin shell (< 100 LOC).
  - Split `.rs` files vertically by domain concept (never create `structs.rs`, `enums.rs`, `types.rs`, `utils.rs`, `helpers.rs`, or `common.rs`; soft guide: 50–300 LOC sweet spot, >500 evaluate, >800 split).
  - Follow BLUF ordering inside every `.rs` file (`//!` → `mod` → `use` [`core`/`std`, external, `crate::`, `super::`] → types/consts → `impl` → private helpers → `#[cfg(test)] mod tests`). Use `crate::` across modules, never `super::super::`, never production `use foo::*;`.
  - Extract unit tests to sibling `foo/tests.rs` once they exceed ~300 LOC; place black-box integration tests under a single binary (`tests/integration/main.rs`).
- **Ultra-Strict Lint & Safety Red Lines**:
  - `unsafe_code = "forbid"` workspace-wide; `clippy::pedantic`, `clippy::nursery`, `clippy::mod_module_files`, `missing_docs`, and `missing_debug_implementations` are active.
  - **Production code forbids** `unwrap_used`, `expect_used`, `panic`, `unimplemented`, `exit`, `dbg_macro`, `print_stdout`, `print_stderr`, and `await_holding_lock` (test-only exceptions live in `clippy.toml`).
  - **Never use `#[allow(...)]`**: always use `#[expect(clippy::..., reason = "...")]` (`M-LINT-OVERRIDE-EXPECT`).
- **Errors, Async & Pure Native Rust Design Principles**:
  - Internal library crates use `thiserror` 2.0 structured error enums; CLI entrypoint uses `color-eyre` with a terminal-restoring panic hook.
  - Apply Parse Don't Validate, Make Illegal States Unrepresentable, Functional Core / Sans-I/O, progressive abstraction (`M-DI-HIERARCHY`), Rule of Three, Newtypes (`AgentId`, `RedactedSecret`), and RAII `DropGuard`.
  - Use structured Tokio concurrency (`CancellationToken`, `TaskTracker`/`JoinSet`, bounded channels); offload CPU-heavy work to `spawn_blocking`. Never emit `tracing` output to stdout/stderr while the TUI is active.
- **L4 TUI Rules**:
  - Use Ratatui 0.30.x + `TachyonFX` shaders + `Block::shadow` + `ratatui-image` + `tui-big-text`, with adaptive frame pacing and `motion = full | reduced | none`.
  - Never hardcode `Color::*` inside widgets — always read semantic tokens from `&Theme` with `TrueColor → 256 → ANSI-16 → NO_COLOR` and `Nerd Font → Unicode → ASCII` fallbacks.
  - Keep Markdown/syntax highlighting pure Rust (`pulldown-cmark` + `syntect` with `default-fancy`, avoiding C `onig`), compute widths via `unicode-segmentation` + `unicode-width`, and write `TestBackend` + `insta` snapshots for UI components.
- **Release & Tag Policy**: Version upgrades and releases happen strictly when a maintainer manually creates and pushes a Git tag (`vX.Y.Z`); never auto-bump versions or auto-create tags.

## Mandatory Local Verification & Supply-Chain Baseline

Run all required checks before pushing or updating a PR:

```bash
cargo fmt --all -- --check
CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
```

- **License Policy**: Permissive licenses only (`Apache-2.0, MIT, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, 0BSD, CC0-1.0`); reject Copyleft (MPL/GPL/AGPL) dependencies.
- **Publish Cooldown**: Enforce a 7-day minimum publish age for new dependency updates.

## GitHub Actions & Secret Hygiene

- Pin every `uses:` action in `.github/workflows/*.yml` to a **full 40-character commit SHA** with a `# vX.Y.Z` version comment.
- Declare top-level `permissions: {}` and minimal per-job permissions, and set `persist-credentials: false` on `actions/checkout`.
- Never use `pull_request_target` with untrusted PR checkouts or interpolate `${{ github.event.* }}` directly into `run:` scripts.
- Treat GitHub tokens (including ~520-char stateless `ghs_*` App installation tokens) as variable-length opaque strings and redact `ghs_[A-Za-z0-9._-]{36,}` in logs and traces.
- Never commit plaintext secrets, API keys, or `.env` files.
