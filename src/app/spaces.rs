//! Logic: Log Spaces: the New Log Space dialog, adding, removing and renaming
//! files, ordering, deleting, and Save As.

use super::{AppView, ConfirmAction};
use crate::workspace::log_space::{FileOrder, ImportSource, LogSpaceKind, RemoveFileError, scratch_dir_for};
use crate::workspace::new_space::{self, LiveCounts, NewLogSpaceForm, NewSpaceKind};
use crate::workspace::scratch_store;
use crate::workspace::working_set::ViewMode;
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use std::path::{Path, PathBuf};

/// State of the New Log Space dialog.
pub struct NewSpaceDialog {
    /// The name, and the list of the Static step.
    pub form: NewLogSpaceForm,
    /// The local files of the Live step (only its list is used); its
    /// remote ones are in `RemoteState`.
    pub live_form: NewLogSpaceForm,
    /// The kind of Log Space Create makes; the other kinds keep their lists.
    pub kind: NewSpaceKind,
    /// The path the Local name is suggested from: the first one added while
    /// the list was empty (a folder gives its own name).
    pub local_first: Option<String>,
    /// The name last put in the field by the dialog itself.
    suggested: Option<String>,
    /// Tracks the kind switch, so Tab can stop there.
    pub kind_focus: gpui::FocusHandle,
    /// Created while rendering, where a window is available.
    pub name_input: Option<Entity<InputState>>,
    /// A name to put in the field at the next render: suggested from the
    /// first file added while the field was empty.
    pending_name: Option<String>,
    /// Tracks the dialog's box: the cursor is kept inside it.
    pub focus: Option<gpui::FocusHandle>,
    /// The Starting Time of one chosen Remote File, being changed.
    pub start_edit: Option<StartEdit>,
    _subscription: Option<Subscription>,
}

/// Changing the Starting Time of a Remote File chosen for the New Log Space.
pub struct StartEdit {
    /// `(server id, path)` of the file.
    pub key: (String, String),
    /// `server:path`, for the title.
    pub label: String,
    /// What the field holds.
    pub text: String,
    /// Created while rendering, where a window is available.
    pub input: Option<Entity<InputState>>,
}

impl StartEdit {
    /// Why Save is off, if it is.
    pub fn error(&self) -> Option<String> {
        let text = self.text.trim();
        (!text.is_empty() && !crate::remote::starting_time::is_readable(text))
            .then(|| crate::remote::starting_time::StartError::Invalid.to_string())
    }
}

impl AppView {
    // ── New Log Space dialog ────────────────────────────────────────────────

    pub fn open_new_space_dialog(&mut self, cx: &mut Context<Self>) {
        self.open_new_space_dialog_with(NewLogSpaceForm::new(), NewSpaceKind::Scratch, cx);
    }

    /// Open the dialog on `kind`, with `form` as its list of local files.
    fn open_new_space_dialog_with(&mut self, form: NewLogSpaceForm, kind: NewSpaceKind, cx: &mut Context<Self>) {
        self.remote.new_space_files.clear();
        self.remote.new_space_globs.clear();
        self.remote.new_space_commands.clear();
        self.remote.new_space_starts.clear();
        let local_first = form.files.first().map(|f| match &f.source {
            new_space::PendingSource::Disk(path) => path.clone(),
            new_space::PendingSource::Entry { archive, .. } => archive.clone(),
        });
        self.new_space = Some(NewSpaceDialog {
            form,
            live_form: NewLogSpaceForm::new(),
            kind,
            local_first,
            suggested: None,
            kind_focus: cx.focus_handle().tab_stop(true),
            name_input: None,
            pending_name: None,
            focus: None,
            start_edit: None,
            _subscription: None,
        });
        self.refresh_suggested_name();
        self.is_palette_open = false;
        cx.notify();
    }

    /// Switch the kind of Log Space to create. Only the kind changes: no list
    /// is emptied, and a name the user typed stays.
    pub fn new_space_set_kind(&mut self, kind: NewSpaceKind, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_space.as_mut() else { return };
        if dialog.kind == kind {
            return;
        }
        dialog.kind = kind;
        self.refresh_suggested_name();
        cx.notify();
    }

    /// Step the kind switch by `delta`, stopping at its ends.
    pub fn new_space_step_kind(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_space.as_ref() else { return };
        let next = dialog.kind.index().saturating_add_signed(delta);
        if let Some(kind) = NewSpaceKind::from_index(next) {
            self.new_space_set_kind(kind, cx);
        }
    }

