---
trigger: glob
globs: **/crates/la-tui/**
description: Architecture boundary for la-tui
---

# la-tui Layer Boundary

Depends only on la-core. Never cross-depend on la-runtime and never hardcode Color::* inside widgets (always read &Theme tokens).
