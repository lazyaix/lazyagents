//! L4 Ultra Cyberpunk HUD `TachyonFX` post-processing shader pipeline.
//!
//! Manages deterministic, `Duration`-stepped terminal buffer shaders (splash scanline/coalesce,
//! view sweep transitions, continuous neon-pulsing focus borders, hook-verdict chromatic flashes,
//! and floating modal dimming scrims) while honoring [`MotionLevel`], focus-lost throttling,
//! and `NO_COLOR`.

use core::fmt;
use core::time::Duration as StdDuration;

use la_core::PolicyDecision;
use ratatui::buffer::Buffer;
use ratatui::layout::{Margin, Rect};
use tachyonfx::{CellFilter, Duration as FxDuration, EffectManager, Interpolation, Motion, fx};

use crate::config::{ColorProfile, MotionLevel, ShaderPreset};
use crate::theme::Theme;

/// Named unique shader channels so new animations cleanly supersede stale ones in the same slot.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EffectSlot {
    /// Startup splash entrance / coalesce animation.
    #[default]
    Splash,
    /// Primary workspace view transition sweep / coalesce.
    ViewTransition,
    /// Continuous neon-pulse HSL glow on focused pane borders (`CyberOverdrive` / `AuroraSweep`).
    FocusBorderPulse,
    /// Hook Playground or Sync result chromatic verdict flash.
    VerdictFlash,
    /// Floating modal entrance fade / coalesce.
    ModalEntrance,
}

/// Stateful `TachyonFX` shader coordinator owned by the studio presentation layer.
pub struct StudioEffects {
    manager: EffectManager<EffectSlot>,
    preset: ShaderPreset,
    motion: MotionLevel,
    focused: bool,
    elapsed_ms: u64,
}

impl fmt::Debug for StudioEffects {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StudioEffects")
            .field("preset", &self.preset)
            .field("motion", &self.motion)
            .field("focused", &self.focused)
            .field("is_running", &self.manager.is_running())
            .field("elapsed_ms", &self.elapsed_ms)
            .finish()
    }
}

impl StudioEffects {
    /// Creates a new [`StudioEffects`] pipeline with the specified preset and motion level.
    #[must_use]
    pub fn new(preset: ShaderPreset, motion: MotionLevel) -> Self {
        Self {
            manager: EffectManager::default(),
            preset,
            motion,
            focused: true,
            elapsed_ms: 0,
        }
    }

    /// Returns the active [`ShaderPreset`].
    #[must_use]
    pub const fn preset(&self) -> ShaderPreset {
        self.preset
    }

    /// Returns the active [`MotionLevel`].
    #[must_use]
    pub const fn motion(&self) -> MotionLevel {
        self.motion
    }

    /// Updates the active [`ShaderPreset`].
    pub const fn set_preset(&mut self, preset: ShaderPreset) {
        self.preset = preset;
    }

    /// Updates the active [`MotionLevel`]; switching to [`MotionLevel::None`] clears running effects.
    pub fn set_motion(&mut self, motion: MotionLevel) {
        self.motion = motion;
        if motion == MotionLevel::None {
            self.clear_all();
        }
    }

    /// Updates terminal window focus state so ambient shaders throttle/pause when unfocused.
    pub const fn set_window_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Returns `true` when the adaptive frame ticker should run at active FPS (false => 0% CPU idle).
    #[must_use]
    pub fn needs_animation_tick(&self, theme: &Theme) -> bool {
        if self.motion == MotionLevel::None || theme.color_profile == ColorProfile::NoColor {
            return false;
        }
        if !self.focused {
            return false;
        }
        self.manager.is_running()
            || (self.motion == MotionLevel::Full
                && matches!(
                    self.preset,
                    ShaderPreset::CyberOverdrive | ShaderPreset::AuroraSweep
                ))
    }

    /// Clears all active one-shot and continuous shader effects immediately.
    pub fn clear_all(&mut self) {
        self.manager = EffectManager::default();
    }

