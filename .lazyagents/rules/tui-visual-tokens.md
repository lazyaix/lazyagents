# L4 Cyber TUI Visual Token & Snapshot Standards

- Never hardcode `Color::*` in widgets; resolve all foreground/background/border/badge colors through `&Theme`.
- Verify every modal or view gracefully degrades across `TrueColor`, `256`, `ANSI-16`, `NO_COLOR`, and `Nerd Font → Unicode → ASCII`.
- Compute cursor offsets and line wrapping strictly via grapheme clusters (`unicode-segmentation`) and `unicode-width` so CJK and Emoji render without panics or clipping.
- Add `TestBackend` + `insta` snapshot tests for new visual widgets or modal flows and never silently overwrite unrelated existing snapshots.
