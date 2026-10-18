//! Logic: the floating color bar next to a selection, the Highlight Palette
//! and the color picker. One color choice can mean painting the selection,
//! changing a Marker, or changing the color of a Highlight Rule.

use super::AppView;
use crate::palette::HighlightColor;
use gpui::{Context, Hsla, Pixels, Point};
use std::ops::Range;

/// What a chosen color is applied to.
#[derive(Debug, Clone, PartialEq)]
pub enum ColorTarget {
    /// Paint the text selected in the Parsed View.
    MarkSelection,
    /// Change the Marker under a byte of a row of the Parsed View.
    RecolorMarker { row: usize, byte: usize },
    /// Paint text selected in the Raw View: `(line, bytes of the line, line text)`.
    RawMark { file_id: String, pieces: Vec<(usize, Range<usize>, String)> },
    /// Change the Marker under a byte of a line in the Raw View.
    RawRecolor { file_id: String, line: usize, line_text: String, byte: usize },
    /// Change the color of a Highlight Rule.
    Rule(u64),
}

/// The floating bar of recent colors, with the Highlight Palette and the
/// color picker one click away.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorBar {
    /// Window position of the bar's top-left corner.
    pub pos: Point<Pixels>,
    pub target: ColorTarget,
    /// The 30-cell palette is open under the bar.
    pub palette_open: bool,
}

/// Colors a bar offers before the user has used any: the medium shade of the
/// first hues.
pub fn default_bar_colors() -> Vec<HighlightColor> {
    use crate::palette::{Hue, Shade};
    [Hue::Amber, Hue::Orange, Hue::Red, Hue::Green, Hue::Blue, Hue::Purple]
        .into_iter()
        .map(|h| HighlightColor::palette(h, Shade::Medium))
        .collect()
}

/// A color from the color picker, as a free `#RRGGBB`.
pub fn color_from_hsla(color: Hsla) -> HighlightColor {
    let rgba: gpui::Rgba = color.into();
    let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    let rgb = (to8(rgba.r) << 16) | (to8(rgba.g) << 8) | to8(rgba.b);
    HighlightColor::Custom(crate::palette::format_hex(rgb))
}

impl AppView {
    /// The colors the bar shows: the recent ones, then defaults to fill it.
    pub fn bar_colors(&self) -> Vec<HighlightColor> {
        let mut colors: Vec<HighlightColor> = self
            .active_project()
            .map(|p| p.recent_colors.colors().to_vec())
            .unwrap_or_default();
        for c in default_bar_colors() {
            if colors.len() >= 6 {
                break;
            }
            if !colors.contains(&c) {
                colors.push(c);
            }
        }
        colors
    }

    /// Open the bar next to the text selected in the Parsed View.
    pub fn open_mark_bar(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.color_bar = Some(ColorBar { pos: at, target: ColorTarget::MarkSelection, palette_open: false });
        cx.notify();
    }

    /// Open the bar to change the color of the Marker under a byte of a row.
    pub fn open_recolor_bar(&mut self, row: usize, byte: usize, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.color_bar = Some(ColorBar { pos: at, target: ColorTarget::RecolorMarker { row, byte }, palette_open: false });
        cx.notify();
    }

    /// Open the palette to choose the color of a Highlight Rule.
    pub fn open_rule_palette(&mut self, rule_id: u64, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.color_bar = Some(ColorBar { pos: at, target: ColorTarget::Rule(rule_id), palette_open: true });
        cx.notify();
    }

    pub fn close_color_bar(&mut self, cx: &mut Context<Self>) {
        if self.color_bar.take().is_some() {
            cx.notify();
        }
    }

    pub fn toggle_palette_open(&mut self, cx: &mut Context<Self>) {
        if let Some(bar) = &mut self.color_bar {
            bar.palette_open = !bar.palette_open;
            cx.notify();
        }
    }

    /// Apply a chosen color to what the bar is open for, then close it.
    pub fn apply_bar_color(&mut self, color: HighlightColor, cx: &mut Context<Self>) {
        let Some(bar) = self.color_bar.take() else { return };
        match bar.target {
            ColorTarget::MarkSelection => self.mark_selection(color, cx),
            ColorTarget::RecolorMarker { row, byte } => self.recolor_marker_under(row, byte, color, cx),
            ColorTarget::RawMark { file_id, pieces } => self.mark_raw_pieces(&file_id, &pieces, color, cx),
            ColorTarget::RawRecolor { file_id, line, line_text, byte } => {
                self.recolor_raw_marker(&file_id, line, &line_text, byte, color, cx)
            }
            ColorTarget::Rule(id) => self.set_highlight_rule_color(id, color, cx),
        }
        cx.notify();
    }
}
