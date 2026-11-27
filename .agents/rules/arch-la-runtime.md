---
trigger: glob
globs: **/crates/la-runtime/**
description: Architecture boundary for la-runtime
---

# la-runtime Layer Boundary

Depends only on la-core. Never cross-depend on la-tui. Handles atomic filesystem writes, git probing, and structured Tokio tasks.
