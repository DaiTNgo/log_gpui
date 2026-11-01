//! Logic: the floating color bar next to a selection, the Highlight Palette
//! and the color picker. One color choice can mean painting the selection
//! (on this line only: a Marker; everywhere: a Text Highlight), changing a
//! Marker or a Text Highlight, or changing the color of a Highlight Rule.

use super::AppView;
use crate::palette::HighlightColor;
use gpui::{Context, Hsla, Pixels, Point};
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
    /// Change the Text Highlight with this key; `raw` when the Raw View
    /// opened the bar.
    RecolorTextHighlight { key: String, raw: bool },
    /// Change the color of a Highlight Rule.
    Rule(u64),
}

/// Where a color chosen for a selection goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarkScope {
    /// A Marker on the selected text of this line.
    #[default]
    Line,
    /// A Text Highlight on every place of the selected text.
    Everywhere,
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
    /// Where a color for a selection goes. "This line only" each time the
    /// bar opens.
    pub scope: MarkScope,
    /// How many lines of the view hold the selected text, once counted.
    pub count: Option<usize>,
}

impl ColorBar {
    pub fn new(pos: Point<Pixels>, target: ColorTarget) -> Self {
        Self { pos, target, palette_open: false, scope: MarkScope::Line, count: None }
    }

    /// Whether the bar paints a selection (and so offers a scope).
    pub fn paints_selection(&self) -> bool {
        matches!(self.target, ColorTarget::MarkSelection | ColorTarget::RawMark { .. })
    }
}