    /// The first item of the list the dialog shows, for the suggested name.
    fn first_item_of_shown_list(&self) -> Option<String> {
        let dialog = self.new_space.as_ref()?;
        match dialog.kind {
            NewSpaceKind::Scratch => None,
            NewSpaceKind::Static => dialog.local_first.clone(),
            // A local file first: it is listed above the remote ones.
            NewSpaceKind::Live => dialog
                .live_form
                .files
                .first()
                .and_then(|f| match &f.source {
                    new_space::PendingSource::Disk(path) => Some(path.clone()),
                    new_space::PendingSource::Entry { .. } => None,
                })
                .or_else(|| self.remote.new_space_files.first().map(|f| f.2.clone()))
                .or_else(|| self.remote.new_space_globs.first().map(|g| g.2.clone())),
        }
    }

    /// Put the suggested name for the kind showing in the field, unless the
    /// user typed one: the field is empty or still holds the last suggestion.
    pub(crate) fn refresh_suggested_name(&mut self) {
        let first = self.first_item_of_shown_list();
        let Some(project) = self.workspace_state.active_project() else { return };
        let Some(dialog) = self.new_space.as_ref() else { return };
        let name = dialog.form.name.trim();
        let untouched = name.is_empty() || dialog.suggested.as_deref() == Some(name);
        if !untouched {
            return;
        }
        let suggestion = new_space::suggested_name(dialog.kind, first.as_deref(), project);
        if let Some(dialog) = self.new_space.as_mut() {
            let text = suggestion.clone().unwrap_or_default();
            dialog.form.set_name(&text);
            dialog.pending_name = Some(text);
            dialog.suggested = suggestion;
        }
    }

