//! Logic: working-set tabs and per-tab session persist/restore.

use super::AppView;
use crate::column::{ColumnLayoutState, FieldMaxChars};
use crate::filter::FilterEngine;
use crate::model::{LogEntry, LogLevel};
use crate::search::SearchState;
use crate::workspace::project::{LogFileDescriptor, Project};
use crate::workspace::working_set::{TabSessionState, TabTarget, WorkingSet};
use gpui::{
    Context, ScrollStrategy,
};

impl AppView {
    // ── Issue 01: Multi-Tab Working Set ──────────────────────────────────────
    // The live editor fields (filter/search/layout/selection) mirror the active
    // tab's `TabSessionState`. Every tab switch persists the current session out
    // to the domain WorkingSet and restores the newly focused tab's session in.

    pub(crate) fn active_project(&self) -> Option<&Project> {
        self.workspace_state.active_project()
    }

    pub(crate) fn active_project_mut(&mut self) -> Option<&mut Project> {
        self.workspace_state.active_project_mut()
    }

    /// Persist the live editor state into the active tab's session so no
    /// context is lost when switching or closing tabs.
    pub(crate) fn persist_active_session(&mut self) {
        let (filter, search, layout, selected_row) = (
            self.filter_engine.clone(),
            self.search_state.clone(),
            self.column_layout.clone(),
            self.selected_row_pos,
        );
        if let Some(proj) = self.active_project_mut() {
            if let Some(tab) = proj.working_set.active_tab_mut() {
                tab.session.filter = filter;
                tab.session.search = search;
                tab.session.column_layout = layout;
                tab.session.selected_row = selected_row;
            }
        }
    }

    /// Restore the active tab's independent session + entries into the live
    /// editor fields and recompute the visible stream.
    pub(crate) fn restore_active_session(&mut self, cx: &mut Context<Self>) {
        let restored = {
            let tab = self
                .active_project()
                .and_then(|p| p.working_set.active_tab());
            tab.map(|t| (t.session.clone(), t.target.clone()))
        };

        match restored {
            Some((session, target)) => {
                self.filter_engine = session.filter;
                self.search_state = session.search;
                self.column_layout = session.column_layout;
                self.selected_row_pos = session.selected_row;

                if let TabTarget::LogFile { file_id } = &target {
                    if let Some(entries) = self.entries_cache.get(file_id) {
                        self.all_entries = entries.clone();
                    }
                    if let Some(max_chars) = self.max_chars_by_file.get(file_id) {
                        self.column_layout.max_chars = *max_chars;
                    }
                    if let Some(f) = self
                        .active_project()
                        .and_then(|p| p.file(file_id))
                    {
                        self.file_name = f.display_name.clone();
                    }
                }
            }
            None => {
                self.filter_engine = FilterEngine::new();
                self.search_state = SearchState::new();
                self.column_layout = ColumnLayoutState::new();
                self.selected_row_pos = None;
            }
        }

        self.apply_filters_and_refresh(cx);
        if let Some(pos) = self.selected_row_pos {
            if pos < self.visible_indices.len() {
                self.scroll_handle.scroll_to_item(pos, ScrollStrategy::Center);
            }
        }
    }

