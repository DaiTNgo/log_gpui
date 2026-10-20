//! Logic: choosing files on a Server: browse folders over SFTP, type a path,
//! or type a glob. The chosen files and Live Globs are added to a New Log
//! Space or to a Remote Log Space.

use super::AppView;
use crate::remote::browse::{
    DirEntry, Resolved, SortKey, SortOrder, dirname, filter_entries, join, list_dir, resolve_input, sort_entries,
};
use crate::remote::{ConnectError, Server, ServerStore};
use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerTarget {
    NewSpace,
    AddTo(String),
}

/// Something chosen in the picker, waiting for Done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Staged {
    File { server_id: String, path: String },
    Glob { server_id: String, dir: String, pattern: String },
}

impl Staged {
    pub fn server_id(&self) -> &str {
        match self {
            Staged::File { server_id, .. } | Staged::Glob { server_id, .. } => server_id,
        }
    }

    /// The path, or the glob as typed.
    pub fn path(&self) -> String {
        match self {
            Staged::File { path, .. } => path.clone(),
            Staged::Glob { dir, pattern, .. } => join(dir, pattern),
        }
    }

    pub fn is_glob(&self) -> bool {
        matches!(self, Staged::Glob { .. })
    }

    pub fn set_server_id(&mut self, id: &str) {
        match self {
            Staged::File { server_id, .. } | Staged::Glob { server_id, .. } => *server_id = id.to_string(),
        }
    }
}

/// The most rows of a folder drawn at once; the filter narrows the rest.
pub const MAX_PICKER_ROWS: usize = 2_000;

/// What the picker adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PickerTab {
    #[default]
    Files,
    Command,
}

/// The small sign-in form shown when the agent was refused by a Server
/// made by Quick Connect: a password or a key file typed here.
pub struct QuickLogin {
    pub server_id: String,
    pub server_name: String,
    pub use_key: bool,
    pub remember: bool,
    /// Password, or key passphrase (created while rendering).
    pub secret: Option<Entity<InputState>>,
    pub key_path: Option<Entity<InputState>>,
    pub error: Option<String>,
}

pub struct RemotePicker {
    pub tab: PickerTab,
    /// The Quick Connect field.
    pub quick_input: Option<Entity<InputState>>,
    /// Servers made by Quick Connect and not saved yet; saved when a file of
    /// theirs is added with Done.
    pub quick_servers: Vec<Server>,
    /// Typed secrets of those Servers, with "Remember", for the save.
    pub quick_secrets: std::collections::HashMap<String, (String, bool)>,
    /// What to add once the Quick Connect Server is connected.
    pub quick_pending: Option<String>,
    pub quick_login: Option<QuickLogin>,
    /// The Command tab.
    pub command: super::remote_command::CommandForm,
    pub target: PickerTarget,
    pub server_id: Option<String>,
    pub dir: String,
    pub entries: Vec<DirEntry>,
    /// What the picker is waiting on, if anything.
    pub busy: Option<PickerBusy>,
    pub error: Option<String>,
    /// What the last add did ("Added 3 files"), shown muted.
    pub info: Option<String>,
    /// What was chosen so far, on any Server.
    pub staged: Vec<Staged>,
    /// Servers ticked next to the Add button, besides the selected one.
    pub checked: HashSet<String>,
    pub sort: SortOrder,
    /// What the filter field holds.
    pub filter: String,
    /// The path field, created while rendering.
    pub input: Option<Entity<InputState>>,
    pub filter_input: Option<Entity<InputState>>,
    /// The search field of the Server chooser.
    pub server_search: Option<Entity<InputState>>,
    /// The fields are emptied at the next render (it needs a window).
    pub clear_input: bool,
    pub clear_filter: bool,
    /// Only the answer to the latest request is shown.
    request: u64,
}

