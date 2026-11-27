# lazyagents

Rust-native, offline-first **Multi-Agent Project Configuration Studio, Engineering Governance Compiler, and Hook Policy Engine**, hosted at [lazyaix/lazyagents](https://github.com/lazyaix/lazyagents).

## Installation

### Homebrew (macOS & Linux — Recommended)

```bash
brew install lazyaix/tap/lazyagents
```

### Shell Installer (macOS & Linux)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/lazyaix/lazyagents/releases/latest/download/lazyagents-installer.sh | sh
```

### Prebuilt Archives (macOS / Linux / Windows)

Download signed platform archives (`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`) and SHA-256 `checksums.txt` directly from [GitHub Releases](https://github.com/lazyaix/lazyagents/releases).

> **Tip for Antigravity Hooks**: Installing `lazyagents` onto your system `PATH` (for example via Homebrew at `/opt/homebrew/bin/lazyagents`) enables real-time `.agents/hooks.json` guardrail enforcement inside Antigravity automatically.

## Quickstart

Launch the interactive **L4 Cyber Config Studio** in any repository:

```bash
lazyagents
```

Or drive the full headless / CI lifecycle from the command line:

```bash
lazyagents init --import          # Probe stack and bootstrap .lazyagents/ SSOT
lazyagents generate               # Compile AGENTS.md, .agents/**, and managed .github/* blocks
lazyagents diff --exit-code       # Inspect deterministic unified diffs
lazyagents check --strict         # Enforce LA001–LA017 compliance, budgets, secrets, and zero drift
lazyagents github audit           # Read-only live GitHub Rulesets & Immutable Releases audit via `gh`
```

### Key TUI Shortcuts

| Key | Action |
| :--- | :--- |
| `Ctrl+K` / `?` | Fuzzy Command Palette / Bilingual Keybindings Cheat Sheet |
| `1`–`9`, `0` | Jump across Dashboard, Engineering (7 domains), GitHub Audit, 8 Antigravity views, Sync & Diff |
| `a` / `A` / `E` / `d` | Add Resource / Add Monorepo `AGENTS.md` Plan / Edit Modal Form / Delete with Confirmation |
| `C` | Offline Built-in Catalog & Preset Picker (`AGENTS.md` presets, skills, GitHub MCP, scaffolds) |
| `i` / `e` | Native Grapheme Multiline Inline Editor / External `$VISUAL` or `$EDITOR` Handoff |
| `s` / `v` | Safe Sync (with External Conflict Guard) / Unified vs Side-by-Side Diff |
| `t` / `p` / `m` / `L` | Cycle Theme / `TachyonFX` Shader Preset / Motion Level / EN–中文 Language |

## What `lazyagents` Does

`lazyagents` manages project-level AI coding agent configurations from a single deterministic **Hybrid SSOT** (`.lazyagents/`)—shipping complete **Google Antigravity** support in V1 with an active M4/M5 roadmap expanding to a 5-target AAIF-first matrix (**Antigravity**, **Claude Code**, **OpenAI Codex**, **Cursor**, and **GitHub Copilot**):

- **Dynamic `AGENTS.md` Composition**: Auto-probes repository facts, renders modular templates (`minijinja`), supports Monorepo nested `AGENTS.md`, and enforces Antigravity's **24 KB per-file** and **20,000-token** always-on context budgets.
- **Complete Antigravity Project Suite**: Manages `AGENTS.md`, Conditional Rules (`.agents/rules/*.md`), Skills (`.agents/skills/`), Custom Subagents (`.agents/agents/*.md`), Lifecycle Hooks (`.agents/hooks.json`), MCP Servers (`.agents/mcp_config.json`), Index Manifests (`skills.json`/`rules.json`/`plugins.json`), and Project Plugins (`.agents/plugins/`).
- **Engineering Governance & Rust Hook Engine**: Declares Git workflows, protected paths, local verification gates, supply-chain rules, architecture layers, and GitHub hygiene once—compiling across `AGENTS.md`, pure-Rust cross-platform guardrail hooks (`lazyagents hook eval <id>`), collaboration Skills, opt-in `.github/*` templates, and read-only `gh` Rulesets audits (`lazyagents github audit`).
- **Dual-Mode Interface**: Launches an **L4 Cyber Flagship TUI Config Studio** (`en`/`zh` i18n, full native modal forms for all 8 Antigravity views & 7 Engineering domains, live preview/diff, budget radar, `$EDITOR` handoff) in interactive terminals and a **CI-grade Headless CLI** (`init`, `import`, `generate`, `diff`, `check`, `inspect`, `hook`, `github`) in non-TTY / automation pipelines.

## Workspace Architecture

`lazyagents` is organized as a four-layer Cargo workspace (`edition = "2024"`, Rust `1.100.0`):

- `crates/la-core` — Pure Sans-I/O canonical IR, scope resolver, template engine, target compilers, compliance/budget linters, hook policy evaluator, and secret redaction
- `crates/la-runtime` — Tokio async & filesystem shell: repository prober, importer, atomic whole-file/managed-block writer, local pack loader, read-only `gh` auditor, and `$EDITOR` handoff
- `crates/la-tui` — Ratatui 0.30 + TachyonFX L4 Config Studio, native shadowed form engine, semantic themes, `en`/`zh` i18n, and modern terminal protocols
- `crates/la-cli` — `lazyagents` binary entrypoint, headless CI/automation CLI, `hook eval` runner, and TUI launcher

## Development

```bash
cargo fmt --all -- --check
CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
cargo run --locked -- check --strict
```

## Documentation & Governance

- **Product & Architecture Specification**: [docs/architecture.md](docs/architecture.md)
- **Rust Engineering Standards**: [docs/rust-standards.md](docs/rust-standards.md)
- **TUI Architecture & Visual Spec**: [docs/tui-architecture.md](docs/tui-architecture.md)
- **GitHub Workflow Specification**: [docs/git-workflow.md](docs/git-workflow.md)
- **2026 AI Agent Methodology & Sources Research**: [docs/research/agent-methodology-2026.md](docs/research/agent-methodology-2026.md)
- **Contributing Guide**: [CONTRIBUTING.md](CONTRIBUTING.md)
- **AI Agent Guidelines**: [AGENTS.md](AGENTS.md)
- **Security Policy**: [SECURITY.md](SECURITY.md)

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
