//! Logic: working-set tabs and per-tab session persist/restore.
//!
//! The live editor fields (filter/search/layout/selection) mirror the active
//! tab's `TabSessionState`. Every tab switch persists the current session out
//! to the domain WorkingSet and restores the newly focused tab's session in.
//!
//! A tab is a Log Space (its files merged by timestamp) or a File View (one
//! file). The Filter belongs to the view: one shared by a Log Space, one of
//! its own for each File View. Markers belong to lines of files.

use super::AppView;
use crate::column::FieldMaxChars;
use crate::filter::FilterState;
use crate::focus::FocusTabs;
use crate::live::LiveBuffer;
use crate::space_view::{BuiltView, EmptyCanvas, ParsedFile, build_file_view, build_space_view};
use crate::archive::{ArchiveError, MAX_UNCOMPRESSED_BYTES, detect, read_entries, read_source};
use crate::workspace::log_space::{LogFile, LogSpace, LogSpaceKind};
use crate::workspace::project::Project;
use crate::workspace::working_set::{CloseScope, TabSessionState, TabTarget, ViewMode, WorkingSet};
use gpui::{Context, ScrollStrategy};
use std::path::Path;

/// The notice for a file that could not be read; an Archive Entry is named
/// by its display name.
fn read_failure(file: &LogFile, e: &std::io::Error) -> String {
    let name = if file.entry.is_some() {
        file.display_name.as_str()
    } else {
        file.path.as_deref().unwrap_or(&file.display_name)
    };
    if e.kind() == std::io::ErrorKind::NotFound {
        format!("{name} was not found")
    } else {
        format!("Could not read {name}: {e}")
    }
}

/// What a tab's view opens with.
pub(crate) struct OpenView {
    pub built: BuiltView,
    pub filter: FilterState,
    pub focus: FocusTabs,
    pub source_visible: bool,
    pub name: String,
}

impl OpenView {
    fn empty() -> Self {
        Self {
            built: BuiltView::default(),
            filter: FilterState::new(),
            focus: FocusTabs::default(),
            source_visible: false,
            name: String::new(),
        }
    }
}

impl AppView {
    /// Whether the focused tab shows log lines (gates the context sidebar).
    /// What the canvas shows instead of rows, or `None` when it shows rows:
    /// nothing open, a Log Space with no files, or a view whose every Source
    /// File failed to load.
    pub fn empty_canvas(&self) -> Option<EmptyCanvas> {
        let space = self.active_space().filter(|_| self.active_tab_is_log_file());
        let Some(space) = space else {
            let log_space_open = matches!(self.active_target(), Some(TabTarget::LogSpace { .. }));
            return Some(if log_space_open { EmptyCanvas::NoFiles { can_add: false } } else { EmptyCanvas::NoLogSpace });
        };
        if space.files.is_empty() {
            return Some(EmptyCanvas::NoFiles { can_add: space.kind == LogSpaceKind::Imported });
        }
        if !self.all_entries.is_empty() {
            return None;
        }
        // A File View shows one file; a Log Space all of its files.
        let viewed = match self.active_target() {
            Some(TabTarget::FileView { file_id, .. }) => vec![file_id],
            _ => space.files.iter().map(|f| f.file_id.clone()).collect(),
        };
        let errors: Vec<String> = viewed
            .iter()
            .map(|id| self.parsed_files.get(id).and_then(|p| p.load_error.clone()))
            .collect::<Option<_>>()?;
        Some(EmptyCanvas::LoadFailed { errors })
    }

    pub(crate) fn active_tab_is_log_file(&self) -> bool {
        self.active_target()
            .map(|t| t.is_log_view())
            .unwrap_or(false)
    }

    pub(crate) fn active_target(&self) -> Option<TabTarget> {
        self.active_project()
            .and_then(|p| p.working_set.active_tab())
            .map(|t| t.target.clone())
    }

    pub(crate) fn active_project(&self) -> Option<&Project> {
        self.workspace_state.active_project()
    }

    pub(crate) fn active_project_mut(&mut self) -> Option<&mut Project> {
        self.workspace_state.active_project_mut()
    }

    /// The Log Space the focused tab views.
    pub(crate) fn active_space(&self) -> Option<&LogSpace> {
        let target = self.active_target()?;
        let space_id = target.space_id()?.to_string();
        self.active_project()?.log_space(&space_id)
    }