    /// Core switching primitive: persist out, apply a WorkingSet mutation, then
    /// restore the newly focused tab's session + entries.
    pub(crate) fn do_switch<'a>(
        &'a mut self,
        op: impl FnOnce(&mut WorkingSet) + 'a,
        cx: &mut Context<Self>,
    ) {
        let has_tabs = self
            .active_project()
            .map(|p| p.working_set.tab_count() > 0)
            .unwrap_or(false);
        if !has_tabs {
            return;
        }
        self.persist_active_session();
        if let Some(proj) = self.active_project_mut() {
            op(&mut proj.working_set);
        }
        self.restore_active_session(cx);
    }

    pub fn switch_to_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        self.do_switch(|ws| ws.set_active_tab(index), cx);
    }

    pub fn next_tab(&mut self, cx: &mut Context<Self>) {
        self.do_switch(|ws| ws.next_tab(), cx);
    }

    pub fn prev_tab(&mut self, cx: &mut Context<Self>) {
        self.do_switch(|ws| ws.prev_tab(), cx);
    }

    pub fn select_tab_number(&mut self, number: usize, cx: &mut Context<Self>) {
        self.do_switch(|ws| ws.select_tab_by_number(number), cx);
    }

    pub fn close_tab_at(&mut self, index: usize, cx: &mut Context<Self>) {
        self.persist_active_session();
        if let Some(proj) = self.active_project_mut() {
            proj.working_set.close_tab(index);
            // Closing a tab keeps the file registered in the project's file
            // list; only the view is removed.
        }
        self.restore_active_session(cx);
    }

    pub fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self
            .active_project()
            .and_then(|p| p.working_set.active_tab_index())
        {
            self.close_tab_at(idx, cx);
        }
    }

    /// Registers a log file in the active project (if new), caches its parsed
    /// entries, and opens/focuses its tab in the working set.
    pub fn open_log_file_in_tab(
        &mut self,
        file_id: String,
        display_name: String,
        entries: Vec<LogEntry>,
        max_chars: FieldMaxChars,
        cx: &mut Context<Self>,
    ) {
        self.persist_active_session();
        self.entries_cache.insert(file_id.clone(), entries);
        self.max_chars_by_file.insert(file_id.clone(), max_chars);

        let session = TabSessionState {
            filter: self.filter_engine.clone(),
            search: self.search_state.clone(),
            column_layout: self.column_layout.clone(),
            selected_row: self.selected_row_pos,
            ..TabSessionState::new()
        };

        if let Some(proj) = self.active_project_mut() {
            if proj.file(&file_id).is_none() {
                proj.register_file(LogFileDescriptor::new(file_id.clone(), display_name));
            }
            proj.open_file(&file_id, session);
        }
        self.restore_active_session(cx);
    }

    pub fn open_file_from_sidebar(&mut self, file_id: &str, cx: &mut Context<Self>) {
        // Switch to the project that owns this file
        if let Some(proj) = self.workspace_state.active_project() {
            if proj.file(file_id).is_some() {
                // File is in active project, open it as a tab
                self.persist_active_session();
                let session = TabSessionState::new();
                if let Some(proj) = self.active_project_mut() {
                    proj.open_file(file_id, session);
                }
                self.restore_active_session(cx);
                cx.notify();
                return;
            }
        }

        // Find which project owns this file and switch to it
        let project_id = self
            .workspace_state
            .projects
            .iter()
            .find(|p| p.file(file_id).is_some())
            .map(|p| p.id.clone());

        if let Some(pid) = project_id {
            self.persist_active_session();
            self.workspace_state.switch_project(&pid);
            let session = TabSessionState::new();
            if let Some(proj) = self.active_project_mut() {
                proj.open_file(file_id, session);
            }
            self.restore_active_session(cx);
            cx.notify();
        }
    }

    pub fn close_tab_for_file(&mut self, file_id: &str, cx: &mut Context<Self>) {
        if let Some(proj) = self.workspace_state.active_project() {
            let target = TabTarget::LogFile {
                file_id: file_id.to_string(),
            };
            if let Some(idx) = proj.working_set.find_tab(&target) {
                self.close_tab_at(idx, cx);
            }
        }
    }

    /// Scan cached entries for error/warning presence used by the tab indicator.
    pub(crate) fn scan_hazards(&self, file_id: &str) -> (bool, bool) {
        self.entries_cache
            .get(file_id)
            .map(|entries| {
                let mut error = false;
                let mut warn = false;
                for entry in entries {
                    match entry.level {
                        LogLevel::Fatal | LogLevel::Error => error = true,
                        LogLevel::Warn => warn = true,
                        _ => {}
                    }
                    if error && warn {
                        break;
                    }
                }
                (error, warn)
            })
            .unwrap_or((false, false))
    }
}
