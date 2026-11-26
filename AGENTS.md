# AI Agent Engineering & Git Workflow Guidelines

All AI coding agents working in `lazyaix/lazyagents` must follow these GitHub-first workflow and Rust/TUI engineering rules. For full details, read:
- [docs/git-workflow.md](docs/git-workflow.md) — Git collaboration, Rulesets, CI, and release governance
- [docs/rust-standards.md](docs/rust-standards.md) — Rust 1.100 engineering standards, four-layer workspace, lints, patterns, and testing
- [docs/tui-architecture.md](docs/tui-architecture.md) — L4 Cyber Flagship Ratatui 0.30 + TachyonFX TUI architecture and terminal protocols

---

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

## 3. Rust & TUI Engineering Rules (Non-Negotiable)

- **Toolchain & Edition**: Rust `1.100.0` (`rust-toolchain.toml`, `edition = "2024"`, `resolver = "3"`). Prefer native Rust 1.85–1.100 idioms (`!`, `DropGuard`, `assert_matches!`, `cfg_select!`, `if let` match guards, let-chains, Trait Upcasting, `std::io::pipe`) over legacy boilerplate or micro-crates.
- **Four-Layer Unidirectional Workspace**:
  - `la-core` (`crates/la-core`): **Pure Sans-I/O** (`M-IMPL-IO`). Zero network, filesystem, PTY, Tokio, or Ratatui dependencies.
  - `la-runtime` (`crates/la-runtime`): Depends **only** on `la-core`.
  - `la-tui` (`crates/la-tui`): Depends **only** on `la-core`. **Never** cross-depend between `la-runtime` and `la-tui`.
  - `lazyagents` (`crates/la-cli`): Binary entrypoint wiring `la-core`, `la-runtime`, and `la-tui`.
- **Module Layout, `lib.rs` Facade & `.rs` Splitting**:
  - Use modern `foo.rs` + `foo/bar.rs` layout; **never create `mod.rs`** (`clippy::mod_module_files = "warn"`) and avoid `foo/foo.rs` (`module_inception`). Keep nesting ≤ 3 levels per crate.
  - **`lib.rs` is a Strict Facade**: put only `//!` crate docs, `mod`/`pub mod` declarations, and curated explicit `pub use` re-exports (no domain structs, no business logic, no `pub use foo::*`, no `prelude::*`). Flatten high-frequency core types to crate root; keep multi-type subsystems under `pub mod`. Keep `crates/la-cli/src/main.rs` a thin shell (< 100 LOC).
  - Split `.rs` files vertically by domain concept (never create `structs.rs`, `enums.rs`, `types.rs`, `utils.rs`, `helpers.rs`, or `common.rs`). Split on independent invariant boundaries, distinct change axes, or `cfg`/feature isolation (soft guide: 50–300 LOC sweet spot, >500 evaluate, >800 split; avoid Java-style one-type-per-file fragmentation).
  - Follow BLUF ordering inside every `.rs` file (`//!` → `mod` → `use` [`core`/`std`, external, `crate::`, `super::`] → types/consts → `impl` [inherent, project traits, std traits] → private helpers → `#[cfg(test)] mod tests`). Use `crate::` across modules, `super::` only in tight parent/child or tests, never `super::super::`, never production `use foo::*;`. Follow least visibility (`private → pub(super) → pub(crate) → pub`).
  - Extract unit tests to sibling `foo/tests.rs` once they exceed ~300 LOC; place black-box integration tests under a single binary (`tests/integration/main.rs`).
- **Ultra-Strict Lint & Safety Red Lines**:
  - `unsafe_code = "forbid"` workspace-wide.
  - `clippy::pedantic`, `clippy::nursery`, `clippy::mod_module_files`, `missing_docs`, and `missing_debug_implementations` are active across the workspace.
  - **Production code forbids** `unwrap_used`, `expect_used`, `panic`, `unimplemented`, `exit`, `dbg_macro`, `print_stdout`, `print_stderr`, and `await_holding_lock` (test-only exceptions live in `clippy.toml`).
  - **Never use `#[allow(...)]`**: always use `#[expect(clippy::..., reason = "...")]` (`M-LINT-OVERRIDE-EXPECT`).
