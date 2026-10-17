//! Logic: search state, match navigation and row jumping.

use super::AppView;
use crate::ui::floating_panel::PanelKind;
use gpui::{
    Context, ScrollStrategy,
    Window,
};

impl AppView {
    pub fn toggle_search(&mut self, cx: &mut Context<Self>) {
        self.search_state.is_open = !self.search_state.is_open;
        if self.search_state.is_open {
            self.toggle_panel(PanelKind::Filter, cx);
        }
        cx.notify();
    }

    /// Focus the search field and make sure the active query is surfaced
    /// (match navigator, ruler marks, highlight).
    pub fn activate_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.search_input.read(cx).text().to_string();
        self.search_state.query = text;
        self.search_state.is_open = true;
        self.search_input.update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    }

    pub fn run_search(&mut self, cx: &mut Context<Self>) {
        self.search_state
            .update_matches(&self.all_entries, &self.visible_indices);
        if let Some(target_vis_pos) = self
            .search_state
            .matched_visible_indices
            .get(self.search_state.current_match_idx)
            .copied()
        {
            self.scroll_handle
                .scroll_to_item(target_vis_pos, ScrollStrategy::Center);
            self.selected_row_pos = Some(target_vis_pos);
        }
        cx.notify();
    }

    pub fn jump_next_match(&mut self, cx: &mut Context<Self>) {
        if let Some(vis_pos) = self.search_state.next_match() {
            self.scroll_handle
                .scroll_to_item(vis_pos, ScrollStrategy::Center);
            self.selected_row_pos = Some(vis_pos);
            cx.notify();
        }
    }

    pub fn jump_prev_match(&mut self, cx: &mut Context<Self>) {
        if let Some(vis_pos) = self.search_state.prev_match() {
            self.scroll_handle
                .scroll_to_item(vis_pos, ScrollStrategy::Center);
            self.selected_row_pos = Some(vis_pos);
            cx.notify();
        }
    }

    /// Jump to a specific search match by absolute entry index (used by the
    /// overview ruler click). Updates the current match so the navigator
    /// counter reflects the new position.
    pub fn jump_to_match_by_entry(&mut self, entry_ix: usize, cx: &mut Context<Self>) {
        if let Some(vis_pos) = self.visible_indices.iter().position(|&e| e == entry_ix) {
            if let Some(match_idx) = self
                .search_state
                .matched_visible_indices
                .iter()
                .position(|&p| p == vis_pos)
            {
                self.search_state.current_match_idx = match_idx;
            }
            self.jump_to_row(vis_pos, cx);
        }
    }

    pub fn jump_to_row(&mut self, vis_pos: usize, cx: &mut Context<Self>) {
        if vis_pos < self.visible_indices.len() {
            self.selected_row_pos = Some(vis_pos);
            self.scroll_handle
                .scroll_to_item(vis_pos, ScrollStrategy::Center);
            cx.notify();
        }
    }

    pub fn trigger_search_query(&mut self, q: &str, cx: &mut Context<Self>) {
        self.search_state.query = q.to_string();
        self.search_state.is_open = true;
        self.run_search(cx);
    }
}