/// What the picker waits on, said in the words the user sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerBusy {
    /// Listing a folder (`None`: the home folder). Its rows are hidden
    /// meanwhile, so a row of the folder being left cannot be clicked.
    Listing(Option<String>),
    /// Checking what was typed: a path, or a glob to match.
    Resolving(String),
}

impl PickerBusy {
    pub fn label(&self) -> String {
        match self {
            PickerBusy::Listing(Some(dir)) => format!("Listing {dir}\u{2026}"),
            PickerBusy::Listing(None) => "Listing the home folder\u{2026}".into(),
            PickerBusy::Resolving(text) if is_glob(text) => format!("Matching {}\u{2026}", text.trim()),
            PickerBusy::Resolving(text) => format!("Checking {}\u{2026}", text.trim()),
        }
    }
}

fn is_glob(text: &str) -> bool {
    text.contains(['*', '?', '['])
}

/// What one add did, for the line under the path field.
pub fn added_message(added_files: usize, skipped_files: usize, added_globs: usize, skipped_globs: usize) -> Option<String> {
    let plural = |n: usize, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
    let mut parts = Vec::new();
    if added_files > 0 {
        parts.push(plural(added_files, "file", "files"));
    }
    if added_globs > 0 {
        parts.push(plural(added_globs, "Live Glob", "Live Globs"));
    }
    let skipped = skipped_files + skipped_globs;
    let already = (skipped > 0).then(|| format!("{skipped} already chosen"));
    match (parts.is_empty(), already) {
        (true, None) => None,
        (true, Some(already)) => Some(format!("Nothing new: {already}")),
        (false, None) => Some(format!("Added {}", parts.join(" and "))),
        (false, Some(already)) => Some(format!("Added {} ({already})", parts.join(" and "))),
    }
}

impl RemotePicker {
    /// The line the file list shows instead of rows, if any: what it is
    /// loading, or why it is empty.
    pub fn list_notice(&self) -> Option<String> {
        match &self.busy {
            Some(busy @ PickerBusy::Listing(_)) => return Some(busy.label()),
            _ => {}
        }
        if !self.visible_entries().is_empty() {
            return None;
        }
        if self.entries.is_empty() {
            Some("This folder is empty".into())
        } else {
            Some("Nothing in this folder matches the filter".into())
        }
    }

    /// Whether the folder's rows are drawn (not while another is listed).
    pub fn shows_rows(&self) -> bool {
        !matches!(self.busy, Some(PickerBusy::Listing(_)))
    }

    pub fn is_staged(&self, server_id: &str, path: &str) -> bool {
        self.staged
            .iter()
            .any(|s| matches!(s, Staged::File { server_id: sid, path: p } if sid == server_id && p == path))
    }

    /// The listing after the filter and the sort.
    pub fn visible_entries(&self) -> Vec<&DirEntry> {
        let mut entries = filter_entries(&self.entries, &self.filter);
        sort_entries(&mut entries, self.sort);
        entries
    }

    /// Whether a Server is ticked (the selected one always is).
    pub fn is_ticked(&self, server_id: &str) -> bool {
        self.server_id.as_deref() == Some(server_id) || self.checked.contains(server_id)
    }
}

/// What a request to a Server found.
enum Listing {
    Dir {
        dir: String,
        entries: Vec<DirEntry>,
        /// The remembered folder that could not be opened, when this is home.
        unavailable: Option<String>,
    },
}

type LoadError = (String, Option<ConnectError>);

fn describe(e: ConnectError) -> String {
    e.message()
}

async fn home_dir(sftp: &russh_sftp::client::SftpSession) -> Result<String, LoadError> {
    sftp.canonicalize(".").await.map_err(|e| (e.to_string(), None))
}

/// How a load treats a failure.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LoadKind {
    /// The user chose a Server: open its remembered folder, home when it is
    /// gone.
    Open,
    /// The user moved within the Server: on failure stay where they are.
    Navigate,
}

