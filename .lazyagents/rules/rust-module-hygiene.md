# Rust 1.100 Module Hygiene & Zero-Panic Red Lines

- Never create `mod.rs`; use `foo.rs` + `foo/bar.rs` with nesting ≤ 3 levels per crate.
- Keep `lib.rs` strictly as a facade (`//!` docs, module declarations, curated explicit `pub use`).
- Never use `#[allow(...)]`; use `#[expect(clippy::..., reason = "...")]` when an exception is justified.
- Production code must never call `unwrap()`, `expect()`, `panic!()`, `unimplemented!()`, `dbg!()`, `println!()`, `eprintln!()`, or hold a lock across `.await`.
- Keep every `.rs` source file under 800 LOC and functions under Clippy's `too_many_lines` (120 LOC) threshold.