    /// Persist the live editor state into the active tab's session so no
    /// context is lost when switching or closing tabs.
    ///
    /// The Filter belongs to the view (the Log Space, or the File View). Markers
    /// are edited on the files of the Log Space directly.
    pub(crate) fn persist_active_session(&mut self) {
        self.flush_scratch_saves();
        let (layout, selected_row, mode) = (
            self.column_layout.clone(),
            self.selected_row_pos,
            self.view_mode,
        );
        let filter = self.filter_state.clone();
        let focus = self.focus.clone();
        let mut target = None;
        if let Some(proj) = self.workspace_state.active_project_mut() {
            if let Some(tab) = proj.working_set.active_tab_mut() {
                tab.session.column_layout = layout;
                tab.session.selected_row = selected_row;
                tab.session.mode = mode;
                target = Some(tab.target.clone());
            }
            match target {
                Some(TabTarget::LogSpace { space_id }) => {
                    if let Some(space) = proj.log_space_mut(&space_id) {
                        space.filter = filter;
                        space.focus = focus;
                        space.last_mode = mode;
                    }
                }
                Some(TabTarget::FileView { space_id, file_id }) => {
                    if let Some(space) = proj.log_space_mut(&space_id) {
                        space.set_file_filter(&file_id, filter);
                        space.set_file_focus(&file_id, focus);
                        space.last_mode = mode;
                    }
                }
                _ => {}
            }
        }
    }

    /// Make sure the file is parsed and cached. A file that cannot be read
    /// counts as empty and leaves a notice.
    pub(crate) fn ensure_file_loaded(&mut self, file: &LogFile) {
        if self.parsed_files.contains_key(&file.file_id) {
            return;
        }
        let parsed = match file.source() {
            Some(source) => match ParsedFile::from_source(source) {
                Ok(parsed) => parsed,
                Err(e) => {
                    let error = read_failure(file, &e);
                    self.notice = Some(error.clone());
                    ParsedFile::failed(error)
                }
            },
            None => ParsedFile::empty(),
        };
        self.parsed_files.insert(file.file_id.clone(), parsed);
    }

    /// Load a file that is read live: what is on disk now, and the state the
    /// source needs to carry on from there. An Archive Entry is static: it
    /// is read once and has no live state.
    pub(crate) fn ensure_live_file_loaded(&mut self, file: &LogFile) {
        if self.parsed_files.contains_key(&file.file_id) {
            return;
        }
        let Some(source) = file.source() else {
            self.parsed_files.insert(file.file_id.clone(), ParsedFile::empty());
            return;
        };
        let bytes = match read_source(source) {
            Ok(bytes) => bytes,
            Err(e) => {
                let error = read_failure(file, &e);
                self.notice = Some(error.clone());
                self.parsed_files.insert(file.file_id.clone(), ParsedFile::failed(error));
                return;
            }
        };
        let mut parsed = ParsedFile::from_bytes(&bytes);
        if file.entry.is_none() {
            parsed.live = Some(LiveBuffer::loaded_from_bytes(&file.display_name, &bytes, parsed.entries.len()));
        }
        self.parsed_files.insert(file.file_id.clone(), parsed);
    }

    /// Read the Archive Entries of a space that are not cached yet, once per
    /// Archive, so a `.tar.gz` is unpacked a single time however many of its
    /// entries the space holds.
    fn ensure_archive_entries_loaded(&mut self, space: &LogSpace) {
        let mut by_archive: Vec<(&str, Vec<&LogFile>)> = Vec::new();
        for file in &space.files {
            let (Some(archive), Some(_)) = (file.path.as_deref(), file.entry.as_deref()) else { continue };
            if self.parsed_files.contains_key(&file.file_id) {
                continue;
            }
            match by_archive.iter_mut().find(|(a, _)| *a == archive) {
                Some((_, files)) => files.push(file),
                None => by_archive.push((archive, vec![file])),
            }
        }
        let mut notices = Vec::new();
        for (archive, files) in by_archive {
            let names: Vec<&str> = files.iter().filter_map(|f| f.entry.as_deref()).collect();
            let path = Path::new(archive);
            let result = match detect(path) {
                Some(kind) => read_entries(path, kind, &names, MAX_UNCOMPRESSED_BYTES),
                None if path.exists() => Err(ArchiveError::Damaged("not an archive".into())),
                None => Err(ArchiveError::NotFound),
            };
            let mut entries = match result {
                Ok(entries) => entries,
                Err(e) => {
                    let e = std::io::Error::from(e);
                    for file in files {
                        let error = read_failure(file, &e);
                        notices.push(error.clone());
                        self.parsed_files.insert(file.file_id.clone(), ParsedFile::failed(error));
                    }
                    continue;
                }
            };
            for file in files {
                let content = file.entry.as_deref().and_then(|name| entries.remove(name));
                let parsed = match content {
                    Some(Ok(bytes)) => ParsedFile::from_bytes(&bytes),
                    Some(Err(e)) => {
                        let error = read_failure(file, &std::io::Error::from(e));
                        notices.push(error.clone());
                        ParsedFile::failed(error)
                    }
                    None => ParsedFile::empty(),
                };
                self.parsed_files.insert(file.file_id.clone(), parsed);
            }
        }
        if !notices.is_empty() {
            self.notice = Some(notices.join("; "));
        }
    }