    /// Create the name field of a freshly opened dialog and focus it, and
    /// put a suggested name in it.
    pub fn sync_new_space_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_space.as_mut() else { return };
        if let Some(edit) = dialog.start_edit.as_mut().filter(|e| e.input.is_none()) {
            let text = edit.text.clone();
            let input = cx.new(|cx| {
                let mut state = InputState::new(window, cx).placeholder("1h, 30m, 2d, 10:42, 2026-10-19 10:00");
                state.set_value(text, window, cx);
                state
            });
            cx.subscribe_in(&input, window, |this: &mut Self, input, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => {
                    let text = input.read(cx).value().to_string();
                    if let Some(edit) = this.new_space.as_mut().and_then(|d| d.start_edit.as_mut()) {
                        edit.text = text;
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => this.new_space_save_start(cx),
                _ => {}
            })
            .detach();
            input.update(cx, |state, cx| state.focus(window, cx));
            edit.input = Some(input);
        }
        if let Some(input) = dialog.name_input.clone() {
            if let Some(name) = dialog.pending_name.take() {
                input.update(cx, |state, cx| state.set_value(name, window, cx));
            }
            // The cursor stays in the dialog: back from the remote picker or
            // the Servers dialog, whose fields took it with them, or tabbed
            // past the last button towards the view behind, it returns to the
            // name, where Enter creates the Log Space.
            let covered = self.remote.picker.is_some()
                || self.server_dialog.is_some()
                || self.confirm.is_some()
                || dialog.start_edit.is_some();
            let inside = dialog.focus.as_ref().is_some_and(|f| f.contains_focused(window, cx));
            if !covered && !inside {
                input.update(cx, |state, cx| state.focus(window, cx));
            }
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Log Space name"));
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, input, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => {
                    let name = input.read(cx).value().to_string();
                    if let Some(dialog) = this.new_space.as_mut() {
                        dialog.form.set_name(&name);
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => this.new_space_create(cx),
                _ => {}
            },
        );
        if let Some(name) = dialog.pending_name.take() {
            input.update(cx, |state, cx| state.set_value(name, window, cx));
        }
        input.update(cx, |state, cx| state.focus(window, cx));
        dialog.name_input = Some(input);
        dialog.focus = Some(cx.focus_handle());
        dialog._subscription = Some(subscription);
    }

    pub fn new_space_cancel(&mut self, cx: &mut Context<Self>) {
        self.new_space = None;
        self.remote.new_space_files.clear();
        self.remote.new_space_globs.clear();
        self.remote.new_space_commands.clear();
        self.remote.new_space_starts.clear();
        cx.notify();
    }

    /// Add dropped or picked paths to the dialog's list.
    pub fn new_space_add_paths(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_space.as_mut() else { return };
        // Files dropped or chosen are local files, whatever list was showing.
        dialog.kind = NewSpaceKind::Static;
        if dialog.form.files.is_empty() {
            dialog.local_first = paths.first().cloned();
        }
        let archives = dialog.form.begin_paths(&paths);
        self.refresh_suggested_name();
        cx.notify();
        // An Archive is listed in the background: it may be unpacked whole.
        for (archive, kind) in archives {
            cx.spawn(async move |this, cx| {
                let path = archive.clone();
                let items = cx
                    .background_executor()
                    .spawn(async move { new_space::list_archive(&path, kind) })
                    .await;
                this.update(cx, |this, cx| {
                    if let Some(dialog) = this.new_space.as_mut() {
                        dialog.form.finish_archive(&archive, items);
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }
    }

    /// Files or folders dropped anywhere on the window: with the New Log
    /// Space dialog open they go to its list of local files; with nothing
    /// open they make an Imported Log Space at once. Ignored while another
    /// dialog is open.
    pub fn drop_paths_on_window(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let other_dialog = self.confirm.is_some()
            || self.server_dialog.is_some()
            || self.format_editor.is_some()
            || self.custom_load.is_some()
            || self.project_switcher_open
            || self.remote.picker.is_some()
            || self.remote.download.is_some()
            || self.remote.download_options.is_some()
            || self.remote.full_dialog.is_some()
            || self.remote.secret_prompt.is_some()
            || self.remote.sign_in_dialog.is_some()
            || !self.remote.host_prompts.is_empty();
        if paths.is_empty() || other_dialog {
            return;
        }
        if self.new_space.is_some() {
            self.new_space_add_paths(paths, cx);
        } else {
            self.drop_create_imported(paths, cx);
        }
    }

    /// Make an Imported Log Space of dropped paths, named after the first.
    /// Archives are read first ("Reading archive…" in the status bar). When
    /// some item cannot be added, or there is none, nothing is made: the
    /// dialog opens on Static with the list and each file's warning.
    fn drop_create_imported(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let mut form = NewLogSpaceForm::new();
        let archives = form.begin_paths(&paths);
        let first = paths.first().cloned();
        if archives.is_empty() {
            self.finish_drop(form, first, cx);
            return;
        }
        self.dropped_archives_reading += 1;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let items = cx
                .background_executor()
                .spawn(async move {
                    archives
                        .into_iter()
                        .map(|(path, kind)| {
                            let items = new_space::list_archive(&path, kind);
                            (path, items)
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            this.update(cx, |this, cx| {
                for (path, items) in items {
                    form.finish_archive(&path, items);
                }
                this.dropped_archives_reading = this.dropped_archives_reading.saturating_sub(1);
                this.finish_drop(form, first, cx);
            })
            .ok();
        })
        .detach();
    }

    /// A drop whose Archives are all read: create, or show what went wrong.
    fn finish_drop(&mut self, form: NewLogSpaceForm, first: Option<String>, cx: &mut Context<Self>) {
        let usable = !form.files.is_empty() && form.files.iter().all(|f| f.is_importable());
        let name = first
            .and_then(|p| new_space::suggest_name(&p, crate::local_path::PathStyle::native()))
            .zip(self.workspace_state.active_project())
            .map(|(name, project)| new_space::unique_name(&name, project));
        match (usable, name) {
            (true, Some(name)) if self.new_space.is_none() => {
                let sources = form.importable_sources();
                self.create_log_space(&name, LogSpaceKind::Static, &sources, cx);
            }
            _ if self.new_space.is_none() => {
                self.open_new_space_dialog_with(form, NewSpaceKind::Static, cx);
            }
            // A dialog opened while the Archive was read: the files go to it.
            _ => {
                let paths: Vec<String> = form
                    .files
                    .iter()
                    .map(|f| match &f.source {
                        new_space::PendingSource::Disk(p) => p.clone(),
                        new_space::PendingSource::Entry { archive, .. } => archive.clone(),
                    })
                    .collect();
                self.new_space_add_paths(paths, cx);
            }
        }
        cx.notify();
    }

    /// Change the Starting Time of the chosen Remote File at `index`.
    pub fn new_space_edit_start(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some((server_id, server, path)) = self.remote.new_space_files.get(index).cloned() else { return };
        let key = (server_id, path.clone());
        let text = self.remote.new_space_starts.get(&key).cloned().unwrap_or_default();
        if let Some(dialog) = self.new_space.as_mut() {
            dialog.start_edit = Some(StartEdit { key, label: format!("{server}:{path}"), text, input: None });
            cx.notify();
        }
    }

    /// Save: the file keeps the Starting Time typed; an empty field reads
    /// its last lines again.
    pub fn new_space_save_start(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_space.as_mut() else { return };
        let Some(edit) = dialog.start_edit.as_ref() else { return };
        if edit.error().is_some() {
            return;
        }
        let text = edit.text.trim().to_string();
        let key = edit.key.clone();
        dialog.start_edit = None;
        if text.is_empty() {
            self.remote.new_space_starts.remove(&key);
        } else {
            self.remote.new_space_starts.insert(key, text);
        }
        cx.notify();
    }

    pub fn new_space_cancel_start(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.new_space.as_mut() {
            dialog.start_edit = None;
            cx.notify();
        }
    }

    /// What the Live step lists, counted for Create.
    pub fn new_space_live_counts(&self) -> LiveCounts {
        LiveCounts {
            local: self.new_space.as_ref().map_or(0, |d| d.live_form.importable_sources().len()),
            files: self.remote.new_space_files.len(),
            globs: self.remote.new_space_globs.len(),
            commands: self.remote.new_space_commands.len(),
        }
    }

    /// Add dropped or picked paths to the local files of the Live step.
    pub fn new_space_add_live_paths(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_space.as_mut() else { return };
        dialog.live_form.add_live_paths(&paths);
        self.refresh_suggested_name();
        cx.notify();
    }

    pub fn new_space_remove_live_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(dialog) = self.new_space.as_mut() {
            dialog.live_form.remove_file(index);
            self.refresh_suggested_name();
            cx.notify();
        }
    }

    /// "Choose files…" of the Live step: the files go to its local list.
    pub fn new_space_browse_live(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let Some(files) = rfd::AsyncFileDialog::new().pick_files().await else { return };
            let paths: Vec<String> = files.iter().map(|f| f.path().to_string_lossy().to_string()).collect();
            this.update(cx, |this, cx| this.new_space_add_live_paths(paths, cx)).ok();
        })
        .detach();
    }

    pub fn new_space_remove_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(dialog) = self.new_space.as_mut() {
            dialog.form.remove_file(index);
            if dialog.form.files.is_empty() {
                dialog.local_first = None;
            }
            cx.notify();
        }
    }

    /// Pick files, or folders when `folders`, with the native dialog
    /// (awaited off the event handler). The dialog cannot offer both at once.
    pub fn new_space_browse(&mut self, folders: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let dialog = rfd::AsyncFileDialog::new();
            let picked = if folders { dialog.pick_folders().await } else { dialog.pick_files().await };
            let Some(files) = picked else {
                return;
            };
            let paths: Vec<String> = files
                .iter()
                .map(|f| f.path().to_string_lossy().to_string())
                .collect();
            this.update(cx, |this, cx| this.new_space_add_paths(paths, cx)).ok();
        })
        .detach();
    }

    /// Create the Log Space of the kind the dialog shows, from that kind's
    /// list only: the lists of the other kinds are dropped.
    pub fn new_space_create(&mut self, cx: &mut Context<Self>) {
        let live_items = self.new_space_live_counts().total();
        let Some(dialog) = self.new_space.as_ref() else { return };
        let Some(project) = self.active_project() else { return };
        let kind = dialog.kind;
        if !dialog.form.can_create(kind, live_items, project) {
            return;
        }
        let name = dialog.form.name.clone();
        match kind {
            NewSpaceKind::Live => {
                let files = std::mem::take(&mut self.remote.new_space_files);
                let globs = std::mem::take(&mut self.remote.new_space_globs);
                let commands = std::mem::take(&mut self.remote.new_space_commands);
                let starts = std::mem::take(&mut self.remote.new_space_starts);
                let local = dialog.live_form.importable_sources();
                self.new_space = None;
                let space_id = self.create_live_log_space(&name, &local, files, globs, commands, cx);
                let space = space_id.and_then(|id| self.active_project_mut()?.log_space_mut(&id));
                if let Some(space) = space {
                    for ((server_id, path), start) in starts {
                        if let Some(id) = space.remote_file_id(&server_id, &path) {
                            space.set_start_time(&id, Some(start));
                        }
                    }
                }
            }
            NewSpaceKind::Static => {
                let sources = dialog.form.importable_sources();
                self.new_space = None;
                self.create_log_space(&name, LogSpaceKind::Static, &sources, cx);
            }
            NewSpaceKind::Scratch => {
                self.new_space = None;
                self.create_log_space(&name, LogSpaceKind::Scratch, &[], cx);
            }
        }
    }

    /// Create a Remote Log Space with files on Servers, given as
    /// `(server id, server name, path)`, Live Globs, given as `(server id,
    /// server name, folder, pattern)`, and commands, given as `(server id,
    /// server name, command, preset)`, and open it.
    pub fn create_remote_log_space(
        &mut self,
        name: &str,
        files: Vec<(String, String, String)>,
        globs: Vec<(String, String, String, String)>,
        commands: Vec<(String, String, String, Option<crate::remote::PresetFields>)>,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        self.create_live_log_space(name, &[], files, globs, commands, cx)
    }

    /// Create a Live Log Space with local files (`local`) as well as files,
    /// Live Globs and Remote Commands on Servers, as `create_remote_log_space`
    /// takes them, and open it.
    pub fn create_live_log_space(
        &mut self,
        name: &str,
        local: &[ImportSource],
        files: Vec<(String, String, String)>,
        globs: Vec<(String, String, String, String)>,
        commands: Vec<(String, String, String, Option<crate::remote::PresetFields>)>,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        self.persist_active_session();
        let space_id = {
            let project = self.active_project_mut()?;
            let space = match project.create_log_space(name, LogSpaceKind::Live) {
                Ok(space) => space,
                Err(e) => {
                    self.notice = Some(e.message().to_string());
                    cx.notify();
                    return None;
                }
            };
            let _ = space.add_remote_files(&files);
            for (server_id, _, dir, pattern) in &globs {
                let _ = space.add_live_glob(server_id, dir, pattern);
            }
            let _ = space.add_remote_commands(&commands);
            let _ = space.add_imported_sources(local);
            space.id.clone()
        };
        self.expanded_spaces.insert(space_id.clone());
        self.open_log_space_tab(&space_id, cx);
        self.sync_glob_watchers(cx);
        Some(space_id)
    }

    /// Create a Log Space in the active project and open it.
    pub fn create_log_space(
        &mut self,
        name: &str,
        kind: LogSpaceKind,
        sources: &[ImportSource],
        cx: &mut Context<Self>,
    ) -> Option<String> {
        self.persist_active_session();
        let mut skipped = Vec::new();
        let mut scratch_path: Option<PathBuf> = None;
        let space_id = {
            let project = self.active_project_mut()?;
            let space = match project.create_log_space(name, kind) {
                Ok(space) => space,
                Err(e) => {
                    self.notice = Some(e.message().to_string());
                    cx.notify();
                    return None;
                }
            };
            match kind {
                LogSpaceKind::Static | LogSpaceKind::Live => {
                    if let Ok(outcome) = space.add_imported_sources(sources) {
                        skipped = outcome.skipped;
                    }
                }
                LogSpaceKind::Scratch => {
                    let dir = scratch_dir_for(&space.id);
                    if let Ok(file) = space.new_scratch_file(&dir) {
                        scratch_path = file.path.as_deref().map(PathBuf::from);
                    }
                }
            }
            space.id.clone()
        };
        if let Some(path) = scratch_path {
            if let Err(e) = scratch_store::create_file(&path) {
                self.notice = Some(format!("Could not create {}: {e}", path.display()));
            }
        }
        if !skipped.is_empty() {
            self.notice = Some(format!("{} file(s) were already added", skipped.len()));
        }
        self.expanded_spaces.insert(space_id.clone());
        self.open_log_space_tab(&space_id, cx);
        Some(space_id)
    }

    /// Load the demo: a Scratch Log Space with sample log lines.
    pub fn load_demo(&mut self, cx: &mut Context<Self>) {
        let mut name = "Demo".to_string();
        let mut n = 2;
        while self.active_project().is_some_and(|p| p.has_space_named(&name)) {
            name = format!("Demo {n}");
            n += 1;
        }
        let Some(space_id) = self.create_log_space(&name, LogSpaceKind::Scratch, &[], cx) else {
            return;
        };
        let text = crate::sample::generate_sample_text();
        let file = self
            .active_project()
            .and_then(|p| p.log_space(&space_id))
            .and_then(|s| s.files.first())
            .map(|f| (f.file_id.clone(), f.path.clone()));
        if let Some((file_id, Some(path))) = file {
            if scratch_store::save_text(Path::new(&path), &text).is_ok() {
                self.forget_file(&file_id);
                self.set_view_mode_for_new_content(&space_id, cx);
            }
        }
    }

    /// After a space got its content, show it parsed.
    fn set_view_mode_for_new_content(&mut self, _space_id: &str, cx: &mut Context<Self>) {
        self.persist_active_session();
        if let Some(tab) = self
            .active_project_mut()
            .and_then(|p| p.working_set.active_tab_mut())
        {
            tab.session.mode = ViewMode::Parsed;
        }
        self.restore_active_session(cx);
        cx.notify();
    }

    // ── Changing a Log Space ────────────────────────────────────────────────

    /// Rebuild the open view after the files of a space changed.
    pub(crate) fn refresh_after_space_change(&mut self, cx: &mut Context<Self>) {
        self.restore_active_session(cx);
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn toggle_space_expand(&mut self, space_id: &str, cx: &mut Context<Self>) {
        if !self.expanded_spaces.remove(space_id) {
            self.expanded_spaces.insert(space_id.to_string());
        }
        cx.notify();
    }

    pub fn rename_log_space(
        &mut self,
        project_id: &str,
        space_id: &str,
        name: &str,
        cx: &mut Context<Self>,
    ) {
        let result = self
            .workspace_state
            .project_mut(project_id)
            .map(|p| p.rename_log_space(space_id, name));
        if let Some(Err(e)) = result {
            self.notice = Some(e.message().to_string());
        }
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn set_file_order(&mut self, space_id: &str, order: FileOrder, cx: &mut Context<Self>) {
        self.persist_active_session();
        if let Some(space) = self.active_project_mut().and_then(|p| p.log_space_mut(space_id)) {
            space.set_file_order(order);
        }
        self.refresh_after_space_change(cx);
    }

    /// "Add files…" or "Add folder…" (`folders`): pick files or folders for
    /// a Static or Live Log Space.
    pub fn add_files_dialog(&mut self, space_id: &str, folders: bool, cx: &mut Context<Self>) {
        let space_id = space_id.to_string();
        cx.spawn(async move |this, cx| {
            let dialog = rfd::AsyncFileDialog::new();
            let picked = if folders { dialog.pick_folders().await } else { dialog.pick_files().await };
            let Some(files) = picked else {
                return;
            };
            let paths: Vec<String> = files
                .iter()
                .map(|f| f.path().to_string_lossy().to_string())
                .collect();
            this.update(cx, |this, cx| this.add_files_to_space(&space_id, paths, cx)).ok();
        })
        .detach();
    }

    /// Add files to a Static or Live Log Space. Folders are opened one level,
    /// Archives are opened into their entries (in the background), binary
    /// files are left out, and files already in the space are skipped.
    pub fn add_files_to_space(&mut self, space_id: &str, paths: Vec<String>, cx: &mut Context<Self>) {
        if paths.iter().any(|p| crate::archive::detect(Path::new(p)).is_some()) {
            self.notice = Some("Reading archive…".to_string());
            cx.notify();
        }
        let space_id = space_id.to_string();
        cx.spawn(async move |this, cx| {
            let items = cx
                .background_executor()
                .spawn(async move { new_space::resolve_paths(&paths) })
                .await;
            this.update(cx, |this, cx| this.add_resolved_to_space(&space_id, items, cx)).ok();
        })
        .detach();
    }

    fn add_resolved_to_space(&mut self, space_id: &str, items: Vec<new_space::PendingFile>, cx: &mut Context<Self>) {
        self.persist_active_session();
        let mut form = NewLogSpaceForm::new();
        form.files = items;
        let left_out = form.files.iter().filter(|f| !f.is_importable()).count();
        let mut messages = Vec::new();
        if let Some(message) = form.size_limit_message() {
            self.notice = Some(message);
            cx.notify();
            return;
        }
        let outcome = self
            .active_project_mut()
            .and_then(|p| p.log_space_mut(space_id))
            .and_then(|s| s.add_imported_sources(&form.importable_sources()).ok());
        if let Some(outcome) = &outcome {
            if !outcome.skipped.is_empty() {
                messages.push(format!("{} file(s) already in this Log Space", outcome.skipped.len()));
            }
            if !outcome.static_only.is_empty() {
                messages.push(format!("{} archive file(s) can only be added to a Static Log Space", outcome.static_only.len()));
            }
        }
        if left_out > 0 {
            messages.push(format!("{left_out} file(s) are not readable text and were left out"));
        }
        self.notice = if messages.is_empty() { None } else { Some(messages.join("; ")) };
        self.refresh_after_space_change(cx);
    }

    /// "New scratch file": add an empty `untitled-N.log` to a Scratch space.
    pub fn new_scratch_file_in(&mut self, space_id: &str, cx: &mut Context<Self>) {
        self.persist_active_session();
        let path = self
            .active_project_mut()
            .and_then(|p| p.log_space_mut(space_id))
            .and_then(|space| {
                let dir = scratch_dir_for(&space.id);
                space.new_scratch_file(&dir).ok().and_then(|f| f.path.clone())
            });
        if let Some(path) = path {
            if let Err(e) = scratch_store::create_file(Path::new(&path)) {
                self.notice = Some(format!("Could not create {path}: {e}"));
            }
        }
        self.refresh_after_space_change(cx);
    }

    /// "Remove from Log Space" on an Imported file: only the list changes.
    pub fn remove_file_from_space(&mut self, space_id: &str, file_id: &str, cx: &mut Context<Self>) {
        self.persist_active_session();
        let removed = self
            .active_project_mut()
            .and_then(|p| p.remove_file(space_id, file_id).ok());
        if let Some(file) = &removed {
            self.delete_copy_of(file);
            self.forget_file(file_id);
        }
        self.refresh_after_space_change(cx);
    }

    /// Rename a scratch file (also on disk). Imported files are never renamed.
    pub fn rename_scratch_file(
        &mut self,
        project_id: &str,
        space_id: &str,
        file_id: &str,
        name: &str,
        cx: &mut Context<Self>,
    ) {
        let name = name.trim();
        let valid = !name.is_empty() && !name.contains(['/', '\\']) && name != "." && name != "..";
        let Some(space) = self
            .workspace_state
            .project_mut(project_id)
            .and_then(|p| p.log_space_mut(space_id))
        else {
            return;
        };
        if space.kind != LogSpaceKind::Scratch {
            return;
        }
        if !valid || !space.rename_file(file_id, name) {
            self.notice = Some("That name cannot be used".to_string());
            cx.notify();
            return;
        }
        let dir = scratch_dir_for(space_id);
        let new_path = dir.join(name);
        let old_path = space.file(file_id).and_then(|f| f.path.clone());
        if let Some(file) = space.file_mut(file_id) {
            file.path = Some(new_path.to_string_lossy().to_string());
        }
        if let Some(old) = old_path {
            if let Err(e) = scratch_store::rename_file(Path::new(&old), &new_path) {
                self.notice = Some(format!("Could not rename the file: {e}"));
            }
        }
        self.persist_active_session();
        self.refresh_after_space_change(cx);
    }

    /// Save As: write a copy of a scratch file to a place the user picks.
    pub fn save_as_dialog(&mut self, space_id: &str, file_id: &str, cx: &mut Context<Self>) {
        self.flush_scratch_saves();
        let Some(file) = self
            .active_project()
            .and_then(|p| p.file(space_id, file_id))
            .cloned()
        else {
            return;
        };
        let text = match self.raw.texts.get(file_id) {
            Some(text) => text.clone(),
            None => file
                .path
                .as_deref()
                .and_then(|p| scratch_store::read_text(Path::new(p)).ok())
                .unwrap_or_default(),
        };
        let default_name = file.display_name.clone();
        cx.spawn(async move |this, cx| {
            let Some(handle) = rfd::AsyncFileDialog::new()
                .set_file_name(&default_name)
                .save_file()
                .await
            else {
                return;
            };
            let result = scratch_store::save_as(&text, handle.path());
            this.update(cx, |this, cx| {
                this.notice = Some(match result {
                    Ok(()) => format!("Saved a copy to {}", handle.path().display()),
                    Err(e) => format!("Could not save: {e}"),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Show a file, selected, in the system file manager.
    pub fn reveal_in_file_manager(&mut self, path: &str) {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            // Explorer reads `/select,` and the path as one argument and takes
            // quotes only around the path, so the argument is written as is.
            let path = path.replace('/', "\\");
            let _ = std::process::Command::new("explorer").raw_arg(format!("/select,\"{path}\"")).spawn();
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let path = std::path::PathBuf::from(path);
            // Ask the file manager to select the file (Nautilus, Dolphin, Nemo…
            // answer on D-Bus); open its folder when none does.
            std::thread::spawn(move || {
                let uri = file_uri(&path);
                let shown = std::process::Command::new("dbus-send")
                    .args([
                        "--session",
                        "--print-reply",
                        "--dest=org.freedesktop.FileManager1",
                        "/org/freedesktop/FileManager1",
                        "org.freedesktop.FileManager1.ShowItems",
                    ])
                    .arg(format!("array:string:{uri}"))
                    .arg("string:")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success());
                if !shown {
                    if let Some(dir) = path.parent() {
                        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
                    }
                }
            });
        }
    }

    // ── Deleting ────────────────────────────────────────────────────────────

    pub fn request_delete_log_space(&mut self, space_id: &str, cx: &mut Context<Self>) {
        self.confirm = Some(ConfirmAction::DeleteLogSpace { space_id: space_id.to_string() });
        cx.notify();
    }

    pub fn request_delete_scratch_file(
        &mut self,
        space_id: &str,
        file_id: &str,
        cx: &mut Context<Self>,
    ) {
        let last = self
            .active_project()
            .and_then(|p| p.log_space(space_id))
            .is_some_and(|s| s.files.len() <= 1);
        if last {
            return;
        }
        self.confirm = Some(ConfirmAction::DeleteScratchFile {
            space_id: space_id.to_string(),
            file_id: file_id.to_string(),
        });
        cx.notify();
    }

    pub fn cancel_confirm(&mut self, cx: &mut Context<Self>) {
        self.confirm = None;
        cx.notify();
    }

    /// Carry out the confirmed deletion.
    pub fn accept_confirm(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.confirm.take() else { return };
        match &action {
            ConfirmAction::ClearFiles { file_ids, .. } => {
                self.clear_files_now(file_ids, cx);
                return;
            }
            ConfirmAction::DeleteServer { server_id } => {
                self.delete_server_from_dialog(server_id, cx);
                return;
            }
            ConfirmAction::RemoveProject { project_id } => {
                self.remove_project_at(project_id, cx);
                cx.notify();
                return;
            }
            ConfirmAction::DeleteLogFormat { format_id } => {
                self.delete_log_format(format_id, cx);
                return;
            }
            _ => {}
        }
        self.persist_active_session();
        match action {
            ConfirmAction::ClearFiles { .. }
            | ConfirmAction::DeleteServer { .. }
            | ConfirmAction::RemoveProject { .. }
            | ConfirmAction::DeleteLogFormat { .. } => {}
            ConfirmAction::DeleteLogSpace { space_id } => {
                let removed = self
                    .active_project_mut()
                    .and_then(|p| p.remove_log_space(&space_id));
                if let Some(space) = removed {
                    for file in &space.files {
                        self.delete_copy_of(file);
                        self.forget_file(&file.file_id);
                    }
                    // A Scratch space owns its folder; an Imported one owns nothing on disk.
                    if space.kind == LogSpaceKind::Scratch {
                        let _ = scratch_store::delete_space_dir(&scratch_dir_for(&space.id));
                    }
                    self.expanded_spaces.remove(&space.id);
                }
            }
            ConfirmAction::DeleteScratchFile { space_id, file_id } => {
                let removed = self
                    .active_project_mut()
                    .and_then(|p| p.remove_file(&space_id, &file_id).ok());
                match removed {
                    Some(file) => {
                        if let Some(path) = &file.path {
                            let _ = scratch_store::delete_file(Path::new(path));
                        }
                        self.forget_file(&file_id);
                    }
                    None => {
                        let last = matches!(
                            self.active_project_mut()
                                .and_then(|p| p.log_space_mut(&space_id))
                                .map(|s| s.files.len()),
                            Some(1)
                        );
                        if last {
                            self.notice = Some("A scratch Log Space keeps at least one file".to_string());
                        }
                    }
                }
            }
        }
        self.refresh_after_space_change(cx);
    }

    // ── Quick Settings ──────────────────────────────────────────────────────

    /// Whether the open Log Space has a Remote Command (so its lines have a
    /// Stream and the Stream Column can be shown).
    pub fn active_space_has_command(&self) -> bool {
        self.active_space().is_some_and(|s| s.files.iter().any(|f| f.is_remote_command()))
    }

    /// Whether the Stream Column is laid out in the open view: the Canvas
    /// has it whenever the open Log Space has a Remote Command.
    pub fn stream_column_visible(&self) -> bool {
        self.active_space_has_command()
    }

    /// Show or hide the Stream Column to match the open view.
    pub(crate) fn sync_stream_column(&mut self) {
        let visible = self.stream_column_visible();
        if self.column_layout.stream_visible != visible {
            self.column_layout.set_stream_visible(visible);
            if visible {
                self.column_layout.max_chars.stream = self.column_layout.max_chars.stream.max(6);
                self.column_layout.auto_fit(crate::column::PinnableColumn::Stream, self.text_scale());
            }
        }
    }

    /// Why a scratch file cannot be deleted, if so.
    pub fn can_delete_scratch_file(&self, space_id: &str) -> bool {
        self.active_project()
            .and_then(|p| p.log_space(space_id))
            .is_some_and(|s| s.files.len() > 1)
    }

    /// Used by tests and menus: whether removing would be refused.
    pub fn remove_error_for(&self, space_id: &str, file_id: &str) -> Option<RemoveFileError> {
        let space = self.active_project()?.log_space(space_id)?;
        if !space.has_file(file_id) {
            Some(RemoveFileError::NotFound)
        } else if space.kind == LogSpaceKind::Scratch && space.files.len() == 1 {
            Some(RemoveFileError::LastScratchFile)
        } else {
            None
        }
    }

    /// Dismiss the notice.
    pub fn dismiss_notice(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        cx.notify();
    }
}

/// `path` as a `file://` URI, every byte outside the unreserved set escaped.
#[cfg(not(any(target_os = "macos", windows)))]
fn file_uri(path: &std::path::Path) -> String {
    use std::os::unix::ffi::OsStrExt as _;
    let mut uri = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => uri.push(b as char),
            _ => uri.push_str(&format!("%{b:02X}")),
        }
    }
    uri
}
