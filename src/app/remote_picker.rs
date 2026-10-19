//! Logic: choosing files on a Server: browse folders over SFTP, type a path,
//! or type a glob. The chosen files are added to a New Log Space or to a
//! Remote Log Space.

use super::AppView;
use crate::remote::browse::{DirEntry, dirname, join, list_dir, resolve_input};
use crate::remote::{ConnectError, ServerStore};
use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerTarget {
    NewSpace,
    AddTo(String),
}

/// What the picker adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PickerTab {
    #[default]
    Files,
    Command,
}

pub struct RemotePicker {
    pub tab: PickerTab,
    /// The Command tab.
    pub command: super::remote_command::CommandForm,
    pub target: PickerTarget,
    pub server_id: Option<String>,
    pub dir: String,
    pub entries: Vec<DirEntry>,
    pub loading: bool,
    pub error: Option<String>,
    /// Files chosen so far, as `(server id, path)`, on any Server.
    pub staged: Vec<(String, String)>,
    /// The path or glob field, created while rendering.
    pub input: Option<Entity<InputState>>,
    /// The path field is emptied at the next render (it needs a window).
    pub clear_input: bool,
    /// Only the answer to the latest request is shown.
    request: u64,
}

impl RemotePicker {
    pub fn is_staged(&self, server_id: &str, path: &str) -> bool {
        self.staged.iter().any(|(s, p)| s == server_id && p == path)
    }
}

/// What a request to a Server found.
enum Listing {
    Dir { dir: String, entries: Vec<DirEntry> },
}

fn describe(e: ConnectError) -> String {
    e.message()
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
            command: Default::default(),
            target,
            server_id: None,
            dir: "/".into(),
            entries: Vec::new(),
            loading: false,
            error: None,
            staged: Vec::new(),
            input: None,
            clear_input: false,
            request: 0,
        });
        self.picker_select_server(&first, cx);
    }

    pub fn close_remote_picker(&mut self, cx: &mut Context<Self>) {
        self.remote.picker = None;
        cx.notify();
    }

    /// Create the path field of a freshly opened picker.
    pub fn sync_remote_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_command_form(window, cx);
        let Some(picker) = self.remote.picker.as_mut() else { return };
        if picker.clear_input {
            picker.clear_input = false;
            if let Some(input) = picker.input.clone() {
                input.update(cx, |state, cx| state.set_value(gpui::SharedString::from(""), window, cx));
            }
        }
        if picker.input.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("/var/log/app/*.log or /var/log/app.log"));
        cx.subscribe_in(&input, window, |this: &mut Self, _input, event: &InputEvent, _window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                this.picker_add_typed(cx);
            }
        })
        .detach();
        picker.input = Some(input);
    }

    pub fn picker_select_server(&mut self, server_id: &str, cx: &mut Context<Self>) {
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
            self.picker_load(None, cx);
        } else {
            self.command_load_suggestions(cx);
            cx.notify();
        }
    }

    /// List `dir` (the Server's home folder when `None`).
    pub fn picker_load(&mut self, dir: Option<String>, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        let Some(server_id) = picker.server_id.clone() else { return };
        picker.loading = true;
        picker.error = None;
        picker.request += 1;
        let request = picker.request;
        let hub = self.remote_hub(cx);
        let store: ServerStore = self.remote.servers.clone();
        let task = hub.spawn({
            let hub = hub.clone();
            async move {
                let session = hub.session(&store, &server_id).await.map_err(|e| (e.message(), Some(e)))?;
                let sftp = session.open_sftp().await.map_err(|e| (e, None))?;
                let dir = match dir {
                    Some(dir) => dir,
                    None => sftp.canonicalize(".").await.map_err(|e| (e.to_string(), None))?,
                };
                let entries = list_dir(&sftp, &dir).await.map_err(|e| (e, None))?;
                Ok::<_, (String, Option<ConnectError>)>(Listing::Dir { dir, entries })
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
                picker.loading = false;
                match result {
                    Ok(Ok(Listing::Dir { dir, entries })) => {
                        picker.dir = dir;
                        picker.entries = entries;
                    }
                    Ok(Err((message, connect))) => {
                        picker.entries.clear();
                        picker.error = Some(message);
                        if let (Some(ConnectError::NeedsSecret { server_id, .. }), Some(_)) = (connect, server) {
                            this.open_secret_prompt(&server_id, cx);
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

    /// Tick or untick a file of the listing.
    pub fn picker_toggle_file(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        let Some(server_id) = picker.server_id.clone() else { return };
        let path = join(&picker.dir, name);
        if let Some(pos) = picker.staged.iter().position(|(s, p)| *s == server_id && *p == path) {
            picker.staged.remove(pos);
        } else {
            picker.staged.push((server_id, path));
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

    /// Add what the path field holds: a path, or a glob matched now.
    pub fn picker_add_typed(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.as_mut() else { return };
        let (Some(server_id), Some(input)) = (picker.server_id.clone(), picker.input.clone()) else { return };
        let text = input.read(cx).value().to_string();
        if text.trim().is_empty() {
            return;
        }
        picker.loading = true;
        picker.error = None;
        let hub = self.remote_hub(cx);
        let store: ServerStore = self.remote.servers.clone();
        let sid = server_id.clone();
        let task = hub.spawn({
            let hub = hub.clone();
            async move {
                let session = hub.session(&store, &sid).await.map_err(describe)?;
                let sftp = session.open_sftp().await?;
                resolve_input(&sftp, &text).await
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let Some(picker) = this.remote.picker.as_mut() else { return };
                picker.loading = false;
                match result {
                    Ok(Ok(paths)) => {
                        for path in paths {
                            if !picker.is_staged(&server_id, &path) {
                                picker.staged.push((server_id.clone(), path));
                            }
                        }
                        picker.error = None;
                        picker.clear_input = true;
                    }
                    Ok(Err(message)) => picker.error = Some(message),
                    Err(e) => picker.error = Some(format!("Could not reach the server: {e}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Done: the chosen files go to the New Log Space or the Remote Log Space.
    pub fn picker_confirm(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = self.remote.picker.take() else { return };
        let files: Vec<(String, String, String)> = picker
            .staged
            .iter()
            .filter_map(|(server_id, path)| {
                self.remote
                    .servers
                    .get(server_id)
                    .map(|s| (server_id.clone(), s.name.clone(), path.clone()))
            })
            .collect();
        match picker.target {
            PickerTarget::NewSpace => {
                for file in files {
                    if !self.remote.new_space_files.contains(&file) {
                        self.remote.new_space_files.push(file);
                    }
                }
            }
            PickerTarget::AddTo(space_id) => self.add_remote_files_to_space(&space_id, files, cx),
        }
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
        self.persist_active_session();
        let outcome = self
            .active_project_mut()
            .and_then(|p| p.log_space_mut(space_id))
            .and_then(|s| s.add_remote_files(&files).ok());
        if let Some(outcome) = outcome {
            if !outcome.skipped.is_empty() {
                self.notice = Some(format!("Already in this Log Space: {}", outcome.skipped.join(", ")));
            }
        }
        self.refresh_after_space_change(cx);
    }
}
