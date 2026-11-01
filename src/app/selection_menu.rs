//! Logic: the Selection Menu, the one right-click menu on text in the Parsed
//! View, the Raw View list and a scratch editor. A right-click records what
//! it landed on (`SelectionTarget`); the menu's items act on that.

use super::AppView;
use super::color::{ColorBar, ColorTarget};
use crate::palette::HighlightColor;
use crate::ui::cells::TextCell;
use crate::workspace::state::ContextSection;
use gpui::{Context, Pixels, Point, Window};

/// Where the menu was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSource {
    Parsed,
    RawList,
    /// A scratch file's editor.
    Editor,
}

/// The value of a cell of the Parsed View the menu can filter or make a rule on.
#[derive(Debug, Clone, PartialEq)]
pub struct CellValue {
    /// Column key.
    pub column: String,
    /// The selected text, or the whole value of the cell.
    pub value: String,
    /// The value is the cell's whole value (a Filter Value), not a piece of
    /// it (a Custom Value).
    pub whole: bool,
}

/// What a right-click landed on.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionTarget {
    pub source: MenuSource,
    /// Where the pointer was, for a color bar opened from the menu.
    pub at: Point<Pixels>,
    /// The selected text, for Copy.
    pub text: Option<String>,
    /// Paints the selection: `MarkSelection` or `RawMark`.
    pub mark: Option<ColorTarget>,
    /// The text "Highlight everywhere" would highlight: the selection, in one
    /// line and not only whitespace.
    pub everywhere: Option<String>,
    /// The Marker under the pointer: `RecolorMarker` or `RawRecolor`.
    pub marker: Option<ColorTarget>,
    /// Key of the Text Highlight under the pointer.
    pub highlight: Option<String>,
    /// The cell to filter on or make a rule from (Parsed View only).
    pub cell: Option<CellValue>,
    /// "Highlight rows where …" is offered (not on TIMESTAMP).
    pub rule: bool,
}

impl SelectionTarget {
    fn new(source: MenuSource, at: Point<Pixels>) -> Self {
        Self {
            source,
            at,
            text: None,
            mark: None,
            everywhere: None,
            marker: None,
            highlight: None,
            cell: None,
            rule: false,
        }
    }
}

/// The cell value the menu offers for a cell holding `cell_text` at
/// `cell_range` of the line, with `selected` the bytes selected on that line
/// (if any). `None` when the selection leaves the cell, or the cell is
/// TIMESTAMP or the whole line.
pub fn cell_value(
    cell: TextCell,
    line: &str,
    cell_range: std::ops::Range<usize>,
    selected: Option<std::ops::Range<usize>>,
) -> Option<CellValue> {
    if matches!(cell, TextCell::Timestamp | TextCell::Raw) {
        return None;
    }
    let column = cell.column_key().to_string();
    let whole_text = line.get(cell_range.clone())?;
    match selected {
        Some(sel) => {
            if sel.start < cell_range.start || sel.end > cell_range.end {
                return None;
            }
            let text = line.get(sel)?;
            let value = text.trim();
            if value.is_empty() {
                return None;
            }
            let whole = value == whole_text.trim() && cell != TextCell::Message;
            Some(CellValue { column, value: value.to_string(), whole })
        }
        // MESSAGE is filtered by a piece of it, never by the whole message.
        None if cell == TextCell::Message => None,
        None => {
            let value = whole_text.trim();
            (!value.is_empty()).then(|| CellValue { column, value: value.to_string(), whole: true })
        }
    }
}

impl AppView {
    /// A right-click in the Parsed View: record what it landed on. A click
    /// outside the selection drops the selection first.
    pub fn on_parsed_right_click(&mut self, at: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        self.color_bar = None;
        let target = self.parsed_menu_target(at, window, cx);
        self.selection_menu = target;
        cx.notify();
    }

