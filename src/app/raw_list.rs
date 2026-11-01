//! Logic: the Raw View's list of read-only File Sections (openspec change
//! `line-offset-index`, task 8.1). Every section's lines are rows of one
//! virtualized list, read from the file's Line Index as they are drawn;
//! nothing holds the file's whole text. A section has a header (when there
//! are several), the row of what a Remote File holds, and its Breaks between
//! its lines. Text is selected with the mouse within one section, to copy it
//! or paint it with a Marker.

use super::AppView;
use super::raw::RawSection;
use crate::app::color::{ColorBar, ColorTarget};
use gpui::{Bounds, Context, Pixels, Point, ScrollStrategy, Window};
use std::ops::Range;

/// A row of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawRow {
    Header { section: usize },
    /// What a Remote File holds, and what can be done about it.
    Actions { section: usize },
    /// The Break drawn above line `line` of the section.
    Break { section: usize, line: usize },
    Line { section: usize, line: usize },
}

/// How a section lays out in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SectionRows {
    start: usize,
    header: bool,
    actions: bool,
    collapsed: bool,
    lines: usize,
    /// Lines with a Break above them, ascending.
    breaks: Vec<usize>,
}

impl SectionRows {
    fn len(&self) -> usize {
        let body = if self.collapsed { 0 } else { usize::from(self.actions) + self.lines + self.breaks.len() };
        usize::from(self.header) + body
    }
}

/// The rows of the list: found by arithmetic, with no list of rows kept.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawRows {
    sections: Vec<SectionRows>,
    total: usize,
}

impl RawRows {
    /// `sections`: each one's line count, whether it is collapsed, whether
    /// it has an actions row, and the lines with a Break above them.
    pub fn new(sections: impl IntoIterator<Item = (usize, bool, bool, Vec<usize>)>) -> Self {
        let sections: Vec<_> = sections.into_iter().collect();
        let header = sections.len() > 1;
        let mut start = 0;
        let mut out = Vec::with_capacity(sections.len());
        for (lines, collapsed, actions, breaks) in sections {
            let rows = SectionRows { start, header, actions, collapsed: collapsed && header, lines, breaks };
            start += rows.len();
            out.push(rows);
        }
        Self { sections: out, total: start }
    }

    pub fn len(&self) -> usize {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// The row at `index`.
    pub fn row(&self, index: usize) -> Option<RawRow> {
        let section = self.sections.partition_point(|s| s.start <= index).checked_sub(1)?;
        let s = &self.sections[section];
        let mut at = index - s.start;
        if at >= s.len() {
            return None;
        }
        if s.header {
            if at == 0 {
                return Some(RawRow::Header { section });
            }
            at -= 1;
        }
        if s.actions {
            if at == 0 {
                return Some(RawRow::Actions { section });
            }
            at -= 1;
        }
        // The Break above line `b[i]` is row `b[i] + i` of the lines.
        let before = s.breaks.iter().enumerate().take_while(|(i, b)| **b + i <= at).count();
        if before > 0 && s.breaks[before - 1] + before - 1 == at {
            return Some(RawRow::Break { section, line: s.breaks[before - 1] });
        }
        Some(RawRow::Line { section, line: at - before })
    }

    /// The row of line `line` of `section`; `None` while it is collapsed.
    pub fn index_of_line(&self, section: usize, line: usize) -> Option<usize> {
        let s = self.sections.get(section)?;
        if s.collapsed || line >= s.lines {
            return None;
        }
        let breaks = s.breaks.partition_point(|&b| b <= line);
        Some(s.start + usize::from(s.header) + usize::from(s.actions) + breaks + line)
    }
}

/// Where in the Raw View's text: a byte of a line of a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RawPos {
    pub section: usize,
    pub line: usize,
    pub byte: usize,
}

/// Text selected with the mouse, within one section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSelection {
    pub anchor: RawPos,
    pub head: RawPos,
}

impl RawSelection {
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub fn ordered(&self) -> (RawPos, RawPos) {
        if self.anchor <= self.head { (self.anchor, self.head) } else { (self.head, self.anchor) }
    }

    /// The bytes of line `line` of `section` it covers, `len` the line's
    /// length.
    pub fn on_line(&self, section: usize, line: usize, len: usize) -> Option<Range<usize>> {
        let (start, end) = self.ordered();
        if section != start.section || line < start.line || line > end.line || self.is_empty() {
            return None;
        }
        let from = if line == start.line { start.byte.min(len) } else { 0 };
        let to = if line == end.line { end.byte.min(len) } else { len };
        (from < to).then_some(from..to)
    }
}

/// Width of the gutter of line numbers for lines up to `lines`.
pub fn gutter_width(lines: usize, font_size: f32) -> f32 {
    let digits = lines.max(1).to_string().len().max(3);
    digits as f32 * crate::ui::mono::char_width(font_size) + 16.0
}

/// Room left of a line's text: the gutter, then a little space.
pub const TEXT_PAD: f32 = 8.0;

