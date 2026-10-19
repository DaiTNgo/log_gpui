//! Text in the theme's monospace font: where a pointer lands in a line, and
//! how wide a column of characters is.
//!
//! The font differs by platform (Menlo on macOS, Consolas on Windows, DejaVu
//! Sans Mono on Linux), and a line may hold glyphs the font lacks (CJK, emoji)
//! that come from a fallback font. So nothing here assumes a width: hit-testing
//! shapes the line as it is drawn, and column fitting uses the advance measured
//! from the font.

use std::sync::atomic::{AtomicU32, Ordering};

use gpui::{App, Hsla, TextRun, Window, px};
use gpui_component::ActiveTheme as _;

/// Used until measured (and in tests): the advance of Menlo / SF Mono.
const DEFAULT_RATIO: f32 = 0.6;

/// Advance of one character as a fraction of the font size, as `f32` bits.
static CHAR_RATIO: AtomicU32 = AtomicU32::new(DEFAULT_RATIO.to_bits());

/// Width in pixels of one character at `font_size`, for fitting a column to a
/// number of characters.
pub fn char_width(font_size: f32) -> f32 {
    f32::from_bits(CHAR_RATIO.load(Ordering::Relaxed)) * font_size
}

/// Measure the theme's monospace font. Call after the theme is set.
pub fn measure(cx: &App) {
    let text_system = cx.text_system();
    let font_id = text_system.resolve_font(&gpui::font(cx.theme().mono_font_family.clone()));
    // Measured at a large size so the ratio keeps its precision.
    let ratio = text_system
        .ch_advance(font_id, px(100.0))
        .map(|w| f32::from(w) / 100.0)
        .ok()
        .filter(|r| r.is_finite() && *r > 0.0)
        .unwrap_or(DEFAULT_RATIO);
    CHAR_RATIO.store(ratio.to_bits(), Ordering::Relaxed);
}

/// The byte offset in `text` of the character boundary nearest `x` pixels
/// from the start of the text, with `text` shaped in the theme's monospace
/// font at `font_size`, exactly as the rows draw it.
pub fn index_for_x(text: &str, x: f32, font_size: f32, window: &Window, cx: &App) -> usize {
    if x <= 0.0 || text.is_empty() {
        return 0;
    }
    let run = TextRun {
        len: text.len(),
        font: gpui::font(cx.theme().mono_font_family.clone()),
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let layout = window.text_system().layout_line(text, px(font_size), &[run], None);
    // Every glyph starts a boundary and the line's end closes the last one;
    // the pointer belongs to whichever boundary is nearest.
    layout
        .runs
        .iter()
        .flat_map(|run| run.glyphs.iter().map(|g| (g.index, f32::from(g.position.x))))
        .chain([(text.len(), f32::from(layout.width))])
        .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
        .map_or(0, |(index, _)| index)
}
