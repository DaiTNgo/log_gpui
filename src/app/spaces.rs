//! Logic: Log Spaces: the New Log Space dialog, adding, removing and renaming
//! files, ordering, deleting, and Save As.

use super::{AppView, ConfirmAction};
use crate::workspace::log_space::{FileOrder, ImportSource, LogSpaceKind, RemoveFileError, scratch_dir_for};
use crate::workspace::new_space::{self, NewLogSpaceForm};
use crate::workspace::scratch_store;
use crate::workspace::working_set::ViewMode;
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use std::path::{Path, PathBuf};

/// State of the New Log Space dialog.
pub struct NewSpaceDialog {
    pub form: NewLogSpaceForm,
    /// Created while rendering, where a window is available.
    pub name_input: Option<Entity<InputState>>,
    _subscription: Option<Subscription>,
}

impl AppView {
    // ── New Log Space dialog ────────────────────────────────────────────────

    pub fn open_new_space_dialog(&mut self, cx: &mut Context<Self>) {
        self.remote.new_space_files.clear();
        self.new_space = Some(NewSpaceDialog {
            form: NewLogSpaceForm::new(),
            name_input: None,
            _subscription: None,
        });
        self.is_palette_open = false;
        cx.notify();
    }

    /// Create the name field of a freshly opened dialog and focus it.
    pub fn sync_new_space_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_space.as_mut() else { return };
        if dialog.name_input.is_some() {
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
        input.update(cx, |state, cx| state.focus(window, cx));
        dialog.name_input = Some(input);
        dialog._subscription = Some(subscription);
    }

    pub fn new_space_cancel(&mut self, cx: &mut Context<Self>) {
        self.new_space = None;
        self.remote.new_space_files.clear();
        cx.notify();
    }

    /// Add dropped or picked paths to the dialog's list.
    pub fn new_space_add_paths(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_space.as_mut() else { return };
        let archives = dialog.form.begin_paths(&paths);
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

    pub fn new_space_remove_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(dialog) = self.new_space.as_mut() {
            dialog.form.remove_file(index);
            cx.notify();
        }
    }

    /// Pick files with the native dialog (awaited off the event handler).
    pub fn new_space_browse(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let Some(files) = rfd::AsyncFileDialog::new().pick_files().await else {
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

    /// Create the Log Space the dialog describes: Scratch when no file was
    /// added, Imported otherwise.
    pub fn new_space_create(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.new_space.as_ref() else { return };
        let Some(project) = self.active_project() else { return };
        if !dialog.form.can_create(project) {
            return;
        }
        let name = dialog.form.name.clone();
        if !self.remote.new_space_files.is_empty() {
            let files = std::mem::take(&mut self.remote.new_space_files);
            self.new_space = None;
            self.create_remote_log_space(&name, files, cx);
            return;
        }
        let sources = dialog.form.importable_sources();
        let kind = dialog.form.kind();
        self.new_space = None;
        self.create_log_space(&name, kind, &sources, cx);
    }

    /// Create a Remote Log Space with files on Servers, given as
    /// `(server id, server name, path)`, and open it.
    pub fn create_remote_log_space(
        &mut self,
        name: &str,
        files: Vec<(String, String, String)>,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        self.persist_active_session();
        let space_id = {
            let project = self.active_project_mut()?;
            let space = match project.create_log_space(name, LogSpaceKind::Remote) {
                Ok(space) => space,
                Err(e) => {
                    self.notice = Some(e.message().to_string());
                    cx.notify();
                    return None;
                }
            };
            let _ = space.add_remote_files(&files);
            space.id.clone()
        };
        self.expanded_spaces.insert(space_id.clone());
        self.open_log_space_tab(&space_id, cx);
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
                LogSpaceKind::Imported => {
                    if let Ok(outcome) = space.add_imported_sources(sources) {
                        skipped = outcome.skipped;
                    }
                }
                LogSpaceKind::Remote => {}
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

    /// "Add files…": pick files for an Imported Log Space.
    pub fn add_files_dialog(&mut self, space_id: &str, cx: &mut Context<Self>) {
        let space_id = space_id.to_string();
        cx.spawn(async move |this, cx| {
            let Some(files) = rfd::AsyncFileDialog::new().pick_files().await else {
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

    /// Add files to an Imported Log Space. Folders are opened one level,
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
        if removed.is_some() {
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
        self.persist_active_session();
        match action {
            ConfirmAction::DeleteLogSpace { space_id } => {
                let removed = self
                    .active_project_mut()
                    .and_then(|p| p.remove_log_space(&space_id));
                if let Some(space) = removed {
                    for file in &space.files {
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

    /// Whether the Source Column is shown in the focused Log Space.
    pub fn source_column_shown(&self) -> bool {
        self.column_layout.source_visible
    }

    /// The "Show source file" switch of Settings; one choice for every Log Space.
    pub fn set_show_source(&mut self, shown: bool, cx: &mut Context<Self>) {
        if !self.active_is_log_space() {
            return;
        }
        self.workspace_state.show_source = Some(shown);
        self.column_layout.set_source_visible(shown);
        if shown {
            self.column_layout
                .auto_fit(crate::column::PinnableColumn::Source, self.density);
        }
        self.schedule_session_save(cx);
        cx.notify();
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
