# lazyagents

Rust-native, offline-first **Multi-Agent Project Configuration Studio, Engineering Governance Compiler, and Hook Policy Engine**, hosted at [lazyaix/lazyagents](https://github.com/lazyaix/lazyagents).

## What `lazyagents` Does

`lazyagents` manages project-level AI coding agent configurations from a single deterministic **Hybrid SSOT** (`.lazyagents/`)—starting with full **Google Antigravity** support while reserving clean interfaces for **Claude Code**, **OpenAI Codex**, and **GitHub Copilot**:

- **Dynamic `AGENTS.md` Composition**: Auto-probes repository facts, renders modular templates (`minijinja`), supports Monorepo nested `AGENTS.md`, and enforces Antigravity's **24 KB per-file** and **20,000-token** always-on context budgets.
- **Complete Antigravity Project Suite**: Manages `AGENTS.md`, Conditional Rules (`.agents/rules/*.md`), Skills (`.agents/skills/`), Custom Subagents (`.agents/agents/*.md`), Lifecycle Hooks (`.agents/hooks.json`), MCP Servers (`.agents/mcp_config.json`), Index Manifests (`skills.json`/`rules.json`/`plugins.json`), and Project Plugins (`.agents/plugins/`).
- **Engineering Governance & Rust Hook Engine**: Declares Git workflows, protected paths, local verification gates, supply-chain rules, architecture layers, and GitHub hygiene once—compiling across `AGENTS.md`, pure-Rust cross-platform guardrail hooks (`lazyagents hook eval <id>`), collaboration Skills, opt-in `.github/*` templates, and read-only `gh` Rulesets audits (`lazyagents github audit`).
- **Dual-Mode Interface**: Launches an **L4 Cyber Flagship TUI Config Studio** (`en`/`zh` i18n, live preview/diff, budget radar, `$EDITOR` handoff) in interactive terminals and a **CI-grade Headless CLI** (`init`, `import`, `generate`, `diff`, `check`, `inspect`, `hook`, `github`) in non-TTY / automation pipelines.

## Workspace Architecture

`lazyagents` is organized as a four-layer Cargo workspace (`edition = "2024"`, Rust `1.100.0`):

- `crates/la-core` — Pure Sans-I/O canonical IR, scope resolver, template engine, target compilers, compliance/budget linters, hook policy evaluator, and secret redaction
- `crates/la-runtime` — Tokio async & filesystem shell: repository prober, importer, atomic whole-file/managed-block writer, local pack loader, read-only `gh` auditor, and `$EDITOR` handoff
- `crates/la-tui` — Ratatui 0.30 + TachyonFX L4 Config Studio, semantic themes, `en`/`zh` i18n, and modern terminal protocols
- `crates/la-cli` — `lazyagents` binary entrypoint, headless CI/automation CLI, `hook eval` runner, and TUI launcher

## Development

```bash
cargo fmt --all -- --check
CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
```

## Documentation & Governance

- **Product & Architecture Specification**: [docs/architecture.md](docs/architecture.md)
- **Rust Engineering Standards**: [docs/rust-standards.md](docs/rust-standards.md)
- **TUI Architecture & Visual Spec**: [docs/tui-architecture.md](docs/tui-architecture.md)
- **GitHub Workflow Specification**: [docs/git-workflow.md](docs/git-workflow.md)
- **Contributing Guide**: [CONTRIBUTING.md](CONTRIBUTING.md)
- **AI Agent Guidelines**: [AGENTS.md](AGENTS.md)
- **Security Policy**: [SECURITY.md](SECURITY.md)

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
