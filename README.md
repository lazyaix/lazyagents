# lazyagents

Open-source AI agents toolkit and terminal workbench in Rust, hosted at [lazyaix/lazyagents](https://github.com/lazyaix/lazyagents).

## Workspace Architecture

`lazyagents` is organized as a four-layer Cargo workspace (`edition = "2024"`, Rust `1.100.0`):

- `crates/la-core` — Pure Sans-I/O domain types, state machines, events, and secret redaction
- `crates/la-runtime` — Tokio async runtime, LLM providers, tools, and embedded PTY execution
- `crates/la-tui` — Ratatui 0.30 + TachyonFX L4 terminal workbench, themes, and modern terminal protocols
- `crates/la-cli` — `lazyagents` binary entrypoint, CLI parsing, and wiring

## Development

```bash
cargo fmt --all -- --check
CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
```

## Documentation & Governance

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