/// The bar's text cut in the middle when it is long.
pub fn short_text(text: &str, max_chars: usize) -> String {
    let n = text.chars().count();
    if n <= max_chars {
        return text.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let head: String = text.chars().take(keep.div_ceil(2)).collect();
    let tail: String = text.chars().skip(n - keep / 2).collect();
    format!("{head}…{tail}")
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

/// A color from the color picker, as `0xRRGGBB`.
pub fn rgb_of_hsla(color: Hsla) -> u32 {
    let rgba: gpui::Rgba = color.into();
    let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (to8(rgba.r) << 16) | (to8(rgba.g) << 8) | to8(rgba.b)
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
        self.color_bar = Some(ColorBar::new(at, ColorTarget::MarkSelection));
        cx.notify();
    }

    /// Open the bar to change the color of the Marker under a byte of a row.
    pub fn open_recolor_bar(&mut self, row: usize, byte: usize, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.color_bar = Some(ColorBar::new(at, ColorTarget::RecolorMarker { row, byte }));
        cx.notify();
    }

    /// Open the bar to change the color of the Text Highlight with `key`.
    pub fn open_text_highlight_bar(&mut self, key: String, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.color_bar = Some(ColorBar::new(at, ColorTarget::RecolorTextHighlight { key, raw: false }));
        cx.notify();
    }

    /// Open the palette to choose the color of a Highlight Rule.
    pub fn open_rule_palette(&mut self, rule_id: u64, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.color_bar = Some(ColorBar { palette_open: true, ..ColorBar::new(at, ColorTarget::Rule(rule_id)) });
        cx.notify();
    }

    /// The text "Everywhere" would highlight: the selection, when it lies in
    /// one line and is not only whitespace.
    pub fn bar_everywhere_text(&self) -> Option<String> {
        let bar = self.color_bar.as_ref()?;
        let text = match &bar.target {
            ColorTarget::MarkSelection => {
                let pieces = self.selection_pieces();
                let [piece] = pieces.as_slice() else { return None };
                let entry = self.rows().get(piece.entry_ix)?;
                entry.raw_line.get(piece.range.clone())?.to_string()
            }
            ColorTarget::RawMark { pieces, .. } => {
                let [(_, range, line)] = pieces.as_slice() else { return None };
                line.get(range.clone())?.to_string()
            }
            _ => return None,
        };
        crate::text_highlight::can_highlight(&text).then_some(text)
    }

    pub fn set_bar_scope(&mut self, scope: MarkScope, cx: &mut Context<Self>) {
        if let Some(bar) = &mut self.color_bar {
            bar.scope = scope;
            cx.notify();
        }
    }

    /// Count, in the background, the lines of the view holding the text the
    /// bar would highlight everywhere. A count for another selection is
    /// cancelled; the result shows only once it is done. Called while
    /// rendering.
    pub fn sync_bar_count(&mut self, cx: &mut Context<Self>) {
        let wanted = self
            .color_bar
            .as_ref()
            .filter(|b| b.paints_selection() && b.count.is_none())
            .and_then(|b| Some((b.target.clone(), self.bar_everywhere_text()?)));
        let same = match (&self.bar_count, &wanted) {
            (Some((target, _)), Some((want, _))) => target == want,
            _ => false,
        };
        if same {
            return;
        }
        if let Some((_, cancel)) = self.bar_count.take() {
            cancel.store(true, Ordering::Release);
        }
        let Some((target, text)) = wanted else { return };
        let cancel = Arc::new(AtomicBool::new(false));
        self.bar_count = Some((target.clone(), cancel.clone()));
        let texts = Arc::new(self.view.texts(&self.parsed_files));
        let rows = texts.len();
        if rows == 0 {
            // Lines held in memory (a scratch file): counted here.
            let n = self
                .raw_sections()
                .iter()
                .filter(|s| matches!(s.lines, crate::raw_view::RawLines::Held(_)))
                .map(|s| crate::text_highlight::count_lines(&text, s.lines.all().iter().map(|l| l.as_ref())))
                .sum();
            if let Some(bar) = &mut self.color_bar {
                bar.count = Some(n);
            }
            return;
        }
        let needle = crate::unicode_text::Needle::new(&text);
        let matcher: Arc<crate::index::scan::Matcher> = Arc::new(move |first: usize, lines: &[&str]| {
            lines.iter().enumerate().filter(|(_, l)| needle.is_in(l)).map(|(i, _)| (first + i) as u32).collect()
        });
        let worker_cancel = cancel.clone();
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move {
                    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
                    crate::index::scan::scan_in_steps(&*texts, 0..rows, 0, &*matcher, workers, &worker_cancel, |_| {})
                })
                .await;
            if cancel.load(Ordering::Acquire) || !found.done || found.error.is_some() {
                return;
            }
            let _ = this.update(cx, |this, cx| {
                if let Some(bar) = this.color_bar.as_mut().filter(|b| b.target == target) {
                    bar.count = Some(found.lines.len());
                    cx.notify();
                }
            });
        })
        .detach();
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

    /// Apply a chosen color to what the bar is open for, then close it. With
    /// `everywhere` (Alt held) a selection gets a Text Highlight whatever the
    /// bar's scope.
    pub fn apply_bar_color(&mut self, color: HighlightColor, everywhere: bool, cx: &mut Context<Self>) {
        let everywhere_text = self.bar_everywhere_text();
        let Some(bar) = self.color_bar.take() else { return };
        if bar.paints_selection()
            && (everywhere || bar.scope == MarkScope::Everywhere)
            && let Some(text) = everywhere_text
        {
            self.add_text_highlight(&text, color, cx);
            self.text_selection = None;
            self.raw.list_selection = None;
            cx.notify();
            return;
        }
        match bar.target {
            ColorTarget::MarkSelection => self.mark_selection(color, cx),
            ColorTarget::RecolorMarker { row, byte } => self.recolor_marker_under(row, byte, color, cx),
            ColorTarget::RawMark { file_id, pieces } => self.mark_raw_pieces(&file_id, &pieces, color, cx),
            ColorTarget::RawRecolor { file_id, line, line_text, byte } => {
                self.recolor_raw_marker(&file_id, line, &line_text, byte, color, cx)
            }
            ColorTarget::RecolorTextHighlight { key, .. } => self.recolor_text_highlight(&key, color, cx),
            ColorTarget::Rule(id) => self.set_highlight_rule_color(id, color, cx),
        }
        cx.notify();
    }

    /// The Text Highlights of the active project.
    pub fn text_highlights(&self) -> &[crate::text_highlight::TextHighlight] {
        self.active_project().map_or(&[], |p| p.text_highlights.as_slice())
    }

    /// Highlight every place of `text` in `color`.
    pub fn add_text_highlight(&mut self, text: &str, color: HighlightColor, cx: &mut Context<Self>) {
        let Some(project) = self.active_project_mut() else { return };
        if crate::text_highlight::add_text_highlight(&mut project.text_highlights, text, color.clone()) {
            self.note_recent_color(color);
            self.after_text_highlights_changed(cx);
        }
    }

    pub fn recolor_text_highlight(&mut self, key: &str, color: HighlightColor, cx: &mut Context<Self>) {
        let Some(project) = self.active_project_mut() else { return };
        if crate::text_highlight::recolor_text_highlight(&mut project.text_highlights, key, color.clone()) {
            self.note_recent_color(color);
            self.after_text_highlights_changed(cx);
        }
    }

    /// Remove the Text Highlight with `key`: every place it painted.
    pub fn remove_text_highlight(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(project) = self.active_project_mut() else { return };
        if crate::text_highlight::remove_text_highlight(&mut project.text_highlights, key) {
            self.after_text_highlights_changed(cx);
        }
    }

    fn after_text_highlights_changed(&mut self, cx: &mut Context<Self>) {
        // The scratch editors hold their paint; the other views paint as they draw.
        self.update_raw_highlights(cx);
        self.schedule_session_save(cx);
        cx.notify();
    }
}
