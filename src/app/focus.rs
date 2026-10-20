//! Logic: the Focus Pane. Focus Tabs, the lines the open tab lists, Focus
//! Find, the line the view and the pane agree on, and the pane's height.

use super::AppView;
use crate::app::raw::RawSection;
use crate::focus::{FocusPaneNotice, FocusRow, LineRef, compute_focus_rows, nearest_row_before};
use crate::search::SearchState;
use crate::workspace::state::FocusPaneState;
use crate::workspace::working_set::ViewMode;
use crate::text_selection::{TextPos, TextSelection};
use gpui::{Context, Pixels, Point, ScrollStrategy, Window};
use std::sync::Arc;

/// Window height taken by everything outside the view and the Focus Pane
/// (title bar, tabs, toolbar, status bar).
pub const WINDOW_CHROME_HEIGHT: f32 = 170.0;

/// Where a change of the current line came from, which decides what scrolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A click in the view: the view already shows the line, the pane follows.
    View,
    /// A click in the pane: the pane already shows the line, the view follows.
    Pane,
    /// Next/previous Match or Find result: both scroll to the line.
    Jump,
}

impl AppView {
    // ── The open tab ────────────────────────────────────────────────────────

    pub fn active_search(&self) -> &SearchState {
        self.focus.search()
    }

    pub fn active_search_mut(&mut self) -> &mut SearchState {
        self.focus.search_mut()
    }

    /// Whether Filter limits the Focus Tab (only the Parsed View has Filter).
    fn filter_limits_focus(&self) -> bool {
        self.view_mode == ViewMode::Parsed && self.filter_state.active_count() > 0
    }

    // ── Lines ───────────────────────────────────────────────────────────────

    /// The line an entry of the Parsed View is.
    pub fn line_ref_of_entry(&self, entry_ix: usize) -> Option<LineRef> {
        let entry = self.all_entries.get(entry_ix)?;
        Some(LineRef {
            file_id: entry.file_id.clone().unwrap_or_else(|| Arc::from("")),
            line: entry.index,
            generation: entry.generation,
        })
    }

    /// The entry of the Parsed View a line is, if the view has it.
    pub fn entry_of_line_ref(&self, line: &LineRef) -> Option<usize> {
        let is_line = |ix: usize| {
            self.all_entries.get(ix).is_some_and(|e| {
                e.index == line.line
                    && e.generation == line.generation
                    && e.file_id.as_deref().unwrap_or("") == &*line.file_id
            })
        };
        // Called on every render (Inspector) and click: the selected row, or
        // the row a single file puts the line at, is almost always it.
        let likely = [self.selected_row_pos, line.line.checked_sub(1)];
        if let Some(ix) = likely.into_iter().flatten().find(|&ix| is_line(ix)) {
            return Some(ix);
        }
        (0..self.all_entries.len()).find(|&ix| is_line(ix))
    }

    /// The row of the open view a line is: an entry of the Parsed View or a
    /// line of a File Section of the Raw View.
    pub fn focus_row_of_line(&self, line: &LineRef, sections: &[RawSection]) -> Option<FocusRow> {
        match self.view_mode {
            ViewMode::Parsed => self.entry_of_line_ref(line).map(FocusRow::Entry),
            ViewMode::Raw => {
                let section = sections.iter().position(|s| *s.file_id == *line.file_id)?;
                let raw_line = line.line.checked_sub(1)?;
                (raw_line < sections[section].lines.len())
                    .then_some(FocusRow::Raw { section, line: raw_line })
            }
        }
    }

    /// The line a row of the pane is.
    pub fn line_of_focus_row(&self, row: FocusRow, sections: &[RawSection]) -> Option<LineRef> {
        match row {
            FocusRow::Entry(ix) => self.line_ref_of_entry(ix),
            FocusRow::Raw { section, line } => {
                sections.get(section).map(|s| LineRef::new(s.file_id.as_str(), line + 1))
            }
        }
    }

    /// The sections of the Raw View, empty in the Parsed View.
    fn sections_if_raw(&self) -> Vec<RawSection> {
        if self.view_mode == ViewMode::Raw { self.raw_sections() } else { Vec::new() }
    }

