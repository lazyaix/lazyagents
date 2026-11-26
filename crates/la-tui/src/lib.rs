//! L4 Cyber Flagship terminal user interface for `lazyagents`.
//!
//! This crate depends strictly on [`la_core`] and must never depend on `la-runtime`,
//! ensuring fast compilation and deterministic headless visual tests.

mod config;
mod status;

pub use crate::config::{
    ColorProfile, DEFAULT_ACTIVE_FPS, GlyphSet, MotionLevel, TuiConfig, ViewportMode,
};
pub use crate::status::status_label;