impl AppView {
    /// The sections and the rows of the Raw View's list.
    pub fn raw_rows(&self) -> (Vec<RawSection>, RawRows) {
        let sections = self.raw_sections();
        let rows = RawRows::new(sections.iter().map(|s| {
            (
                s.lines.len(),
                self.is_section_collapsed(&s.file_id),
                self.hold_info(&s.file_id).is_some(),
                s.breaks.keys().copied().filter(|&b| b < s.lines.len()).collect(),
            )
        }));
        (sections, rows)
    }

    /// Whether the Raw View is the list: no section is edited here.
    pub fn raw_is_list(&self) -> bool {
        !self.raw_sections().iter().any(|s| s.editable)
    }

    /// Where a pointer at window position `at` is in the list's text.
    pub fn raw_pos_at(&self, at: Point<Pixels>, window: &Window, cx: &gpui::App) -> Option<RawPos> {
        let (sections, rows) = self.raw_rows();
        if rows.is_empty() {
            return None;
        }
        let bounds: Bounds<Pixels> = self.raw.list_bounds.get();
        let scroll = self.raw.scroll.0.borrow().base_handle.offset();
        let row_h = self.text_scale().row_height();
        let y = f32::from(at.y - bounds.origin.y - scroll.y);
        let index = ((y / row_h).floor().max(0.0) as usize).min(rows.len() - 1);
        // Above the first line or on a header: the nearest line below.
        let (section, line) = (index..rows.len()).chain((0..index).rev()).find_map(|i| match rows.row(i)? {
            RawRow::Line { section, line } => Some((section, line)),
            _ => None,
        })?;
        let text = sections.get(section)?.lines.get(line).unwrap_or_default();
        let font_size = self.text_scale().font_size();
        let max = sections.iter().map(|s| s.lines.len()).max().unwrap_or(0);
        let x = f32::from(at.x - bounds.origin.x - scroll.x) - gutter_width(max, font_size) - TEXT_PAD;
        let byte = crate::ui::mono::index_for_x(&text, x, font_size, window, cx);
        Some(RawPos { section, line, byte })
    }

    pub fn start_raw_selection(&mut self, at: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        self.color_bar = None;
        self.raw.list_selection = self.raw_pos_at(at, window, cx).map(|p| RawSelection { anchor: p, head: p });
        self.raw.list_selecting = self.raw.list_selection.is_some();
        cx.notify();
    }

    pub fn drag_raw_selection(&mut self, at: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        if !self.raw.list_selecting {
            return;
        }
        let Some(pos) = self.raw_pos_at(at, window, cx) else { return };
        let Some(anchor) = self.raw.list_selection.map(|s| s.anchor) else { return };
        // Within the section it started in.
        let head = if pos.section == anchor.section {
            pos
        } else if pos.section > anchor.section {
            let lines = self.raw_sections().get(anchor.section).map_or(0, |s| s.lines.len());
            RawPos { section: anchor.section, line: lines.saturating_sub(1), byte: usize::MAX }
        } else {
            RawPos { section: anchor.section, line: 0, byte: 0 }
        };
        self.raw.list_selection = Some(RawSelection { anchor, head });
        cx.notify();
    }

