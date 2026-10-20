//! Logic: search state, match navigation and row jumping. The Search
//! Expression belongs to the open Focus Tab.

use super::AppView;
use super::focus::Origin;
use crate::workspace::working_set::ViewMode;
use gpui::{Context, Window};

impl AppView {
    /// Focus the search field and make sure the active query is surfaced
    /// (match navigator, ruler marks, highlight).
    pub fn activate_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.search_input.read(cx).value().to_string();
        let search = self.focus.search_mut();
        search.query = text;
        search.is_open = true;
        self.search_input.update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    }

    /// Show the open tab's query in the search field. Called from `render`.
    pub fn sync_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_input_dirty {
            return;
        }
        self.search_input_dirty = false;
        let query = self.focus.search().query.clone();
        self.search_input.update(cx, |state, cx| {
            state.set_value(gpui::SharedString::from(query), window, cx);
        });
    }

    pub fn run_search(&mut self, cx: &mut Context<Self>) {
        // Found here and now: a scan under way is not needed.
        self.search_generation += 1;
        if self.view_mode == ViewMode::Raw {
            // Raw View ignores Filter: look at every line of every section.
            self.run_raw_search(cx);
            return;
        }
        self.focus
            .search_mut()
            .update_matches(&self.all_entries, &self.visible_indices);
        self.refresh_focus();
        if let Some(entry_ix) = self.focus.search().current_match_pos() {
            self.goto_entry(entry_ix, Origin::Jump, cx);
        }
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn jump_next_match(&mut self, cx: &mut Context<Self>) {
        if self.view_mode == ViewMode::Raw {
            self.raw_jump_next(cx);
            return;
        }
        if let Some(entry_ix) = self.focus.search_mut().next_match() {
            self.goto_entry(entry_ix, Origin::Jump, cx);
        }
    }

    pub fn jump_prev_match(&mut self, cx: &mut Context<Self>) {
        if self.view_mode == ViewMode::Raw {
            self.raw_jump_prev(cx);
            return;
        }
        if let Some(entry_ix) = self.focus.search_mut().prev_match() {
            self.goto_entry(entry_ix, Origin::Jump, cx);
        }
    }

    /// Jump to a specific search match by absolute entry index (used by the
    /// overview ruler click). Updates the current match so the navigator
    /// counter reflects the new position.
    pub fn jump_to_match_by_entry(&mut self, entry_ix: usize, cx: &mut Context<Self>) {
        let search = self.focus.search_mut();
        if let Some(match_idx) = search.matched_visible_indices.iter().position(|&p| p == entry_ix) {
            search.current_match_idx = match_idx;
        }
        self.jump_to_row(entry_ix, cx);
    }

    /// Select a row of the view and scroll to it. The view shows every entry,
    /// so a row position is the entry's position.
    pub fn jump_to_row(&mut self, vis_pos: usize, cx: &mut Context<Self>) {
        if vis_pos < self.visible_indices.len() {
            self.goto_entry(vis_pos, Origin::Jump, cx);
        }
    }

    /// A row of the view was clicked: select it; the pane follows.
    pub fn select_view_row(&mut self, vis_pos: usize, cx: &mut Context<Self>) {
        if vis_pos < self.visible_indices.len() {
            self.goto_entry(vis_pos, Origin::View, cx);
        }
    }

    pub fn trigger_search_query(&mut self, q: &str, cx: &mut Context<Self>) {
        let search = self.focus.search_mut();
        search.query = q.to_string();
        search.is_open = true;
        self.run_search(cx);
    }

    /// The text of the Search field changed, as the user types. The Matches
    /// are found on a worker thread once the typing pauses (`SEARCH_DEBOUNCE`),
    /// and only for the last text: until then the Matches are those of the
    /// text before it, and the typed expression already highlights.
    pub fn search_text_changed(&mut self, cx: &mut Context<Self>) {
        if self.view_mode == ViewMode::Raw {
            return self.run_search(cx);
        }
        let search = self.focus.search_mut();
        if !search.reparse() && search.parse_error.is_some() {
            // Half typed: the expression before it stays in use.
            cx.notify();
            return;
        }
        if search.matcher.is_none() {
            // Nothing to scan for: the Matches go at once.
            return self.run_search(cx);
        }
        search.pending = true;
        self.search_generation += 1;
        let generation = self.search_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_DEBOUNCE).await;
            this.update(cx, |this, cx| this.start_search_scan(generation, cx)).ok();
        })
        .detach();
        cx.notify();
    }

    /// Type `text` in the Search field, as the user would.
    pub fn type_search_query(&mut self, text: &str, cx: &mut Context<Self>) {
        let search = self.focus.search_mut();
        search.query = text.to_string();
        search.is_open = true;
        self.search_text_changed(cx);
    }

    /// Look for the Matches among every row on a worker thread. The rows are
    /// read by the worker from a copy of their texts, so Live lines can go on
    /// arriving.
    fn start_search_scan(&mut self, generation: u64, cx: &mut Context<Self>) {
        if generation != self.search_generation {
            return;
        }
        let Some(matcher) = self.focus.search().matcher.clone() else { return };
        let lines: Vec<gpui::SharedString> = self.all_entries.iter().map(|e| e.raw_line.clone()).collect();
        let (epoch, scanned) = (self.rows_epoch, lines.len());
        self.refreshes.scans += 1;
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move {
                    let rows: Vec<usize> = (0..lines.len()).collect();
                    crate::par::filter(&rows, |ix| matcher.is_match(&lines[ix]))
                })
                .await;
            this.update(cx, |this, cx| this.finish_search_scan(generation, epoch, scanned, found, cx)).ok();
        })
        .detach();
    }

    fn finish_search_scan(
        &mut self,
        generation: u64,
        epoch: u64,
        scanned: usize,
        found: Vec<usize>,
        cx: &mut Context<Self>,
    ) {
        if generation != self.search_generation {
            return;
        }
        if epoch != self.rows_epoch {
            // Rows left or moved while it ran: look again.
            return self.start_search_scan(generation, cx);
        }
        self.focus.search_mut().set_scanned_matches(found, &self.all_entries, scanned);
        self.refresh_focus();
        if let Some(entry_ix) = self.focus.search().current_match_pos() {
            self.goto_entry(entry_ix, Origin::Jump, cx);
        }
        self.schedule_session_save(cx);
        cx.notify();
    }
}

/// How long the Search field stays still before the Matches are looked for.
pub const SEARCH_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(100);