- **Errors, Async & Pure Native Rust Design Principles**:
  - Internal library crates use `thiserror` 2.0 structured error enums (never `String` errors, never leak third-party error types); CLI entrypoint uses `color-eyre` with a terminal-restoring panic hook.
  - Apply pure native Rust principles: high cohesion / single reason to change, **Parse Don't Validate**, **Make Illegal States Unrepresentable** (ADTs + TypeState), Functional Core / Sans-I/O (`M-IMPL-IO`), progressive abstraction (`M-DI-HIERARCHY`: concrete → enum/static generics/AFIT → small orthogonal boundary `dyn Trait`; never invent single-impl traits), Rule of Three (WET → DRY) + YAGNI/KISS, `&self` vs `&mut self` CQS & encapsulation, Newtype (`AgentId`, `RedactedSecret`), and RAII `DropGuard`.
  - Use structured Tokio concurrency (`CancellationToken`, `TaskTracker`/`JoinSet`, bounded channels); offload CPU-heavy work (highlighting, diffs, image decoding) to `spawn_blocking`.
  - Never emit `tracing` output to stdout/stderr while the TUI is active; route to file + in-TUI ring buffer with mandatory secret redaction.
- **L4 TUI Rules**:
  - Use Ratatui 0.30.x + `TachyonFX` shaders + `Block::shadow` + `ratatui-image` + `tui-big-text`, with adaptive frame pacing (0% CPU idle, 60 FPS active) and `motion = full | reduced | none`.
  - Never hardcode `Color::*` inside widgets — always read semantic tokens from `&Theme` with `TrueColor → 256 → ANSI-16 → NO_COLOR` and `Nerd Font → Unicode → ASCII` fallbacks.
  - Keep Markdown/syntax highlighting pure Rust (`pulldown-cmark` + `syntect` with `default-features = false, features = ["default-fancy"]`, avoiding C `onig`), and reject all Copyleft (MPL/GPL) dependencies in `deny.toml`.
  - Compute text widths strictly with grapheme clusters (`unicode-segmentation`) + `unicode-width` (CJK/Emoji safe), and write `TestBackend` + `insta` snapshots for new UI components.

## 4. Mandatory Local Verification Before Pushing

Run and ensure all checks pass before pushing or updating a PR:

```bash
cargo fmt --all -- --check
CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
```

## 5. CI Self-Remediation & Security Hygiene

- After pushing a PR branch, monitor all required GitHub Actions checks (`PR Title`, `Format`, `Clippy`, `Test`, `Cargo Deny`, `Zizmor`) plus advisory/extended checks (`Docs`, `Cross-Platform`, `Coverage`). Fix any CI failures directly on the branch until all checks pass.
- When editing `.github/workflows/*.yml`:
  - Pin every `uses:` action to a **full 40-character commit SHA** with a `# vX.Y.Z` comment.
  - Keep top-level `permissions: {}` and declare minimal per-job permissions.
  - Set `persist-credentials: false` on `actions/checkout`.
  - Never use `pull_request_target` with untrusted PR code, and never interpolate untrusted `${{ github.event.* }}` into shell `run:` scripts.
- When handling GitHub tokens in Rust code, agent tools, or log/trace redactors:
  - Treat all GitHub tokens (including ~520-char stateless `ghs_*` App installation tokens) as variable-length opaque strings—never enforce 40/255-char storage caps, never decode JWT payloads, and never use the retired `X-GitHub-Stateless-S2S-Token` header.
  - Ensure secret-redaction regexes match `ghs_[A-Za-z0-9._-]{36,}` (not legacy `ghs_[A-Za-z0-9]{36}`) so stateless tokens never leak into logs or LLM context.
- Never commit secrets, tokens, or `.env` files.
