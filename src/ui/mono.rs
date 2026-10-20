//! Text in the theme's monospace font: where a pointer lands in a line, and
//! how wide a column of characters is. Also the width of each Column Header
//! label, which is drawn in the UI font and sets the narrowest a column fits.
//!
//! The font differs by platform (Menlo on macOS, Consolas on Windows, DejaVu
//! Sans Mono on Linux), and a line may hold glyphs the font lacks (CJK, emoji)
//! that come from a fallback font. So nothing here assumes a width: hit-testing
//! shapes the line as it is drawn, and column fitting uses the advance measured
//! from the font.

use std::sync::atomic::{AtomicU32, Ordering};

use gpui::{App, FontWeight, Hsla, TextRun, Window, px};
use gpui_component::ActiveTheme as _;

use crate::column::PinnableColumn;

/// Used until measured (and in tests): the advance of Menlo / SF Mono.
const DEFAULT_RATIO: f32 = 0.6;

/// Advance of one character as a fraction of the font size, as `f32` bits.
static CHAR_RATIO: AtomicU32 = AtomicU32::new(DEFAULT_RATIO.to_bits());

/// Width in pixels of one character at `font_size`, for fitting a column to a
/// number of characters.
pub fn char_width(font_size: f32) -> f32 {
    f32::from_bits(CHAR_RATIO.load(Ordering::Relaxed)) * font_size
}

/// Font size of the Column Header labels (`text_xs`: 0.75rem at 16px).
pub const HEADER_FONT_SIZE: f32 = 12.0;

/// Used until measured (and in tests): uppercase semibold UI text runs wider
/// than a monospace character.
const DEFAULT_HEADER_RATIO: f32 = 0.72;

/// Width of each Column Header label as a fraction of the font size, as
/// `f32` bits, indexed like `PinnableColumn::ALL`. Zero until measured.
static HEADER_RATIOS: [AtomicU32; PinnableColumn::ALL.len()] =
    [const { AtomicU32::new(0) }; PinnableColumn::ALL.len()];

/// Width in pixels of the label of `col`'s Column Header.
pub fn header_label_width(col: PinnableColumn) -> f32 {
    let measured = col
        .fixed_index()
        .map_or(0.0, |i| f32::from_bits(HEADER_RATIOS[i].load(Ordering::Relaxed)));
    let ratio = if measured > 0.0 { measured } else { col.title_chars() as f32 * DEFAULT_HEADER_RATIO };
    ratio * HEADER_FONT_SIZE
}

/// Width in pixels of any Column Header text (a Column Label, a field name),
/// estimated from its length.
pub fn header_text_width(text: &str) -> f32 {
    text.chars().count() as f32 * DEFAULT_HEADER_RATIO * HEADER_FONT_SIZE
}

/// Measure the theme's monospace font and the Column Header labels. Call
/// after the theme is set.
pub fn measure(cx: &App) {
    let text_system = cx.text_system();
    let header_font = gpui::Font { weight: FontWeight::SEMIBOLD, ..gpui::font(cx.theme().font_family.clone()) };
    let header_id = text_system.resolve_font(&header_font);
    for col in PinnableColumn::ALL {
        let width: Option<f32> = col
            .label()
            .chars()
            .map(|ch| text_system.advance(header_id, px(100.0), ch).ok().map(|s| f32::from(s.width)))
            .sum();
        if let Some(ratio) = width.map(|w| w / 100.0).filter(|r| r.is_finite() && *r > 0.0) {
            if let Some(i) = col.fixed_index() {
                HEADER_RATIOS[i].store(ratio.to_bits(), Ordering::Relaxed);
            }
        }
    }
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