    /// A Remote File or Remote Command starts empty: its lines come from its
    /// source. The lines of a command are numbered from 1 as it prints them.
    pub(crate) fn ensure_remote_file_loaded(&mut self, file_id: &str, name: &str, is_command: bool) {
        if self.parsed_files.contains_key(file_id) {
            return;
        }
        let mut parsed = ParsedFile::empty();
        parsed.live = Some(if is_command { LiveBuffer::command(name) } else { LiveBuffer::remote(name) });
        self.parsed_files.insert(file_id.to_string(), parsed);
    }

    /// Put freshly built rows into the live fields, keeping the Filter, the
    /// Focus Tabs and every other setting of the open view.
    pub(crate) fn install_live_rows(&mut self, open: OpenView) {
        let OpenView { built, source_visible, .. } = open;
        self.no_timestamp_files = built.no_timestamp_files.iter().cloned().collect();
        // Other rows: a scan of the old ones is stale.
        self.rows_epoch += 1;
        self.all_entries = built.entries;
        self.filter_index = built.index;
        self.column_layout.max_chars = built.max_chars;
        self.read_rule_fields();
        self.column_layout.set_source_visible(source_visible);
        self.sync_stream_column();
    }

    /// Load every file of the Log Space into the cache and re-parse the
    /// scratch files edited since they were parsed.
    pub(crate) fn ensure_space_loaded(&mut self, space: &LogSpace) {
        self.ensure_archive_entries_loaded(space);
        for file in &space.files {
            if file.is_remote_copy() {
                self.ensure_copy_loaded(file);
                continue;
            }
            match space.kind {
                LogSpaceKind::Scratch => self.ensure_file_loaded(file),
                LogSpaceKind::Remote => {
                    self.ensure_remote_file_loaded(&file.file_id, &file.display_name, file.is_remote_command())
                }
                _ => self.ensure_live_file_loaded(file),
            }
        }
        self.reparse_edited_scratch_files();
    }

    /// Build the rows of a tab's view, with the Filter and Focus Tabs it keeps.
    pub(crate) fn build_view_for(&mut self, target: &TabTarget) -> Option<OpenView> {
        let space = self.active_project()?.log_space(target.space_id()?)?.clone();
        self.ensure_space_loaded(&space);
        self.apply_log_formats(&space);
        match target {
            TabTarget::LogSpace { .. } => {
                let built = build_space_view(&space, &self.parsed_files);
                Some(OpenView {
                    built,
                    filter: space.filter.clone(),
                    focus: space.focus.clone(),
                    source_visible: self
                        .workspace_state
                        .show_source
                        .unwrap_or_else(|| space.source_column_shown()),
                    name: space.name.clone(),
                })
            }
            TabTarget::FileView { file_id, .. } => {
                let file = space.file(file_id)?;
                let parsed = self.parsed_files.get(file_id)?;
                let built = build_file_view(file, parsed);
                Some(OpenView {
                    built,
                    filter: space.file_filter(file_id),
                    focus: space.file_focus(file_id),
                    source_visible: false,
                    name: file.display_name.clone(),
                })
            }
            TabTarget::InvestigationBoard { .. } => None,
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
                self.search_input_dirty = true;
                self.focus_find_dirty = true;
                self.column_layout = session.column_layout;
                self.selected_row_pos = session.selected_row;
                self.view_mode = session.mode;
                self.raw.clear_search();

                if let Some(open) = self.build_view_for(&target) {
                    self.install_view(open);
                } else if target.is_log_view() {
                    // The Log Space or file is gone: show nothing.
                    self.install_view(OpenView::empty());
                }
            }
            None => {
                self.install_view(OpenView::empty());
                self.search_input_dirty = true;
                self.focus_find_dirty = true;
                self.column_layout = crate::column::ColumnLayoutState::new();
                self.selected_row_pos = None;
                self.view_mode = ViewMode::Parsed;
            }
        }