impl AppView {
    /// Open the picker for a New Log Space or for an existing Remote Log
    /// Space. Without any Server it opens the Servers dialog instead.
    pub fn open_remote_picker(&mut self, target: PickerTarget, cx: &mut Context<Self>) {
        if self.remote.servers.servers.is_empty() {
            self.open_servers_dialog(cx);
            self.new_server_form(cx);
            return;
        }
        let first = self.remote.servers.servers[0].id.clone();
        self.remote.picker = Some(RemotePicker {
            tab: PickerTab::Files,
            quick_input: None,
            quick_servers: Vec::new(),
            quick_secrets: Default::default(),
            quick_pending: None,
            quick_login: None,
            command: Default::default(),
            target,
            server_id: None,
            dir: "/".into(),
            entries: Vec::new(),
            busy: None,
            info: None,
            error: None,
            staged: Vec::new(),
            checked: HashSet::new(),
            sort: self.remote.picker_state.sort,
            filter: String::new(),
            input: None,
            filter_input: None,
            server_search: None,
            clear_input: false,
            clear_filter: false,
            request: 0,
        });
        self.picker_select_server(&first, cx);
    }

    pub fn close_remote_picker(&mut self, cx: &mut Context<Self>) {
        self.remote.picker = None;
        cx.notify();
    }

