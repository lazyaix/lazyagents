//! Pure Sans-I/O terminal graphics adapter powered by `ratatui-image` 11
//! (Kitty -> iTerm2 -> Sixel -> Unicode Halfblocks fallback).
//!
//! Heavy PNG/JPEG/WebP file decoding runs off the render thread in `la-cli` via
//! `tokio::task::spawn_blocking`, passing raw [`DecodedRgbaImage`] buffers into `la-tui`.

use core::fmt;

use image::{DynamicImage, ImageBuffer, Rgba};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

use crate::error::TuiError;
use crate::protocol::GraphicsProtocolKind;

/// Pre-decoded RGBA8 pixel buffer passed across the `la-cli` -> `la-tui` boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedRgbaImage {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Tightly packed RGBA8 pixel data (`width * height * 4` bytes).
    pub rgba: Vec<u8>,
}

/// Stateful `ratatui-image` renderer encapsulating protocol encoding without leaking upstream types.
pub struct PluginLogoPreview {
    protocol: StatefulProtocol,
    kind: GraphicsProtocolKind,
}

impl fmt::Debug for PluginLogoPreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginLogoPreview")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl DecodedRgbaImage {
    /// Decodes raw PNG, JPEG, or WebP bytes in-memory into a [`DecodedRgbaImage`].
    ///
    /// # Errors
    ///
    /// Returns [`TuiError::Highlight`] if the image payload is corrupt or unsupported.
    pub fn decode_bytes(bytes: &[u8]) -> Result<Self, TuiError> {
        let dyn_img = image::load_from_memory(bytes).map_err(|err| TuiError::Highlight {
            message: format!("failed to decode plugin logo image: {err}"),
        })?;
        let rgba = dyn_img.to_rgba8();
        Ok(Self {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        })
    }

    /// Generates a crisp 24x24 procedural Ultra Cyberpunk neon emblem for preview/fallback rendering.
    #[must_use]
    pub fn cyber_emblem() -> Self {
        let size = 24u32;
        let mut rgba = Vec::with_capacity(2_304);
        for y in 0..size {
            for x in 0..size {
                let border =
                    x <= 1 || y <= 1 || x >= size.saturating_sub(2) || y >= size.saturating_sub(2);
                let diag = x == y || x.saturating_add(y) == size.saturating_sub(1);
                let core = (8..16).contains(&x) && (8..16).contains(&y);
                let pixel = if border {
                    [0u8, 240, 255, 255]
                } else if core {
                    [255u8, 42, 133, 255]
                } else if diag {
                    [57u8, 255, 136, 255]
                } else {
                    [13u8, 17, 33, 255]
                };
                rgba.extend_from_slice(&pixel);
            }
        }
        Self {
            width: size,
            height: size,
            rgba,
        }
    }
}

impl PluginLogoPreview {
    /// Wraps a pre-decoded [`DecodedRgbaImage`] into a stateful `ratatui-image` protocol renderer.
    ///
    /// # Errors
    ///
    /// Returns [`TuiError::Highlight`] if the RGBA buffer dimensions do not match `width * height * 4`.
    pub fn from_rgba(
        image: &DecodedRgbaImage,
        kind: GraphicsProtocolKind,
    ) -> Result<Self, TuiError> {
        let buf: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(image.width, image.height, image.rgba.clone()).ok_or_else(
                || TuiError::Highlight {
                    message: "RGBA buffer size does not match image dimensions".to_owned(),
                },
            )?;
        let dyn_img = DynamicImage::ImageRgba8(buf);
        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(match kind {
            GraphicsProtocolKind::Kitty => ProtocolType::Kitty,
            GraphicsProtocolKind::Iterm2 => ProtocolType::Iterm2,
            GraphicsProtocolKind::Sixel => ProtocolType::Sixel,
            GraphicsProtocolKind::Halfblocks => ProtocolType::Halfblocks,
        });
        let protocol = picker.new_resize_protocol(dyn_img);
        Ok(Self { protocol, kind })
    }

    /// Returns the active terminal graphics protocol tier.
    #[must_use]
    pub const fn kind(&self) -> GraphicsProtocolKind {
        self.kind
    }

    /// Renders the scaled plugin logo into `area` using `Resize::Fit`.
    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        if area.width < 2 || area.height < 2 {
            return;
        }
        let widget = StatefulImage::default().resize(Resize::Fit(None));
        frame.render_stateful_widget(widget, area, &mut self.protocol);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    #[test]
    fn renders_procedural_cyber_emblem_via_halfblocks_on_test_backend() {
        let emblem = DecodedRgbaImage::cyber_emblem();
        let mut preview =
            PluginLogoPreview::from_rgba(&emblem, GraphicsProtocolKind::Halfblocks).unwrap();
        assert_eq!(preview.kind(), GraphicsProtocolKind::Halfblocks);

        let mut terminal = Terminal::new(TestBackend::new(24, 12)).unwrap();
        terminal
            .draw(|frame| preview.render(frame, Rect::new(0, 0, 16, 8)))
            .unwrap();
    }
}