    fn parsed_menu_target(&mut self, at: Point<Pixels>, window: &Window, cx: &gpui::App) -> Option<SelectionTarget> {
        use crate::ui::cells::{Hit, cell_text};
        let (pos, hit) = self.text_pos_at(at, window, cx)?;
        if let Some(sel) = self.text_selection {
            let inside = sel.range_in_row(pos.row, usize::MAX).is_some_and(|r| r.contains(&pos.byte))
                || sel.covers_row(pos.row) && sel.ordered().0.row != sel.ordered().1.row;
            if !inside {
                self.text_selection = None;
            }
        }
        let mut target = SelectionTarget::new(MenuSource::Parsed, at);
        let pieces = self.selection_pieces();
        let has_selection = self.text_selection.is_some_and(|s| !s.is_empty()) && !pieces.is_empty();
        if has_selection {
            target.text = self.selected_text();
            target.mark = Some(ColorTarget::MarkSelection);
            if let [piece] = pieces.as_slice() {
                target.everywhere = self
                    .rows()
                    .get(piece.entry_ix)
                    .and_then(|e| e.raw_line.get(piece.range.clone()).map(str::to_string))
                    .filter(|t| crate::text_highlight::can_highlight(t));
            }
        }
        if self.marker_under(pos.row, pos.byte).is_some() {
            target.marker = Some(ColorTarget::RecolorMarker { row: pos.row, byte: pos.byte });
        }
        target.highlight = self.text_highlight_under(pos.row, pos.byte);

        let entry_ix = *self.visible_indices.get(pos.row)?;
        let entry = self.rows().get(entry_ix)?;
        let selected = match pieces.as_slice() {
            [piece] if has_selection && piece.entry_ix == entry_ix => Some(Some(piece.range.clone())),
            _ if has_selection => None,
            _ => Some(None),
        };
        match hit {
            Hit::Text { cell, .. } => {
                target.rule = cell != TextCell::Timestamp && cell != TextCell::Raw;
                if let (Some(selected), Some((_, Some(range)))) = (selected, cell_text(&entry, cell)) {
                    target.cell = cell_value(cell, &entry.raw_line, range, selected);
                }
            }
            Hit::Source if !has_selection => {
                target.cell = self
                    .rows()
                    .source(entry_ix)
                    .filter(|s| !s.is_empty())
                    .map(|s| CellValue { column: "source".to_string(), value: s.to_string(), whole: true });
                target.rule = target.cell.is_some();
            }
            _ => {}
        }
        if target.cell.is_none() {
            target.rule = false;
        }
        Some(target)
    }

    /// A right-click in the Raw View's list.
    pub fn on_raw_list_right_click(&mut self, at: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        self.color_bar = None;
        self.selection_menu = self.raw_list_menu_target(at, window, cx);
        cx.notify();
    }

    fn raw_list_menu_target(&mut self, at: Point<Pixels>, window: &Window, cx: &gpui::App) -> Option<SelectionTarget> {
        let pos = self.raw_pos_at(at, window, cx)?;
        if let Some(sel) = self.raw.list_selection {
            let (start, end) = sel.ordered();
            let inside = pos.section == sel.anchor.section
                && (start.line, start.byte) <= (pos.line, pos.byte)
                && (pos.line, pos.byte) <= (end.line, end.byte);
            if !inside {
                self.raw.list_selection = None;
            }
        }
        let mut target = SelectionTarget::new(MenuSource::RawList, at);
        target.text = self.raw_selected_text();
        target.mark = self.raw_list_mark_target();
        target.everywhere = everywhere_of(target.mark.as_ref());
        let sections = self.raw_sections();
        let section = sections.get(pos.section)?;
        let line_text = section.lines.get(pos.line).map(|t| t.to_string()).unwrap_or_default();
        self.under_raw(&section.file_id, pos.line + 1, &line_text, pos.byte, &mut target);
        Some(target)
    }

    /// A right-click in a scratch file's editor: its selection, or the
    /// Marker at the caret.
    pub fn on_editor_right_click(&mut self, file_id: &str, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.color_bar = None;
        let mut target = SelectionTarget::new(MenuSource::Editor, at);
        let sections = self.raw_sections();
        if let (Some(section), Some(editor)) =
            (sections.iter().find(|s| s.file_id == file_id), self.raw.editors.get(file_id))
        {
            let (range, cursor, text) = {
                let state = editor.state.read(cx);
                (state.selected_range(), state.cursor(), state.text().clone())
            };
            if range.start != range.end {
                target.text = Some(text.slice(range.clone()).to_string());
                let all = section.lines.all();
                let pieces: Vec<_> = crate::raw_view::pieces_from_range(&all, range)
                    .into_iter()
                    .map(|(line, r)| (line, r, all[line - 1].to_string()))
                    .collect();
                if !pieces.is_empty() {
                    target.mark = Some(ColorTarget::RawMark { file_id: file_id.to_string(), pieces });
                }
                target.everywhere = everywhere_of(target.mark.as_ref());
            } else if let Some((line, byte)) =
                crate::raw_view::line_and_byte_in_rope(&text, cursor, section.lines.len())
            {
                let line_text = section.lines.get(line - 1).map(|t| t.to_string()).unwrap_or_default();
                self.under_raw(file_id, line, &line_text, byte, &mut target);
            }
        }
        self.selection_menu = Some(target);
        cx.notify();
    }

    /// The Marker and Text Highlight under a byte of a line (1-based) of a file.
    fn under_raw(&self, file_id: &str, line: usize, line_text: &str, byte: usize, target: &mut SelectionTarget) {
        let on_marker = self
            .active_project()
            .and_then(|p| p.log_spaces.iter().find_map(|s| s.file(file_id)))
            .is_some_and(|f| f.markers_on_line(line).iter().any(|m| m.range.start <= byte && byte < m.range.end));
        if on_marker {
            target.marker = Some(ColorTarget::RawRecolor {
                file_id: file_id.to_string(),
                line,
                line_text: line_text.to_string(),
                byte,
            });
        }
        target.highlight = crate::text_highlight::HighlightMatcher::new(self.text_highlights())
            .key_at(line_text, byte)
            .map(str::to_string);
    }

    fn menu_target(&self) -> Option<SelectionTarget> {
        self.selection_menu.clone()
    }