    /// The stored text of every row of the open tab, in order.
    fn focus_row_texts<'a>(
        entries: &'a [crate::model::LogEntry],
        sections: &'a [RawSection],
        rows: &'a [FocusRow],
    ) -> impl Iterator<Item = &'a str> {
        rows.iter().map(move |row| match *row {
            FocusRow::Entry(ix) => entries.get(ix).map_or("", |e| e.raw_line.as_str()),
            FocusRow::Raw { section, line } => sections
                .get(section)
                .and_then(|s| s.lines.get(line))
                .map_or("", String::as_str),
        })
    }

    // ── Rows of the open tab ────────────────────────────────────────────────

    /// Work out the lines the open tab lists and the Focus Find hits among
    /// them. The tab's current line stays whatever it was.
    pub fn refresh_focus(&mut self) {
        let rows: Vec<FocusRow> = match self.view_mode {
            ViewMode::Parsed => {
                let search = self.focus.search();
                if search.matcher.is_some() {
                    // The Matches are every line of the view; the pane lists
                    // those that also pass Filter.
                    let limited = self.filter_limits_focus();
                    search
                        .matched_visible_indices
                        .iter()
                        .filter(|&&ix| !limited || self.filter_pass.binary_search(&ix).is_ok())
                        .map(|&ix| FocusRow::Entry(ix))
                        .collect()
                } else {
                    compute_focus_rows(&self.all_entries, &self.filter_pass, self.filter_limits_focus(), None)
                }
            }
            ViewMode::Raw => self
                .raw
                .matches
                .iter()
                .map(|m| FocusRow::Raw { section: m.section, line: m.line })
                .collect(),
        };
        self.focus_rows = Arc::new(rows);
        let sections = self.sections_if_raw();
        let texts: Vec<&str> =
            Self::focus_row_texts(&self.all_entries, &sections, &self.focus_rows).collect();
        self.focus_longest = super::filter::longest(texts.iter().map(|t| t.len()));
        self.focus.find_mut().compute_hits(texts.into_iter(), false);
        self.pane_selection = None;
    }

    /// `refresh_focus` after rows were appended to the view: the pane lists
    /// the new Matches (or, with no Search Expression, the new rows that pass
    /// Filter), after the rows it had. `matches_before` and `pass_before`
    /// are how many Matches and rows passing Filter there were before.
    pub(crate) fn extend_focus(&mut self, matches_before: usize, pass_before: usize) {
        if self.view_mode != ViewMode::Parsed {
            return self.refresh_focus();
        }
        let limited = self.filter_limits_focus();
        let first_new = self.focus_rows.len();
        let search = self.focus.search();
        let new_rows: Vec<FocusRow> = if search.matcher.is_some() {
            search.matched_visible_indices[matches_before.min(search.matched_visible_indices.len())..]
                .iter()
                .filter(|&&ix| !limited || self.filter_pass.binary_search(&ix).is_ok())
                .map(|&ix| FocusRow::Entry(ix))
                .collect()
        } else if limited {
            self.filter_pass[pass_before..].iter().map(|&ix| FocusRow::Entry(ix)).collect()
        } else {
            Vec::new()
        };
        let text = |row: &FocusRow| match *row {
            FocusRow::Entry(ix) => self.all_entries.get(ix).map_or("", |e| e.raw_line.as_str()),
            FocusRow::Raw { .. } => "",
        };
        let mut longest_len = match self.focus_rows.get(self.focus_longest) {
            Some(row) if first_new > 0 => text(row).len(),
            _ => 0,
        };
        let mut longest_pos = if first_new > 0 { self.focus_longest } else { 0 };
        for (i, row) in new_rows.iter().enumerate() {
            if text(row).len() > longest_len {
                longest_len = text(row).len();
                longest_pos = first_new + i;
            }
        }
        let texts: Vec<&str> = new_rows.iter().map(text).collect();
        self.focus.find_mut().extend_hits(texts.into_iter(), first_new);
        self.focus_longest = longest_pos;
        Arc::make_mut(&mut self.focus_rows).extend(new_rows);
        self.pane_selection = None;
    }

    /// Called when lines are appended to the open view (tail). Filter and
    /// Search are evaluated again over the whole view, and the tab's current
    /// line stays when it is still listed.
    pub fn on_entries_appended(&mut self, cx: &mut Context<Self>) {
        self.apply_filters_and_refresh(cx);
    }

    // ── Current line ────────────────────────────────────────────────────────

    /// The line the open tab considers current.
    pub fn current_line(&self) -> Option<LineRef> {
        self.focus.active_tab().current.clone()
    }

    /// Make `line` the current line of the open tab: the view selects it, the
    /// pane selects it when it lists it, and whichever side did not originate
    /// the change scrolls to it.
    pub fn set_current_line(&mut self, line: LineRef, origin: Origin, cx: &mut Context<Self>) {
        self.focus.active_tab_mut().current = Some(line.clone());
        match self.view_mode {
            ViewMode::Parsed => {
                if let Some(ix) = self.entry_of_line_ref(&line) {
                    self.selected_row_pos = Some(ix);
                    if origin != Origin::View {
                        self.scroll_handle.scroll_to_item(ix, ScrollStrategy::Center);
                    }
                }
            }
            ViewMode::Raw => {
                if let Some(raw_line) = line.line.checked_sub(1) {
                    self.raw.selected = Some((line.file_id.to_string(), raw_line));
                    if origin != Origin::View {
                        let sections = self.raw_sections();
                        if let Some(section) = sections.iter().position(|s| *s.file_id == *line.file_id) {
                            self.reveal_raw_line(section, raw_line, cx);
                        }
                    }
                }
            }
        }
        if origin != Origin::Pane {
            self.scroll_pane_to_current();
        }
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Select an entry of the Parsed View as the current line.
    pub fn goto_entry(&mut self, entry_ix: usize, origin: Origin, cx: &mut Context<Self>) {
        match self.line_ref_of_entry(entry_ix) {
            Some(line) => self.set_current_line(line, origin, cx),
            None => {
                self.selected_row_pos = Some(entry_ix);
                if origin != Origin::View {
                    self.scroll_handle.scroll_to_item(entry_ix, ScrollStrategy::Center);
                }
                cx.notify();
            }
        }
    }

    /// Select a row of the pane as the current line.
    pub fn goto_pane_row(&mut self, index: usize, origin: Origin, cx: &mut Context<Self>) {
        let Some(row) = self.focus_rows.get(index).copied() else { return };
        self.focus.find_mut().select_row(index);
        let sections = self.sections_if_raw();
        if let Some(line) = self.line_of_focus_row(row, &sections) {
            self.set_current_line(line, origin, cx);
        }
    }

    /// Index in the pane of the row the current line is, when the pane lists it.
    pub fn pane_selected_index(&self) -> Option<usize> {
        let current = self.current_line()?;
        let sections = self.sections_if_raw();
        let row = self.focus_row_of_line(&current, &sections)?;
        self.focus_rows.binary_search(&row).ok()
    }

    /// Scroll the pane to the current line, or to the nearest listed line
    /// before it when the tab does not list it.
    pub fn scroll_pane_to_current(&mut self) {
        let Some(current) = self.current_line() else { return };
        let sections = self.sections_if_raw();
        let Some(row) = self.focus_row_of_line(&current, &sections) else { return };
        let index = match self.focus_rows.binary_search(&row) {
            Ok(i) => Some(i),
            Err(_) => nearest_row_before(&self.focus_rows, row),
        };
        if let Some(i) = index {
            self.focus_scroll.scroll_to_item(i, ScrollStrategy::Center);
        }
    }

    /// Select the current line in the view (after a view or tab change) and
    /// bring it into sight, when the open tab lists it. Otherwise the view
    /// keeps its own selection and the tab follows it.
    pub fn apply_current_line(&mut self, cx: &mut Context<Self>) {
        let sections = self.sections_if_raw();
        let listed = self.current_line().and_then(|line| {
            let row = self.focus_row_of_line(&line, &sections)?;
            self.focus_rows.binary_search(&row).ok().map(|_| line)
        });
        match listed {
            Some(line) => self.set_current_line(line, Origin::Jump, cx),
            None => self.adopt_view_selection(),
        }
    }

    /// Make the open tab's current line the one the view has selected.
    pub fn adopt_view_selection(&mut self) {
        let line = match self.view_mode {
            ViewMode::Parsed => self.selected_row_pos.and_then(|p| self.line_ref_of_entry(p)),
            ViewMode::Raw => self.raw.selected.as_ref().map(|(f, l)| LineRef::new(f.as_str(), l + 1)),
        };
        if line.is_some() {
            self.focus.active_tab_mut().current = line;
        }
    }

    // ── Focus Tabs ──────────────────────────────────────────────────────────

    /// Work out again what the open tab shows after another tab opened.
    fn after_focus_tab_changed(&mut self, cx: &mut Context<Self>) {
        self.search_input_dirty = true;
        self.focus_find_dirty = true;
        match self.view_mode {
            ViewMode::Parsed => {
                self.search_generation += 1;
                self.active_search_mut().reparse();
                self.focus.search_mut().update_matches(&self.all_entries, &self.visible_indices);
            }
            ViewMode::Raw => {
                self.run_raw_search_keep_position(cx);
            }
        }
        self.refresh_focus();
        self.apply_current_line(cx);
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn switch_focus_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index == self.focus.active || index >= self.focus.tabs.len() {
            return;
        }
        let open = self.focus.search().is_open;
        self.focus.set_active(index);
        self.focus.search_mut().is_open = open;
        self.after_focus_tab_changed(cx);
    }

    /// "+": a new tab opens with the Search Expression now in the toolbar, so
    /// the field keeps its text; the cursor goes to the Search field to edit it.
    pub fn add_focus_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.focus.search().is_open;
        let query = self.focus.search().query.clone();
        self.focus.add();
        let search = self.focus.search_mut();
        search.is_open = open;
        search.query = query;
        search.reparse();
        self.after_focus_tab_changed(cx);
        self.sync_search_input(window, cx);
        self.search_input.update(cx, |state, cx| state.focus(window, cx));
    }

    pub fn close_focus_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let open_number = self.focus.active_tab().number;
        let open = self.focus.search().is_open;
        if !self.focus.close(index) {
            return;
        }
        self.focus.search_mut().is_open = open;
        if self.focus.active_tab().number != open_number {
            self.after_focus_tab_changed(cx);
        } else {
            self.schedule_session_save(cx);
            cx.notify();
        }
    }

    // ── Focus Find ──────────────────────────────────────────────────────────

    /// The Focus Find field's text changed.
    pub fn on_focus_find_changed(&mut self, text: &str, cx: &mut Context<Self>) {
        let sections = self.sections_if_raw();
        let texts: Vec<&str> =
            Self::focus_row_texts(&self.all_entries, &sections, &self.focus_rows).collect();
        self.focus.find_mut().set_query(text, texts.into_iter());
        if let Some(row) = self.focus.find().current_row() {
            self.goto_pane_row(row, Origin::Jump, cx);
        }
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn focus_find_next(&mut self, cx: &mut Context<Self>) {
        if let Some(row) = self.focus.find_mut().next_hit() {
            self.goto_pane_row(row, Origin::Jump, cx);
        }
    }

    pub fn focus_find_prev(&mut self, cx: &mut Context<Self>) {
        if let Some(row) = self.focus.find_mut().prev_hit() {
            self.goto_pane_row(row, Origin::Jump, cx);
        }
    }

    /// Show the open tab's Focus Find text in its field. Called from `render`.
    pub fn sync_focus_find_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus_find_dirty {
            return;
        }
        self.focus_find_dirty = false;
        let query = self.focus.find().query.clone();
        self.focus_find_input.update(cx, |state, cx| {
            state.set_value(gpui::SharedString::from(query), window, cx);
        });
    }

    // ── Pane layout ─────────────────────────────────────────────────────────

    /// Expand the pane when a Filter or Search Expression appears. Called from
    /// `render`; a pane the user collapsed stays collapsed until the next time
    /// there is something to list.
    /// Whether there is anything to list: a Filter (Parsed View) or a Search
    /// Expression in any Focus Tab. The pane is not shown without it.
    /// What the Focus Pane says while it lists no lines; `None` when it
    /// lists some.
    pub fn focus_pane_notice(&self) -> Option<FocusPaneNotice> {
        if !self.focus_rows.is_empty() {
            return None;
        }
        let filtering = self.view_mode == ViewMode::Parsed && self.filter_state.active_count() > 0;
        if filtering && self.filter_pass.is_empty() {
            if let Some(notice) = self.unread_filter_notice() {
                return Some(notice);
            }
        }
        let search = self.focus.search();
        let expression = search.matcher.is_some().then_some(search.query.as_str());
        Some(FocusPaneNotice::for_empty_pane(expression, filtering, self.filter_pass.is_empty()))
    }

    /// The Match counter beside the Search field.
    pub fn match_counter_label(&self) -> String {
        let display = if self.view_mode == ViewMode::Raw {
            self.raw.match_display()
        } else {
            self.focus.search().current_match_display()
        };
        SearchState::counter_label(self.focus.search().matcher.is_some(), display)
    }

    pub fn focus_pane_has_content(&self) -> bool {
        let filtering = self.view_mode == ViewMode::Parsed && self.filter_state.active_count() > 0;
        filtering || self.focus.tabs.iter().any(|t| t.search.matcher.is_some())
    }

    pub fn sync_focus_pane_visibility(&mut self, cx: &mut Context<Self>) {
        let active = self.focus_pane_has_content();
        if active && !self.pane_has_content && !self.workspace_state.focus_pane.visible {
            self.workspace_state.focus_pane.visible = true;
            self.schedule_session_save(cx);
        }
        self.pane_has_content = active;
    }

    pub fn toggle_focus_pane(&mut self, cx: &mut Context<Self>) {
        // In the Focus Window the pane cannot be hidden; bring the window up.
        if self.activate_focus_window(cx) {
            return;
        }
        self.workspace_state.focus_pane.toggle();
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Height the pane gets in a window `viewport_height` tall, leaving the
    /// view its minimum.
    pub fn focus_pane_height(&self, viewport_height: f32) -> f32 {
        if !self.workspace_state.focus_pane.visible {
            return crate::ui::focus_pane::COLLAPSED_HEIGHT;
        }
        FocusPaneState::clamp_height(
            self.workspace_state.focus_pane.height,
            viewport_height - WINDOW_CHROME_HEIGHT,
        )
    }

    pub fn start_resizing_focus_pane(&mut self, cx: &mut Context<Self>) {
        self.workspace_state.focus_pane.is_resizing = true;
        cx.notify();
    }

    /// The handle was dragged to window y `y`: the pane reaches from there to
    /// the status bar.
    pub fn update_focus_pane_resize(&mut self, y: gpui::Pixels, viewport_height: f32, cx: &mut Context<Self>) {
        const STATUS_BAR_HEIGHT: f32 = 24.0;
        let height = viewport_height - STATUS_BAR_HEIGHT - f32::from(y);
        self.workspace_state
            .focus_pane
            .set_height(height, viewport_height - WINDOW_CHROME_HEIGHT);
        cx.notify();
    }

    pub fn finish_resizing_focus_pane(&mut self, cx: &mut Context<Self>) {
        if self.workspace_state.focus_pane.is_resizing {
            self.workspace_state.focus_pane.is_resizing = false;
            self.schedule_session_save(cx);
            cx.notify();
        }
    }

    pub fn reset_focus_pane_height(&mut self, cx: &mut Context<Self>) {
        self.workspace_state.focus_pane.height = crate::workspace::state::DEFAULT_FOCUS_PANE_HEIGHT;
        self.schedule_session_save(cx);
        cx.notify();
    }

    // ── Selecting text in the pane ──────────────────────────────────────────

    /// Where a pointer at window position `at` is in the pane's rows: the
    /// pane row (kept inside the rows listed) and the byte of its line.
    fn pane_pos_at(&self, at: Point<Pixels>, window: &Window, cx: &gpui::App) -> Option<TextPos> {
        let total = self.focus_rows.len();
        if total == 0 {
            return None;
        }
        let bounds = self.pane_bounds.get();
        let offset = self.focus_scroll.0.borrow().base_handle.offset();
        let row_h = self.density.row_height();
        let y = f32::from(at.y - bounds.origin.y) - f32::from(offset.y);
        let row = ((y / row_h).floor().max(0.0) as usize).min(total - 1);
        if let Some(layout) = self.pane_columns() {
            // The same columns as the view, none pinned, scrolled with the pane.
            let entry = match self.focus_rows.get(row) {
                Some(crate::focus::FocusRow::Entry(ix)) => self.all_entries.get(*ix)?,
                _ => return None,
            };
            let hit = crate::ui::cells::hit_cell(&layout, f32::from(at.x - bounds.origin.x), -f32::from(offset.x));
            let font = self.density.font_size();
            let byte = crate::ui::cells::byte_for_hit(entry, hit, |text, x| {
                crate::ui::mono::index_for_x(text, x, font, window, cx)
            });
            return Some(TextPos::new(row, byte));
        }
        let parsed = self.view_mode == ViewMode::Parsed;
        let x = f32::from(at.x - bounds.origin.x)
            - f32::from(offset.x)
            - crate::ui::focus_pane::text_offset(parsed, self.pane_shows_source());
        let sections = self.sections_if_raw();
        let text = Self::focus_row_texts(&self.all_entries, &sections, &self.focus_rows[row..=row])
            .next()
            .unwrap_or("");
        let byte = crate::ui::mono::index_for_x(text, x, self.density.font_size(), window, cx);
        Some(TextPos::new(row, byte))
    }

    /// The columns the pane shows in the Parsed View: the view's, with none
    /// pinned (the pane scrolls as one). `None` in the Raw View.
    pub fn pane_columns(&self) -> Option<crate::column::ColumnLayoutState> {
        (self.view_mode == ViewMode::Parsed).then(|| {
            let mut layout = self.column_layout.clone();
            layout.unpin_all();
            layout
        })
    }

    /// Whether the pane names the Source File of each line (the Raw View;
    /// the Parsed View has the Source Column when it is turned on).
    pub fn pane_shows_source(&self) -> bool {
        self.active_space().is_some_and(|s| s.files.len() > 1)
            && matches!(
                self.active_target(),
                Some(crate::workspace::working_set::TabTarget::LogSpace { .. })
            )
    }

    pub fn start_pane_selection(&mut self, at: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        self.text_selection = None;
        self.color_bar = None;
        match self.pane_pos_at(at, window, cx) {
            Some(pos) => {
                self.pane_selection = Some(TextSelection::new(pos, pos));
                self.pane_selecting = true;
            }
            None => self.pane_selection = None,
        }
        cx.notify();
    }

    pub fn drag_pane_selection(&mut self, at: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        if !self.pane_selecting {
            return;
        }
        if let (Some(pos), Some(sel)) = (self.pane_pos_at(at, window, cx), self.pane_selection.as_mut()) {
            sel.move_head(pos);
        }
        cx.notify();
    }

    /// The button went up: a drag leaves a selection, a plain click makes the
    /// row under the pointer the current line.
    pub fn finish_pane_selection(&mut self, at: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        if !self.pane_selecting {
            return;
        }
        self.pane_selecting = false;
        match self.pane_selection {
            Some(sel) if !sel.is_empty() => {}
            _ => {
                self.pane_selection = None;
                if let Some(pos) = self.pane_pos_at(at, window, cx) {
                    self.goto_pane_row(pos.row, Origin::Pane, cx);
                }
            }
        }
        cx.notify();
    }

    /// The stored text of the pane's selection, one line per row listed.
    pub fn pane_selected_text(&self) -> Option<String> {
        let sel = self.pane_selection.filter(|s| !s.is_empty())?;
        let sections = self.sections_if_raw();
        let rows = Self::focus_row_texts(&self.all_entries, &sections, &self.focus_rows)
            .enumerate();
        Some(sel.copy_text(rows))
    }

    /// ⌘C with text selected in the pane.
    pub fn copy_pane_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.pane_selected_text() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
    }
}
