---
trigger: glob
globs: **/crates/la-cli/**
description: Architecture boundary for la-cli
---

# la-cli Layer Boundary

Thin binary entrypoint (main.rs < 100 LOC) wiring la-core, la-runtime, and la-tui. Never emit tracing output to stdout/stderr while TUI is active.