    pub fn menu_copy(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.menu_target().and_then(|t| t.text) {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
    }

    /// "Mark here": a Marker on the selected text of each line.
    pub fn menu_mark_here(&mut self, color: HighlightColor, cx: &mut Context<Self>) {
        let Some(target) = self.menu_target() else { return };
        let Some(mark) = target.mark else { return };
        self.color_bar = Some(ColorBar::new(target.at, mark));
        self.apply_bar_color(color, false, cx);
    }

    /// "Highlight everywhere": a Text Highlight on the selected text.
    pub fn menu_highlight_everywhere(&mut self, color: HighlightColor, cx: &mut Context<Self>) {
        let Some(text) = self.menu_target().and_then(|t| t.everywhere) else { return };
        self.add_text_highlight(&text, color, cx);
        self.text_selection = None;
        self.raw.list_selection = None;
        cx.notify();
    }

    /// "Color…": the color bar for the selection.
    pub fn menu_open_color_bar(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.menu_target() else { return };
        if let Some(mark) = target.mark {
            self.color_bar = Some(ColorBar::new(target.at, mark));
            cx.notify();
        }
    }

    /// "Change color…" of the Marker under the pointer.
    pub fn menu_recolor_marker(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.menu_target() else { return };
        if let Some(marker) = target.marker {
            self.color_bar = Some(ColorBar::new(target.at, marker));
            cx.notify();
        }
    }

    pub fn menu_remove_marker(&mut self, cx: &mut Context<Self>) {
        match self.menu_target().and_then(|t| t.marker) {
            Some(ColorTarget::RecolorMarker { row, byte }) => self.remove_marker_under(row, byte, cx),
            Some(ColorTarget::RawRecolor { file_id, line, line_text, byte }) => {
                self.remove_raw_marker(&file_id, line, &line_text, byte, cx)
            }
            _ => {}
        }
    }

    /// "Change highlight color…" of the Text Highlight under the pointer.
    pub fn menu_recolor_highlight(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.menu_target() else { return };
        if let Some(key) = target.highlight {
            let raw = target.source != MenuSource::Parsed;
            self.color_bar = Some(ColorBar::new(target.at, ColorTarget::RecolorTextHighlight { key, raw }));
            cx.notify();
        }
    }

    pub fn menu_remove_highlight(&mut self, cx: &mut Context<Self>) {
        if let Some(key) = self.menu_target().and_then(|t| t.highlight) {
            self.remove_text_highlight(&key, cx);
        }
    }

    /// "Show only" / "Hide" from the menu.
    pub fn menu_filter(&mut self, hide: bool, cx: &mut Context<Self>) {
        if let Some(cell) = self.menu_target().and_then(|t| t.cell) {
            self.filter_from_selection(&cell.column, &cell.value, cell.whole, hide, cx);
        }
    }

    /// Whether a value of `column` from a selection becomes a Custom Value:
    /// a piece of a cell, MESSAGE, or a column with too many values to tick.
    pub fn filters_as_custom(&self, column: &str, whole: bool) -> bool {
        !whole
            || column == crate::columns::MESSAGE_KEY
            || !self.filter_index.column(column).is_some_and(|c| c.is_filterable())
    }

    /// Show only (or hide) the rows whose `column` is `value`, from a
    /// selection or a field of the Inspector.
    pub fn filter_from_selection(&mut self, column: &str, value: &str, whole: bool, hide: bool, cx: &mut Context<Self>) {
        let custom = self.filters_as_custom(column, whole);
        self.filter_state.apply_from_selection(column, value, custom, hide);
        self.apply_filter_change(cx);
    }

    /// "Highlight rows where …": the New rule form, filled in.
    pub fn menu_highlight_rows(&mut self, cx: &mut Context<Self>) {
        if let Some(cell) = self.menu_target().and_then(|t| t.cell) {
            self.open_rule_form_with(&cell.column, &cell.value, cx);
        }
    }

    /// Open the New rule form for `column` with `value` ticked (a Custom
    /// Value on MESSAGE), and show the Highlights section.
    pub fn open_rule_form_with(&mut self, column: &str, value: &str, cx: &mut Context<Self>) {
        let color = crate::palette::next_rule_color(self.highlight_rules().len());
        let custom = column == crate::highlight::MESSAGE_COLUMN
            || !self.filter_index.column(column).is_some_and(|c| c.is_filterable());
        let (values, customs) = if custom { (Vec::new(), vec![value.to_string()]) } else { (vec![value.to_string()], Vec::new()) };
        self.rule_form = Some(super::highlights::RuleForm {
            column: Some(column.to_string()),
            values,
            custom: customs,
            color,
            error: None,
        });
        self.reveal_context_section(ContextSection::Highlights, cx);
    }
}

/// The text "Highlight everywhere" would use for a Raw View mark target.
fn everywhere_of(mark: Option<&ColorTarget>) -> Option<String> {
    let Some(ColorTarget::RawMark { pieces, .. }) = mark else { return None };
    let [(_, range, line)] = pieces.as_slice() else { return None };
    line.get(range.clone()).map(str::to_string).filter(|t| crate::text_highlight::can_highlight(t))
}