        self.live.follow = true;
        self.live.unseen = 0;
        self.sync_live_sources(cx);
        self.apply_filters_and_refresh(cx);
        if self.view_mode == ViewMode::Raw {
            self.run_raw_search_keep_position(cx);
            self.raw.current = 0;
        }
        match self.focus.active_tab().current {
            // The tab's own current line wins when it lists it.
            Some(_) => self.apply_current_line(cx),
            None => {
                if let Some(pos) = self.selected_row_pos {
                    if pos < self.visible_indices.len() {
                        self.scroll_handle.scroll_to_item(pos, ScrollStrategy::Center);
                    }
                } else if self.active_space_is_live() && self.view_mode == ViewMode::Parsed {
                    // A live Log Space opens with Follow on: at its newest line.
                    self.scroll_handle.scroll_to_bottom();
                }
                self.adopt_view_selection();
            }
        }
    }

    /// Put a built view into the live fields.
    fn install_view(&mut self, open: OpenView) {
        let OpenView { built, filter, focus, source_visible, name } = open;
        self.focus = focus;
        // Search of the open tab shows its Matches as soon as it has a query.
        let has_query = !self.focus.search().query.trim().is_empty();
        self.focus.search_mut().is_open = has_query;
        let fresh = self.column_layout.max_chars == FieldMaxChars::new();
        self.no_timestamp_files = built.no_timestamp_files.iter().cloned().collect();
        // Other rows: a scan of the old ones is stale.
        self.rows_epoch += 1;
        self.all_entries = built.entries;
        self.filter_index = built.index;
        self.filter_state = filter;
        self.filter_ui.picker_open = false;
        self.filter_ui.expanded = None;
        self.filter_ui.inputs_dirty = true;
        self.column_layout.max_chars = built.max_chars;
        self.read_rule_fields();
        self.column_layout.set_source_visible(source_visible);
        self.sync_stream_column();
        if fresh {
            self.column_layout.update_from_entries(&self.all_entries, self.density);
        }
        self.file_name = name;
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
        self.schedule_session_save(cx);
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
            // Closing a tab only removes the view; the Log Space keeps its
            // files, Filter and Markers.
        }
        self.restore_active_session(cx);
        self.schedule_session_save(cx);
    }

    /// Close the active tab, unless it is a Pinned Tab.
    pub fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self
            .active_project()
            .and_then(|p| p.working_set.active_tab_index())
        {
            if self.active_project().is_some_and(|p| p.working_set.is_pinned(idx)) {
                return;
            }
            self.close_tab_at(idx, cx);
        }
    }

    /// Close the unpinned tabs `scope` reaches from the tab at `index`.
    pub fn close_scope_at(&mut self, scope: CloseScope, index: usize, cx: &mut Context<Self>) {
        self.persist_active_session();
        if let Some(proj) = self.active_project_mut() {
            proj.working_set.close_scope(scope, index);
        }
        self.restore_active_session(cx);
        self.schedule_session_save(cx);
    }

    /// Pin or unpin the tab at `index`; the active tab stays the same tab.
    pub fn set_tab_pinned(&mut self, index: usize, pinned: bool, cx: &mut Context<Self>) {
        self.persist_active_session();
        if let Some(proj) = self.active_project_mut() {
            if pinned {
                proj.working_set.pin_tab(index);
            } else {
                proj.working_set.unpin_tab(index);
            }
        }
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// The file a File View tab shows.
    fn tab_file(&self, index: usize) -> Option<LogFile> {
        let proj = self.active_project()?;
        let TabTarget::FileView { space_id, file_id } = &proj.working_set.tabs().get(index)?.target else {
            return None;
        };
        proj.log_space(space_id)?.file(file_id).cloned()
    }

    /// Where a File View tab's file lives, as text for the clipboard: its path
    /// on disk, or `user@host:path` for a Remote File. `relative` shortens a
    /// path under the home folder to `~/…`.
    pub fn tab_file_path(&self, index: usize, relative: bool) -> Option<String> {
        let file = self.tab_file(index)?;
        if let Some(remote) = &file.remote {
            let path = remote.path()?;
            let server = self.remote.servers.get(&remote.server_id)?;
            return Some(format!("{}@{}:{path}", server.user, server.host));
        }
        let path = file.path?;
        if relative {
            if let Some(rest) = crate::workspace::session::home_dir()
                .and_then(|h| std::path::Path::new(&path).strip_prefix(h).ok().map(|r| r.display().to_string()))
            {
                return Some(format!("~/{rest}"));
            }
        }
        Some(path)
    }

    /// Whether the tab's file is on this computer, so Finder can show it.
    pub fn tab_file_is_local(&self, index: usize) -> bool {
        self.tab_file(index).is_some_and(|f| f.remote.is_none() && f.path.is_some())
    }

    pub fn copy_tab_path(&mut self, index: usize, relative: bool, cx: &mut Context<Self>) {
        if let Some(text) = self.tab_file_path(index, relative) {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
    }

    pub fn reveal_tab_file(&mut self, index: usize) {
        if let Some(path) = self.tab_file(index).filter(|f| f.remote.is_none()).and_then(|f| f.path) {
            self.reveal_in_file_manager(&path);
        }
    }

    /// Show the tab's Log Space in the left sidebar, open and expanded.
    pub fn reveal_tab_in_sidebar(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(space_id) = self
            .active_project()
            .and_then(|p| p.working_set.tabs().get(index))
            .and_then(|t| t.target.space_id().map(str::to_string))
        else {
            return;
        };
        self.sidebar_state.visible = true;
        self.expanded_spaces.insert(space_id);
        cx.notify();
    }

    /// The session a newly opened tab of `space` starts with. An Imported
    /// space opens in the Parsed View; a Scratch space opens in the Raw View
    /// while it is empty and otherwise in the view last used.
    fn new_tab_session(&mut self, space: &LogSpace) -> TabSessionState {
        self.ensure_space_loaded(space);
        let mut session = TabSessionState::new();
        session.mode = match space.kind {
            LogSpaceKind::Imported | LogSpaceKind::Remote => ViewMode::Parsed,
            LogSpaceKind::Scratch => {
                let empty = space.files.iter().all(|f| {
                    self.parsed_files
                        .get(&f.file_id)
                        .is_none_or(|p| p.lines.is_empty())
                });
                if empty { ViewMode::Raw } else { space.last_mode }
            }
        };
        session
    }

    /// Open a Log Space in a tab, or focus its tab when it is already open.
    pub fn open_log_space_tab(&mut self, space_id: &str, cx: &mut Context<Self>) {
        let Some(space) = self.active_project().and_then(|p| p.log_space(space_id)).cloned() else {
            return;
        };
        self.persist_active_session();
        let session = self.new_tab_session(&space);
        self.find_filter_off_when_opened_again(&TabTarget::LogSpace { space_id: space_id.to_string() });
        if let Some(proj) = self.active_project_mut() {
            proj.open_log_space(space_id, session);
        }
        self.expanded_spaces.insert(space_id.to_string());
        self.restore_active_session(cx);
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// A view opened again (no tab shows `target` yet): its Find Filter
    /// starts off on every Focus Tab, while the find text is kept.
    fn find_filter_off_when_opened_again(&mut self, target: &TabTarget) {
        let Some(proj) = self.active_project_mut() else { return };
        if proj.working_set.find_tab(target).is_some() {
            return;
        }
        let Some(space) = target.space_id().and_then(|id| proj.log_space_mut(id)) else { return };
        let focus = match target {
            TabTarget::LogSpace { .. } => Some(&mut space.focus),
            TabTarget::FileView { file_id, .. } => space.file_focus.get_mut(file_id),
            TabTarget::InvestigationBoard { .. } => None,
        };
        if let Some(focus) = focus {
            focus.clear_find_filters();
        }
    }

    /// Open one file of a Log Space on its own, or focus its tab.
    pub fn open_file_view_tab(&mut self, space_id: &str, file_id: &str, cx: &mut Context<Self>) {
        let Some(space) = self.active_project().and_then(|p| p.log_space(space_id)).cloned() else {
            return;
        };
        if !space.has_file(file_id) {
            return;
        }
        self.persist_active_session();
        let session = self.new_tab_session(&space);
        self.find_filter_off_when_opened_again(&TabTarget::FileView {
            space_id: space_id.to_string(),
            file_id: file_id.to_string(),
        });
        if let Some(proj) = self.active_project_mut() {
            proj.open_file_view(space_id, file_id, session);
        }
        self.restore_active_session(cx);
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Open a Log Space of another project: switch to that project first.
    pub fn open_space_in_project(&mut self, project_id: &str, space_id: &str, cx: &mut Context<Self>) {
        if self.active_project().map(|p| p.id.as_str()) != Some(project_id) {
            self.switch_project_to(project_id, cx);
        }
        self.open_log_space_tab(space_id, cx);
    }

    /// Open a file of a Log Space of another project.
    pub fn open_file_in_project(
        &mut self,
        project_id: &str,
        space_id: &str,
        file_id: &str,
        cx: &mut Context<Self>,
    ) {
        if self.active_project().map(|p| p.id.as_str()) != Some(project_id) {
            self.switch_project_to(project_id, cx);
        }
        self.open_file_view_tab(space_id, file_id, cx);
    }
}