    /// Create the fields of a freshly opened picker, and empty them when asked.
    pub fn sync_remote_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_command_form(window, cx);
        self.sync_quick_connect(window, cx);
        let Some(picker) = self.remote.picker.as_mut() else { return };
        if picker.clear_input {
            picker.clear_input = false;
            if let Some(input) = picker.input.clone() {
                input.update(cx, |state, cx| state.set_value(gpui::SharedString::from(""), window, cx));
            }
        }
        if picker.clear_filter {
            picker.clear_filter = false;
            if let Some(input) = picker.filter_input.clone() {
                input.update(cx, |state, cx| state.set_value(gpui::SharedString::from(""), window, cx));
            }
        }
        if picker.input.is_none() {
            let input =
                cx.new(|cx| InputState::new(window, cx).placeholder("/var/log/app/*.log or /var/log/app.log"));
            cx.subscribe_in(&input, window, |this: &mut Self, _input, event: &InputEvent, _window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.picker_add_typed(cx);
                }
            })
            .detach();
            picker.input = Some(input);
        }
        if picker.server_search.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search Servers"));
            cx.subscribe_in(&input, window, |this: &mut Self, input, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => {
                    let query = input.read(cx).value().to_string();
                    let first = crate::remote::server_search::first_match(&this.remote.servers.servers, &query)
                        .map(|s| s.id.clone());
                    if let Some(id) = first {
                        this.picker_select_server(&id, cx);
                    }
                }
                _ => {}
            })
            .detach();
            picker.server_search = Some(input);
        }
        if picker.filter_input.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Filter this folder"));
            cx.subscribe_in(&input, window, |this: &mut Self, input, event: &InputEvent, _window, cx| {
                if let InputEvent::Change = event {
                    let text = input.read(cx).value().to_string();
                    if let Some(picker) = this.remote.picker.as_mut() {
                        picker.filter = text;
                    }
                    cx.notify();
                }
            })
            .detach();
            picker.filter_input = Some(input);
        }
    }

    pub fn picker_select_server(&mut self, server_id: &str, cx: &mut Context<Self>) {
        let remembered = self.remote.picker_state.last_dir(server_id).map(str::to_string);
        let on_files = if let Some(picker) = self.remote.picker.as_mut() {
            picker.server_id = Some(server_id.to_string());
            picker.entries.clear();
            picker.error = None;
            picker.command.suggestions.clear();
            picker.command.suggestions_error = None;
            picker.tab == PickerTab::Files
        } else {
            false
        };
        if on_files {
            self.load_picker_dir(remembered, LoadKind::Open, cx);
        } else {
            self.command_load_suggestions(cx);
            cx.notify();
        }
    }

    /// List `dir` (the Server's home folder when `None`). A failure leaves
    /// the folder that was open as it was.
    pub fn picker_load(&mut self, dir: Option<String>, cx: &mut Context<Self>) {
        self.load_picker_dir(dir, LoadKind::Navigate, cx);
    }

    fn load_picker_dir(&mut self, dir: Option<String>, kind: LoadKind, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        let Some(server_id) = picker.server_id.clone() else { return };
        picker.busy = Some(PickerBusy::Listing(dir.clone()));
        picker.error = None;
        picker.request += 1;
        let request = picker.request;
        let hub = self.remote_hub(cx);
        let store: ServerStore = self.picker_store();
        let task = hub.spawn({
            let hub = hub.clone();
            let server_id = server_id.clone();
            async move {
                let session = hub.session(&store, &server_id).await.map_err(|e| (e.message(), Some(e)))?;
                let sftp = session.open_sftp().await.map_err(|e| (e, None))?;
                let (dir, unavailable, entries) = match dir {
                    Some(dir) => match list_dir(&sftp, &dir).await {
                        Ok(entries) => (dir, None, entries),
                        Err(_) if kind == LoadKind::Open => {
                            let home = home_dir(&sftp).await?;
                            let entries = list_dir(&sftp, &home).await.map_err(|e| (e, None))?;
                            (home, Some(dir), entries)
                        }
                        Err(message) => return Err((message, None)),
                    },
                    None => {
                        let home = home_dir(&sftp).await?;
                        let entries = list_dir(&sftp, &home).await.map_err(|e| (e, None))?;
                        (home, None, entries)
                    }
                };
                Ok::<_, LoadError>(Listing::Dir { dir, entries, unavailable })
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let server = this.remote.picker.as_ref().and_then(|p| p.server_id.clone());
                let Some(picker) = this.remote.picker.as_mut() else { return };
                if picker.request != request {
                    return;
                }
                picker.busy = None;
                match result {
                    Ok(Ok(Listing::Dir { dir, entries, unavailable })) => {
                        let changed = picker.dir != dir || picker.server_id != server;
                        picker.dir = dir.clone();
                        picker.entries = entries;
                        if changed || kind == LoadKind::Open {
                            picker.filter.clear();
                            picker.clear_filter = true;
                        }
                        picker.error = unavailable.map(|old| format!("{old} is no longer available"));
                        if let Some(server_id) = server {
                            if !crate::remote::quick::is_temporary(&server_id) {
                                this.remote.picker_state.set_last_dir(&server_id, &dir);
                                this.save_picker_state();
                            }
                            // A Quick Connect string waits for this connection.
                            if let Some(path) = this.remote.picker.as_mut().and_then(|p| p.quick_pending.take()) {
                                this.picker_add_text_on(path, vec![server_id], cx);
                            }
                        }
                    }
                    Ok(Err((message, connect))) => {
                        if kind == LoadKind::Open {
                            picker.entries.clear();
                        }
                        picker.error = Some(message);
                        match (connect, server) {
                            (Some(ConnectError::NeedsSecret { server_id, .. }), Some(_)) => {
                                if !crate::remote::quick::is_temporary(&server_id) {
                                    this.open_secret_prompt(&server_id, cx);
                                }
                            }
                            (Some(ConnectError::AgentRejected { .. } | ConnectError::AgentUnavailable { .. }), Some(sid))
                                if crate::remote::quick::is_temporary(&sid) =>
                            {
                                this.open_quick_login(&sid);
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        picker.error = Some(format!("Could not reach the server: {e}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Write what the picker remembers.
    pub(crate) fn save_picker_state(&self) {
        let _ = self.remote.picker_state.save_to(&self.remote.picker_state_path);
    }

    pub fn picker_open_dir(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_ref() else { return };
        let dir = join(&picker.dir, name);
        self.picker_load(Some(dir), cx);
    }

    pub fn picker_up(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_ref() else { return };
        let parent = dirname(&picker.dir);
        self.picker_load(Some(parent), cx);
    }

    /// The shortcut to the Server's home folder.
    pub fn picker_go_home(&mut self, cx: &mut Context<Self>) {
        self.picker_load(None, cx);
    }

    /// The shortcut to `/var/log`.
    pub fn picker_go_var_log(&mut self, cx: &mut Context<Self>) {
        self.picker_load(Some("/var/log".to_string()), cx);
    }

    /// Click a column header: sort by it, or flip the direction.
    pub fn picker_sort_by(&mut self, key: SortKey, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        picker.sort = picker.sort.toggled(key);
        self.remote.picker_state.sort = picker.sort;
        self.save_picker_state();
        cx.notify();
    }

    /// Tick or untick a Server to add the typed path on as well.
    pub fn picker_toggle_server(&mut self, server_id: &str, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        if picker.server_id.as_deref() == Some(server_id) {
            return;
        }
        if !picker.checked.remove(server_id) {
            picker.checked.insert(server_id.to_string());
        }
        cx.notify();
    }

    /// Tick or untick a file of the listing.
    pub fn picker_toggle_file(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        let Some(server_id) = picker.server_id.clone() else { return };
        let path = join(&picker.dir, name);
        let item = Staged::File { server_id, path };
        if let Some(pos) = picker.staged.iter().position(|s| *s == item) {
            picker.staged.remove(pos);
        } else {
            picker.staged.push(item);
        }
        cx.notify();
    }

    pub fn picker_unstage(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(picker) = self.remote.picker.as_mut() {
            if index < picker.staged.len() {
                picker.staged.remove(index);
            }
        }
        cx.notify();
    }

    /// Add what the path field holds.
    pub fn picker_add_typed(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_ref() else { return };
        let Some(input) = picker.input.clone() else { return };
        let text = input.read(cx).value().to_string();
        self.picker_add_text(text, cx);
    }

    /// Click a Recent Path: it goes to the waiting list as if typed again.
    pub fn picker_add_recent(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(server_id) = self.remote.picker.as_ref().and_then(|p| p.server_id.clone()) else { return };
        let Some(recent) = self.remote.picker_state.recent(&server_id).get(index).cloned() else { return };
        self.picker_add_text(recent.path, cx);
    }

    pub fn picker_remove_recent(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(server_id) = self.remote.picker.as_ref().and_then(|p| p.server_id.clone()) else { return };
        self.remote.picker_state.remove_recent(&server_id, index);
        self.save_picker_state();
        cx.notify();
    }

    pub fn picker_clear_recent(&mut self, cx: &mut Context<Self>) {
        let Some(server_id) = self.remote.picker.as_ref().and_then(|p| p.server_id.clone()) else { return };
        self.remote.picker_state.clear_recent(&server_id);
        self.save_picker_state();
        cx.notify();
    }

    /// Check `text` on every ticked Server at once; what passes goes to the
    /// waiting list, each failure is reported with its Server's name.
    pub fn picker_add_text(&mut self, text: String, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            return;
        }
        let Some(picker) = self.remote.picker.as_mut() else { return };
        let Some(selected) = picker.server_id.clone() else { return };
        let mut targets = vec![selected.clone()];
        targets.extend(
            self.remote
                .servers
                .servers
                .iter()
                .map(|s| s.id.clone())
                .filter(|id| *id != selected && picker.checked.contains(id)),
        );
        self.picker_add_text_on(text, targets, cx);
    }

    /// Like `picker_add_text`, on exactly these Servers (the first is the
    /// selected one).
    pub fn picker_add_text_on(&mut self, text: String, targets: Vec<String>, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        let Some(selected) = targets.first().cloned() else { return };
        picker.busy = Some(PickerBusy::Resolving(text.clone()));
        picker.error = None;
        picker.info = None;
        let hub = self.remote_hub(cx);
        let store: ServerStore = self.picker_store();
        let tasks: Vec<_> = targets
            .into_iter()
            .map(|sid| {
                let task = hub.spawn({
                    let (hub, store, text, sid) = (hub.clone(), store.clone(), text.clone(), sid.clone());
                    async move {
                        let session = hub.session(&store, &sid).await.map_err(describe)?;
                        let sftp = session.open_sftp().await?;
                        // `~/…` is the user's home folder on that Server.
                        let text = match text.trim().strip_prefix("~/") {
                            Some(rest) => join(&home_dir(&sftp).await.map_err(|e| e.0)?, rest),
                            None => text,
                        };
                        resolve_input(&sftp, &text).await
                    }
                });
                (sid, task)
            })
            .collect();
        cx.spawn(async move |this, cx| {
            let mut results = Vec::new();
            for (sid, task) in tasks {
                results.push((sid, task.await));
            }
            this.update(cx, |this, cx| this.picker_resolved(&selected, results, cx)).ok();
        })
        .detach();
        cx.notify();
    }

    /// What checking the typed text found on each Server: files and Live
    /// Globs go to the waiting list, a folder of the selected Server opens.
    #[allow(clippy::type_complexity)]
    pub fn picker_resolved(
        &mut self,
        selected: &str,
        results: Vec<(String, Result<Result<Resolved, String>, tokio::task::JoinError>)>,
        cx: &mut Context<Self>,
    ) {
        let names: Vec<(String, String)> = results
            .iter()
            .map(|(sid, _)| (sid.clone(), self.picker_server(sid).map(|s| s.name).unwrap_or_default()))
            .collect();
        let name_of = |sid: &str| names.iter().find(|(s, _)| s == sid).map(|(_, n)| n.clone()).unwrap_or_default();
        let Some(picker) = self.remote.picker.as_mut() else { return };
        if matches!(picker.busy, Some(PickerBusy::Resolving(_))) {
            picker.busy = None;
        }
        let mut errors = Vec::new();
        let mut open_dir = None;
        let (mut added_files, mut skipped_files, mut added_globs, mut skipped_globs) = (0, 0, 0, 0);
        for (sid, result) in results {
            match result {
                Ok(Ok(Resolved::Dir(dir))) => {
                    if sid == selected {
                        open_dir = Some(dir);
                    }
                }
                Ok(Ok(Resolved::Files(paths))) => {
                    for path in paths {
                        let item = Staged::File { server_id: sid.clone(), path };
                        if picker.staged.contains(&item) {
                            skipped_files += 1;
                        } else {
                            picker.staged.push(item);
                            added_files += 1;
                        }
                    }
                }
                Ok(Ok(Resolved::Glob { dir, pattern })) => {
                    let item = Staged::Glob { server_id: sid.clone(), dir, pattern };
                    if picker.staged.contains(&item) {
                        skipped_globs += 1;
                    } else {
                        picker.staged.push(item);
                        added_globs += 1;
                    }
                }
                Ok(Err(message)) => errors.push(format!("{}: {message}", name_of(&sid))),
                Err(e) => errors.push(format!("{}: Could not reach the server: {e}", name_of(&sid))),
            }
        }
        picker.error = (!errors.is_empty()).then(|| errors.join("\n"));
        picker.info = added_message(added_files, skipped_files, added_globs, skipped_globs);
        // The typed text stays after an add, to be tweaked; opening a folder
        // empties it.
        if open_dir.is_some() {
            picker.clear_input = true;
        }
        if let Some(dir) = open_dir {
            self.picker_load(Some(dir), cx);
            // The load clears the error; keep what other Servers said.
            if !errors.is_empty() {
                if let Some(picker) = self.remote.picker.as_mut() {
                    picker.error = Some(errors.join("\n"));
                }
            }
        }
        cx.notify();
    }

    /// Done: the chosen files and Live Globs go to the New Log Space or the
    /// Remote Log Space, and each Server remembers what was really added.
    pub fn picker_confirm(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.take() else { return };
        let server_name = |sid: &str| self.remote.servers.get(sid).map(|s| s.name.clone());
        let _ = &server_name;
        let mut picker = picker;
        self.save_quick_servers(&mut picker, cx);
        let server_name = |sid: &str| self.remote.servers.get(sid).map(|s| s.name.clone());
        let mut files: Vec<(String, String, String)> = Vec::new();
        let mut globs: Vec<(String, String, String, String)> = Vec::new();
        for item in &picker.staged {
            let Some(name) = server_name(item.server_id()) else { continue };
            match item {
                Staged::File { server_id, path } => files.push((server_id.clone(), name, path.clone())),
                Staged::Glob { server_id, dir, pattern } => {
                    globs.push((server_id.clone(), name, dir.clone(), pattern.clone()))
                }
            }
        }
        let mut remembered: Vec<(String, String, bool)> = Vec::new();
        match picker.target {
            PickerTarget::NewSpace => {
                for file in files {
                    if !self.remote.new_space_files.contains(&file) {
                        remembered.push((file.0.clone(), file.2.clone(), false));
                        self.remote.new_space_files.push(file);
                    }
                }
                for glob in globs {
                    if !self.remote.new_space_globs.contains(&glob) {
                        remembered.push((glob.0.clone(), join(&glob.2, &glob.3), true));
                        self.remote.new_space_globs.push(glob);
                    }
                }
            }
            PickerTarget::AddTo(space_id) => {
                remembered = self.add_remote_items_to_space(&space_id, files, globs, cx);
            }
        }
        for (server_id, path, is_glob) in remembered {
            self.remote.picker_state.add_recent(&server_id, &path, is_glob);
        }
        self.save_picker_state();
        cx.notify();
    }

    /// Add Remote Files to a Remote Log Space; files it already has are
    /// skipped with a message.
    pub fn add_remote_files_to_space(
        &mut self,
        space_id: &str,
        files: Vec<(String, String, String)>,
        cx: &mut Context<Self>,
    ) {
        self.add_remote_items_to_space(space_id, files, Vec::new(), cx);
    }

    /// Add Remote Files and Live Globs `(server id, server name, dir,
    /// pattern)` to a Remote Log Space. Returns what was really added, as
    /// `(server id, path or glob, is glob)`.
    pub fn add_remote_items_to_space(
        &mut self,
        space_id: &str,
        files: Vec<(String, String, String)>,
        globs: Vec<(String, String, String, String)>,
        cx: &mut Context<Self>,
    ) -> Vec<(String, String, bool)> {
        use crate::workspace::log_space::AddGlobError;
        self.persist_active_session();
        let mut added = Vec::new();
        let mut notices = Vec::new();
        let mut new_globs = Vec::new();
        if let Some(space) = self.active_project_mut().and_then(|p| p.log_space_mut(space_id)) {
            if let Ok(outcome) = space.add_remote_files(&files) {
                if !outcome.skipped.is_empty() {
                    notices.push(format!("Already in this Log Space: {}", outcome.skipped.join(", ")));
                }
                for (server_id, _, path) in &files {
                    let id = crate::workspace::log_space::LogFile::remote(server_id, "", path).file_id;
                    if outcome.added.contains(&id) {
                        added.push((server_id.clone(), path.clone(), false));
                    }
                }
            }
            for (server_id, server_name, dir, pattern) in &globs {
                match space.add_live_glob(server_id, dir, pattern) {
                    Ok(glob_id) => {
                        added.push((server_id.clone(), join(dir, pattern), true));
                        new_globs.push(glob_id);
                    }
                    Err(AddGlobError::Duplicate) => {
                        notices.push(format!("Live Glob already in this Log Space: {server_name}:{}", join(dir, pattern)))
                    }
                    Err(AddGlobError::WrongKind) => {}
                }
            }
        }
        if !notices.is_empty() {
            self.notice = Some(notices.join("; "));
        }
        self.refresh_after_space_change(cx);
        if !new_globs.is_empty() {
            self.sync_glob_watchers(cx);
        }
        added
    }
}
