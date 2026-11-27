---
trigger: glob
globs: **/crates/la-core/**
description: Architecture boundary for la-core
---

# la-core Layer Boundary

Pure Sans-I/O domain core (M-IMPL-IO). Zero filesystem, network, PTY, Tokio, or Ratatui dependencies.
