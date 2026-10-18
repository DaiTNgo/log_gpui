//! Logic: search state, match navigation and row jumping. The Search
//! Expression belongs to the open Focus Tab.

use super::AppView;
use super::focus::Origin;
use crate::ui::floating_panel::PanelKind;
use crate::workspace::working_set::ViewMode;
use gpui::{Context, Window};

impl AppView {
    pub fn toggle_search(&mut self, cx: &mut Context<Self>) {
        let open = !self.focus.search().is_open;
        self.focus.search_mut().is_open = open;
        if open {
            self.toggle_panel(PanelKind::Filter, cx);
        }
        cx.notify();
    }

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
        if self.view_mode == ViewMode::Raw {
            // Raw View ignores Filter: look at every line of every section.
            self.run_raw_search(cx);
            return;
        }
        self.focus
            .search_mut()
            .update_matches(&self.all_entries, &self.filter_pass);
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
}