    /// The button was released: a drag leaves the selection with the bar
    /// to paint it; a click makes the line the current one, and on a
    /// Marker or a Text Highlight opens the bar to change it.
    pub fn finish_raw_selection(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.raw.list_selecting) {
            return;
        }
        let Some(sel) = self.raw.list_selection else { return };
        let sections = self.raw_sections();
        let Some(section) = sections.get(sel.anchor.section) else { return };
        let file_id = section.file_id.clone();
        if !sel.is_empty() {
            let (start, end) = sel.ordered();
            let pieces: Vec<(usize, Range<usize>, String)> = (start.line..=end.line)
                .filter_map(|line| {
                    let text = section.lines.get(line)?.to_string();
                    let range = sel.on_line(sel.anchor.section, line, text.len())?;
                    Some((line + 1, range, text))
                })
                .collect();
            if !pieces.is_empty() {
                self.color_bar = Some(ColorBar::new(at, ColorTarget::RawMark { file_id, pieces }));
            }
            cx.notify();
            return;
        }
        self.raw.list_selection = None;
        let (line, byte) = (sel.anchor.line + 1, sel.anchor.byte);
        self.raw.selected = Some((file_id.clone(), sel.anchor.line));
        let current = crate::focus::LineRef::new(file_id.as_str(), line);
        if self.current_line().as_ref() != Some(&current) {
            self.set_current_line(current, super::focus::Origin::View, cx);
        }
        let on_marker = self
            .active_project()
            .and_then(|p| p.log_spaces.iter().find_map(|s| s.file(&file_id)))
            .is_some_and(|f| f.markers_on_line(line).iter().any(|m| m.range.start <= byte && byte < m.range.end));
        let line_text = section.lines.get(line - 1).map(|t| t.to_string()).unwrap_or_default();
        if on_marker {
            let target = ColorTarget::RawRecolor { file_id, line, line_text, byte };
            self.color_bar = Some(ColorBar::new(at, target));
        } else if let Some(key) = crate::text_highlight::HighlightMatcher::new(self.text_highlights()).key_at(&line_text, byte) {
            self.color_bar = Some(ColorBar::new(at, ColorTarget::RecolorTextHighlight { key: key.to_string(), raw: true }));
        }
        cx.notify();
    }

    /// The text selected in the list, lines joined by `\n`.
    pub fn raw_selected_text(&self) -> Option<String> {
        let sel = self.raw.list_selection.filter(|s| !s.is_empty())?;
        let sections = self.raw_sections();
        let section = sections.get(sel.anchor.section)?;
        let (start, end) = sel.ordered();
        let parts: Vec<String> = (start.line..=end.line)
            .map(|line| {
                let text = section.lines.get(line).unwrap_or_default();
                let range = sel.on_line(sel.anchor.section, line, text.len()).unwrap_or(0..0);
                text[range].to_string()
            })
            .collect();
        Some(parts.join("\n"))
    }

    /// Bring line `line` (0-based) of section `section` into the middle of
    /// the list.
    pub(crate) fn reveal_raw_list_line(&mut self, section: usize, line: usize) {
        let (_, rows) = self.raw_rows();
        if let Some(index) = rows.index_of_line(section, line) {
            self.raw.scroll.scroll_to_item(index, ScrollStrategy::Center);
        }
    }

    /// Flash line `line` (0-based) of the file, as a jump does: a fill
    /// that fades out over `ROW_FLASH`.
    pub(crate) fn flash_raw_list_line(&mut self, file_id: &str, line: usize, cx: &mut Context<Self>) {
        let started = std::time::Instant::now();
        self.raw.list_flash = Some((file_id.to_string(), line, started));
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
            let going = this
                .update(cx, |this, cx| {
                    if this.raw.list_flash.as_ref().is_none_or(|(_, _, at)| *at != started) {
                        return false;
                    }
                    if started.elapsed() >= crate::ui::canvas::ROW_FLASH {
                        this.raw.list_flash = None;
                    }
                    cx.notify();
                    this.raw.list_flash.is_some()
                })
                .unwrap_or(false);
            if !going {
                break;
            }
        })
        .detach();
    }

    /// Follow: the list at its last row.
    pub(crate) fn raw_list_to_end(&mut self) {
        let (_, rows) = self.raw_rows();
        if !rows.is_empty() {
            self.raw.scroll.scroll_to_item(rows.len() - 1, ScrollStrategy::Bottom);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_found_with_headers_actions_and_breaks() {
        // Two sections: 3 lines with a Break above line 1, and 2 lines with
        // an actions row.
        let rows = RawRows::new([(3, false, false, vec![1]), (2, false, true, vec![])]);
        let all: Vec<RawRow> = (0..rows.len()).filter_map(|i| rows.row(i)).collect();
        use RawRow::*;
        assert_eq!(
            all,
            [
                Header { section: 0 },
                Line { section: 0, line: 0 },
                Break { section: 0, line: 1 },
                Line { section: 0, line: 1 },
                Line { section: 0, line: 2 },
                Header { section: 1 },
                Actions { section: 1 },
                Line { section: 1, line: 0 },
                Line { section: 1, line: 1 },
            ]
        );
        for (i, row) in all.iter().enumerate() {
            if let Line { section, line } = row {
                assert_eq!(rows.index_of_line(*section, *line), Some(i));
            }
        }
    }

    #[test]
    fn one_section_has_no_header_and_does_not_collapse() {
        let rows = RawRows::new([(2, true, false, vec![0, 1])]);
        let all: Vec<RawRow> = (0..rows.len()).filter_map(|i| rows.row(i)).collect();
        use RawRow::*;
        assert_eq!(
            all,
            [Break { section: 0, line: 0 }, Line { section: 0, line: 0 }, Break { section: 0, line: 1 }, Line { section: 0, line: 1 }]
        );
    }

    #[test]
    fn a_collapsed_section_is_its_header() {
        let rows = RawRows::new([(5, true, true, vec![]), (1, false, false, vec![])]);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows.row(1), Some(RawRow::Header { section: 1 }));
        assert_eq!(rows.index_of_line(0, 0), None);
    }

    #[test]
    fn a_selection_covers_whole_lines_between_its_ends() {
        let sel = RawSelection {
            anchor: RawPos { section: 0, line: 3, byte: 2 },
            head: RawPos { section: 0, line: 1, byte: 4 },
        };
        assert_eq!(sel.on_line(0, 1, 10), Some(4..10));
        assert_eq!(sel.on_line(0, 2, 10), Some(0..10));
        assert_eq!(sel.on_line(0, 3, 10), Some(0..2));
        assert_eq!(sel.on_line(0, 4, 10), None);
        assert_eq!(sel.on_line(1, 2, 10), None);
    }
}