    /// Triggers the startup splash shader sequence (completed in < 550ms, skippable anytime).
    pub fn trigger_splash(&mut self, area: Rect, theme: &Theme) {
        if !self.can_animate(theme) {
            return;
        }
        let effect = match (self.motion, self.preset) {
            (MotionLevel::Reduced, _) | (_, ShaderPreset::CrispMinimal) => {
                fx::fade_from_fg(theme.bg_canvas, (180, Interpolation::SineOut)).with_area(area)
            }
            (MotionLevel::Full, ShaderPreset::AuroraSweep) => fx::sweep_in(
                Motion::LeftToRight,
                14,
                0,
                theme.bg_canvas,
                (420, Interpolation::QuadOut),
            )
            .with_area(area),
            (MotionLevel::Full, ShaderPreset::CyberOverdrive) => fx::parallel(&[
                fx::coalesce((380, Interpolation::QuadOut)),
                fx::sweep_in(
                    Motion::UpToDown,
                    10,
                    2,
                    theme.bg_canvas,
                    (460, Interpolation::CubicOut),
                ),
                fx::hsl_shift_fg([35.0, 12.0, 8.0], (480, Interpolation::SineInOut)),
            ])
            .with_area(area),
            (MotionLevel::None, _) => return,
        };
        self.manager.add_unique_effect(EffectSlot::Splash, effect);
    }

    /// Triggers a crisp directional transition when switching views in the Command Deck.
    pub fn trigger_view_transition(&mut self, area: Rect, theme: &Theme) {
        if !self.can_animate(theme) {
            return;
        }
        let effect = match (self.motion, self.preset) {
            (MotionLevel::Reduced, _) | (_, ShaderPreset::CrispMinimal) => {
                fx::fade_from_fg(theme.bg_surface, (110, Interpolation::SineOut)).with_area(area)
            }
            (MotionLevel::Full, ShaderPreset::AuroraSweep) => fx::sweep_in(
                Motion::LeftToRight,
                10,
                0,
                theme.bg_surface,
                (220, Interpolation::QuadOut),
            )
            .with_area(area),
            (MotionLevel::Full, ShaderPreset::CyberOverdrive) => fx::parallel(&[
                fx::sweep_in(
                    Motion::LeftToRight,
                    12,
                    2,
                    theme.bg_surface,
                    (240, Interpolation::CubicOut),
                ),
                fx::coalesce((180, Interpolation::QuadOut)),
            ])
            .with_area(area),
            (MotionLevel::None, _) => return,
        };
        self.manager
            .add_unique_effect(EffectSlot::ViewTransition, effect);
    }

    /// Triggers a dramatic chromatic verdict flash (used by Hook Policy Playground & Sync actions).
    pub fn trigger_verdict_flash(&mut self, area: Rect, decision: PolicyDecision, theme: &Theme) {
        if !self.can_animate(theme) {
            return;
        }
        let flash_color = match decision {
            PolicyDecision::Allow => theme.status_success,
            PolicyDecision::Ask | PolicyDecision::ForceAsk => theme.status_approval,
            PolicyDecision::Deny => theme.status_error,
        };
        let duration_ms = match self.motion {
            MotionLevel::Full => 320,
            MotionLevel::Reduced => 140,
            MotionLevel::None => return,
        };
        let effect =
            fx::fade_from_fg(flash_color, (duration_ms, Interpolation::QuadOut)).with_area(area);
        self.manager
            .add_unique_effect(EffectSlot::VerdictFlash, effect);
    }

    /// Triggers a subtle entrance transition on an opened floating modal.
    pub fn trigger_modal_entrance(&mut self, area: Rect, theme: &Theme) {
        if !self.can_animate(theme) {
            return;
        }
        let duration_ms = match (self.motion, self.preset) {
            (MotionLevel::Full, ShaderPreset::CyberOverdrive) => 200,
            (MotionLevel::Full, ShaderPreset::AuroraSweep) => 170,
            (MotionLevel::Full, ShaderPreset::CrispMinimal) | (MotionLevel::Reduced, _) => 100,
            (MotionLevel::None, _) => return,
        };
        let effect = fx::fade_from_fg(
            theme.bg_surface_elevated,
            (duration_ms, Interpolation::QuadOut),
        )
        .with_area(area);
        self.manager
            .add_unique_effect(EffectSlot::ModalEntrance, effect);
    }

    /// Applies a dimmed modal scrim over `area` before floating modals are rendered on top.
    pub fn apply_modal_scrim(&self, buf: &mut Buffer, area: Rect, theme: &Theme) {
        if theme.color_profile == ColorProfile::NoColor {
            return;
        }
        let mut scrim = fx::hsl_shift(Some([0.0, -35.0, -30.0]), Some([0.0, -25.0, -35.0]), 1);
        scrim.process(FxDuration::from_millis(1), buf, area);
    }

    /// Advances all active `TachyonFX` shaders by a deterministic duration `dt` and renders them
    /// directly onto `buf`, plus applies ambient neon border breathing on `focused_pane` when active.
    pub fn process_frame(
        &mut self,
        dt: StdDuration,
        buf: &mut Buffer,
        viewport: Rect,
        focused_pane: Option<Rect>,
        theme: &Theme,
    ) {
        if self.motion == MotionLevel::None || theme.color_profile == ColorProfile::NoColor {
            return;
        }

        let dt_ms = u32::try_from(dt.as_millis()).unwrap_or(u32::MAX);
        self.elapsed_ms = self.elapsed_ms.saturating_add(u64::from(dt_ms));

        if self.focused && self.motion == MotionLevel::Full {
            self.apply_ambient_border_pulse(buf, focused_pane);
        }

        self.manager
            .process_effects(FxDuration::from_millis(dt_ms), buf, viewport);
    }

    const fn can_animate(&self, theme: &Theme) -> bool {
        !matches!(self.motion, MotionLevel::None)
            && !matches!(theme.color_profile, ColorProfile::NoColor)
    }

    fn apply_ambient_border_pulse(&self, buf: &mut Buffer, focused_pane: Option<Rect>) {
        let Some(pane) = focused_pane else {
            return;
        };
        if pane.width < 3 || pane.height < 3 {
            return;
        }

        let (hue_amp, light_amp, period_ms) = match self.preset {
            ShaderPreset::CyberOverdrive => (18.0_f32, 9.0_f32, 2_400_u64),
            ShaderPreset::AuroraSweep => (12.0_f32, 5.0_f32, 3_600_u64),
            ShaderPreset::CrispMinimal => return,
        };

        let phase_ms = self.elapsed_ms.checked_rem(period_ms).unwrap_or(0);
        #[expect(
            clippy::cast_precision_loss,
            reason = "phase_ms is bounded by period_ms (<= 3600) so f32 conversion is exact"
        )]
        let ratio = (phase_ms as f32) / (period_ms as f32);
        let wave = (ratio * core::f32::consts::TAU).sin();

        let mut pulse = fx::hsl_shift_fg([wave * hue_amp, 0.0, wave * light_amp], 1)
            .with_filter(CellFilter::Outer(Margin::new(1, 1)));
        pulse.process(FxDuration::from_millis(1), buf, pane);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Block;

    use crate::config::GlyphSet;
    use crate::theme::ResolvedTheme;

    use super::*;

    #[test]
    fn deterministic_shader_stepping_completes_cleanly_on_test_backend() {
        let theme = Theme::build(
            ResolvedTheme::CyberNeon,
            ColorProfile::TrueColor,
            GlyphSet::Unicode,
        );
        let mut effects = StudioEffects::new(ShaderPreset::CyberOverdrive, MotionLevel::Full);
        let area = Rect::new(0, 0, 40, 12);

        effects.trigger_splash(area, &theme);
        effects.trigger_verdict_flash(area, PolicyDecision::Deny, &theme);
        assert!(effects.needs_animation_tick(&theme));

        let backend = TestBackend::new(40, 12);
        let mut terminal = Terminal::new(backend).unwrap();

        for _ in 0..35 {
            terminal
                .draw(|frame| {
                    let block = Block::bordered()
                        .title("HUD")
                        .style(theme.surface_style())
                        .border_style(theme.border_style(true));
                    frame.render_widget(block, area);
                    effects.process_frame(
                        StdDuration::from_millis(20),
                        frame.buffer_mut(),
                        area,
                        Some(area),
                        &theme,
                    );
                })
                .unwrap();
        }

        effects.set_motion(MotionLevel::None);
        assert!(!effects.needs_animation_tick(&theme));
    }

    #[test]
    fn no_color_and_unfocused_window_suppress_animation_ticks() {
        let no_color = Theme::build(
            ResolvedTheme::CyberNeon,
            ColorProfile::NoColor,
            GlyphSet::Ascii,
        );
        let mut effects = StudioEffects::new(ShaderPreset::CyberOverdrive, MotionLevel::Full);
        effects.trigger_view_transition(Rect::new(0, 0, 20, 8), &no_color);
        assert!(!effects.needs_animation_tick(&no_color));

        let color_theme = Theme::build(
            ResolvedTheme::CyberNeon,
            ColorProfile::TrueColor,
            GlyphSet::NerdFont,
        );
        assert!(effects.needs_animation_tick(&color_theme));
        effects.set_window_focused(false);
        assert!(!effects.needs_animation_tick(&color_theme));
    }
}
