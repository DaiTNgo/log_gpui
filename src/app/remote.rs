//! Logic: Servers, the SSH hub, host key and password prompts, and what
//! connects Remote Files to their sources.

use super::AppView;
use crate::live::{LineSource, RemoteCommandSource, RemoteFileSource, SourceCommand, SourceStatus};
use crate::remote::{
    AuthMethod, HostKeyRequest, Keychain, SignInRequest, KnownHosts, RemoteHub, SecretStore, Server, ServerError, ServerStore,
};
use crate::workspace::log_space::{LogSpaceKind, RemoteTarget};
use crate::workspace::session::app_data_dir;
use gpui::{Context, Entity};
use gpui_component::input::InputState;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub fn servers_path() -> PathBuf {
    app_data_dir().join("servers.json")
}

pub fn known_hosts_path() -> PathBuf {
    app_data_dir().join("known_hosts.json")
}

/// The password or passphrase prompt of a Server.
pub struct SecretPrompt {
    pub server_id: String,
    pub server_name: String,
    pub passphrase: bool,
    pub remember: bool,
    pub error: Option<String>,
    /// Created while rendering, where a window exists.
    pub input: Option<Entity<InputState>>,
}

/// A question of the Server on top of the Sign-in Prompts, with its answer
/// fields (created while rendering, where a window exists).
pub struct SignInDialog {
    pub request: SignInRequest,
    pub inputs: Vec<Entity<InputState>>,
}

/// Servers, the hub and the dialogs that go with them.
pub struct RemoteUi {
    pub servers: ServerStore,
    /// The same Servers, read by the sources when they connect.
    pub shared_servers: Arc<Mutex<ServerStore>>,
    pub hub: Option<Arc<RemoteHub>>,
    pub known_hosts: Arc<Mutex<KnownHosts>>,
    /// Host keys waiting for the user's answer, the first one shown.
    pub host_prompts: VecDeque<HostKeyRequest>,
    pub secret_prompt: Option<SecretPrompt>,
    /// Sign-in Prompts waiting for the user, the first one shown.
    pub sign_in_prompts: VecDeque<SignInRequest>,
    pub sign_in_dialog: Option<SignInDialog>,
    pub picker: Option<super::remote_picker::RemotePicker>,
    pub download: Option<DownloadState>,
    /// The Rotated Files to tick before a download.
    pub download_options: Option<DownloadOptions>,
    /// The Open in full dialog.
    pub full_dialog: Option<super::open_in_full::OpenInFull>,
    /// Remote Files chosen for the New Log Space being made, as
    /// `(server id, server name, path)`.
    pub new_space_files: Vec<(String, String, String)>,
    /// Live Globs chosen for the New Log Space, as `(server id, server name,
    /// folder, pattern)`.
    pub new_space_globs: Vec<(String, String, String, String)>,
    /// Starting Time typed in the picker for the Remote Files of the New Log
    /// Space.
    pub new_space_start: Option<String>,
    /// What the file picker remembers, and where it is written.
    pub picker_state: crate::remote::picker_state::PickerState,
    pub picker_state_path: PathBuf,
    /// Remote Commands chosen for the New Log Space: `(server id, server name,
    /// command, preset)`.
    pub new_space_commands: Vec<(String, String, String, Option<crate::remote::PresetFields>)>,
    pub secrets: Arc<dyn SecretStore>,
}

impl RemoteUi {
    pub fn load() -> Self {
        Self::with_paths(&servers_path(), &known_hosts_path(), Arc::new(Keychain))
    }

    pub fn with_paths(servers: &std::path::Path, known: &std::path::Path, secrets: Arc<dyn SecretStore>) -> Self {
        let store = ServerStore::load(servers);
        // Next to `servers.json`.
        let picker_state_path = servers.with_file_name("picker_state.json");
        Self {
            shared_servers: Arc::new(Mutex::new(store.clone())),
            servers: store,
            hub: None,
            known_hosts: Arc::new(Mutex::new(KnownHosts::load(known))),
            host_prompts: VecDeque::new(),
            secret_prompt: None,
            sign_in_prompts: VecDeque::new(),
            sign_in_dialog: None,
            picker: None,
            download: None,
            full_dialog: None,
            download_options: None,
            new_space_files: Vec::new(),
            new_space_globs: Vec::new(),
            new_space_start: None,
            picker_state: crate::remote::picker_state::PickerState::load(&picker_state_path),
            picker_state_path,
            new_space_commands: Vec::new(),
            secrets,
        }
    }
}

impl AppView {
    /// The hub, created on first use together with the loop that brings its
    /// host key questions to the user.
    pub(crate) fn remote_hub(&mut self, cx: &mut Context<Self>) -> Arc<RemoteHub> {
        if let Some(hub) = &self.remote.hub {
            return hub.clone();
        }
        let (hub, requests) = RemoteHub::new(self.remote.secrets.clone(), self.remote.known_hosts.clone());
        self.remote.hub = Some(hub.clone());
        // Quitting ends the commands still running on Servers (a source that
        // is closed asks its process to stop): wait for that, up to the
        // time the app gives a quitting app.
        let quit_hub = hub.clone();
        cx.on_app_quit(move |this, _cx| {
            for source in this.live.sources.values() {
                source.handle.close();
            }
            let hub = quit_hub.clone();
            async move {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                while hub.running_commands() > 0 && std::time::Instant::now() < deadline {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
        })
        .detach();
        // Connection Details follow the health of the Servers.
        let changes = hub.health().changes();
        cx.spawn(async move |this, cx| {
            while changes.recv().await.is_ok() {
                if this
                    .update(cx, |this, cx| {
                        this.sync_sign_in_flags();
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            while let Ok(request) = requests.recv().await {
                if this
                    .update(cx, |this, cx| {
                        this.remote.host_prompts.push_back(request);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let sign_ins = hub.sign_in_requests();
        cx.spawn(async move |this, cx| {
            while let Ok(request) = sign_ins.recv().await {
                if this
                    .update(cx, |this, cx| {
                        this.remote.sign_in_prompts.push_back(request);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        hub
    }

    /// Answer the Sign-in Prompt on top (`None` cancels the connection to
    /// that Server). The answers are only passed on, never kept.
    pub fn answer_sign_in(&mut self, answers: Option<Vec<String>>, cx: &mut Context<Self>) {
        if let Some(dialog) = self.remote.sign_in_dialog.take() {
            let _ = dialog.request.reply.send(answers);
        }
        cx.notify();
    }

    /// Save in the Servers which of them needed Sign-in Prompts at their last
    /// connection (so a lost connection or a reopened Project waits for
    /// Connect instead of asking by itself).
    fn sync_sign_in_flags(&mut self) {
        let Some(hub) = self.remote.hub.clone() else { return };
        let changed: Vec<(String, bool)> = self
            .remote
            .servers
            .servers
            .iter()
            .filter_map(|s| {
                let now = hub.needs_sign_in_prompt(s);
                (now != s.needs_sign_in_prompt).then(|| (s.id.clone(), now))
            })
            .collect();
        if changed.is_empty() {
            return;
        }
        for (id, flag) in changed {
            if let Some(server) = self.remote.servers.servers.iter_mut().find(|s| s.id == id) {
                server.needs_sign_in_prompt = flag;
            }
        }
        let _ = self.remote.servers.save_to(&servers_path());
        *self.remote.shared_servers.lock().unwrap() = self.remote.servers.clone();
    }

    /// The user answered the host key question on top.
    pub fn answer_host_key(&mut self, trust: bool, cx: &mut Context<Self>) {
        if let Some(request) = self.remote.host_prompts.pop_front() {
            let _ = request.reply.send(trust);
            if trust {
                // The hub already trusts it; keep it for next time.
                let hosts = self.remote.known_hosts.lock().unwrap().clone();
                let _ = hosts.save_to(&known_hosts_path());
            }
        }
        cx.notify();
    }

    /// Ask for the password or passphrase of a Server.
    pub fn open_secret_prompt(&mut self, server_id: &str, cx: &mut Context<Self>) {
        let Some(server) = self.remote.servers.get(server_id) else { return };
        self.remote.secret_prompt = Some(SecretPrompt {
            server_id: server.id.clone(),
            server_name: server.name.clone(),
            passphrase: matches!(server.auth, AuthMethod::Key { .. }),
            remember: server.remember_secret,
            error: None,
            input: None,
        });
        cx.notify();
    }

    /// Ask again for a secret a Server refused: the files on it that report
    /// `AuthFailed` wait to Connect, and the secret dialog reopens with why.
    /// Called after each batch of source events; one Server at a time.
    pub fn ask_again_after_auth_failure(&mut self, cx: &mut Context<Self>) {
        if self.remote.secret_prompt.is_some() {
            return;
        }
        let server_id = self.workspace_state.projects.iter().flat_map(|p| &p.log_spaces).find_map(|space| {
            space.files.iter().find_map(|file| {
                let remote = file.remote.as_ref()?;
                let failed = self.live.sources.get(&file.file_id)?.status == SourceStatus::AuthFailed;
                failed.then(|| remote.server_id.clone())
            })
        });
        let Some(server_id) = server_id else { return };
        for id in self.remote_file_ids_on_server(&server_id) {
            if let Some(source) = self.live.sources.get_mut(&id) {
                if source.status == SourceStatus::AuthFailed {
                    source.status = SourceStatus::NeedsSecret;
                }
            }
        }
        self.open_secret_prompt(&server_id, cx);
        if let Some(prompt) = self.remote.secret_prompt.as_mut() {
            let what = if prompt.passphrase { "passphrase" } else { "password" };
            prompt.error = Some(format!("Wrong {what} \u{2014} try again"));
        }
    }

    pub fn cancel_secret_prompt(&mut self, cx: &mut Context<Self>) {
        self.remote.secret_prompt = None;
        cx.notify();
    }

    pub fn toggle_secret_remember(&mut self, cx: &mut Context<Self>) {
        if let Some(prompt) = self.remote.secret_prompt.as_mut() {
            prompt.remember = !prompt.remember;
        }
        cx.notify();
    }

    /// Use the typed secret and connect every file waiting on the Server.
    pub fn submit_secret(&mut self, secret: String, cx: &mut Context<Self>) {
        let Some(prompt) = self.remote.secret_prompt.take() else { return };
        let Some(mut server) = self.remote.servers.get(&prompt.server_id).cloned() else { return };
        if server.remember_secret != prompt.remember {
            server.remember_secret = prompt.remember;
            let _ = self.remote.servers.save(server.clone());
            let _ = self.remote.servers.save_to(&servers_path());
        }
        let hub = self.remote_hub(cx);
        hub.provide_secret(&server, &secret);
        self.reconnect_server_sources(&server.id);
        cx.notify();
    }

    /// Ask every source on a Server to try again now.
    pub fn reconnect_server_sources(&mut self, server_id: &str) {
        let ids = self.remote_file_ids_on_server(server_id);
        for id in ids {
            if let Some(source) = self.live.sources.get(&id) {
                source.handle.send(SourceCommand::Reconnect);
            }
        }
    }

    /// `file_id`s of the Remote Files of open Log Spaces on a Server.
    fn remote_file_ids_on_server(&self, server_id: &str) -> Vec<String> {
        let mut ids = Vec::new();
        for project in &self.workspace_state.projects {
            for space in &project.log_spaces {
                for file in space.files_on_server(server_id) {
                    ids.push(file.file_id.clone());
                }
            }
        }
        ids
    }

    /// What uses a Server, by Log Space and Project: for the refusal to
    /// delete it.
    pub fn server_users(&self, server_id: &str) -> Vec<String> {
        let mut users = Vec::new();
        for project in &self.workspace_state.projects {
            for space in &project.log_spaces {
                if !space.files_on_server(server_id).is_empty() {
                    users.push(format!("{} ({})", space.name, project.name));
                }
            }
        }
        users
    }

    /// Save a Server (new or edited) and write the list to disk. An edited
    /// Server in use reconnects with the new settings.
    pub fn save_server(&mut self, server: Server, secret: Option<String>, cx: &mut Context<Self>) -> Result<String, ServerError> {
        let editing = !server.id.is_empty();
        let id = self.remote.servers.save(server.clone())?;
        let _ = self.remote.servers.save_to(&servers_path());
        *self.remote.shared_servers.lock().unwrap() = self.remote.servers.clone();
        let saved = self.remote.servers.get(&id).cloned().expect("just saved");
        if let Some(secret) = secret.filter(|s| !s.is_empty()) {
            let hub = self.remote_hub(cx);
            hub.provide_secret(&saved, &secret);
        }
        if editing {
            self.rename_remote_files(&saved);
            self.restart_server_sources(&saved.id, cx);
        }
        cx.notify();
        Ok(id)
    }

    /// Remote Files show `<server>:<path>`: follow a rename.
    fn rename_remote_files(&mut self, server: &Server) {
        for project in &mut self.workspace_state.projects {
            for space in &mut project.log_spaces {
                for file in &mut space.files {
                    if let Some(remote) = &file.remote {
                        if remote.server_id == server.id {
                            file.display_name = match &remote.target {
                                RemoteTarget::File { path } => format!("{}:{}", server.name, path),
                                RemoteTarget::Command { command, .. } => format!("{}$ {}", server.name, command),
                            };
                        }
                    }
                }
            }
        }
    }

    /// Drop the connection of a Server: its files reconnect with the new
    /// settings from the byte where they stopped, a Break marking the cut.
    fn restart_server_sources(&mut self, server_id: &str, cx: &mut Context<Self>) {
        let _ = cx;
        if let Some(hub) = self.remote.hub.clone() {
            let sid = server_id.to_string();
            let h = hub.clone();
            hub.spawn(async move { h.drop_session(&sid).await });
        }
    }

    /// Delete a Server unless something uses it; its secret leaves the
    /// Keychain.
    pub fn delete_server(&mut self, server_id: &str, cx: &mut Context<Self>) -> Result<(), ServerError> {
        let users = self.server_users(server_id);
        let removed = self.remote.servers.delete(server_id, users)?;
        let _ = self.remote.servers.save_to(&servers_path());
        *self.remote.shared_servers.lock().unwrap() = self.remote.servers.clone();
        self.remote.secrets.delete(&removed.secret_account());
        self.remote.picker_state.forget_server(server_id);
        self.save_picker_state();
        cx.notify();
        Ok(())
    }

    /// The Remote Files of the open Log Space that wait for the user: the
    /// first Server needing a secret, or the files to Reconnect.
    pub fn connect_button(&self) -> Option<(String, ConnectAction)> {
        let space = self.active_space()?;
        if space.kind != LogSpaceKind::Remote {
            return None;
        }
        for file in &space.files {
            let Some(source) = self.live.sources.get(&file.file_id) else { continue };
            if source.status == SourceStatus::NeedsSecret {
                let server_id = file.remote.as_ref()?.server_id.clone();
                return Some(("Connect".into(), ConnectAction::Secret(server_id)));
            }
        }
        let stuck = space.files.iter().any(|f| {
            self.live
                .sources
                .get(&f.file_id)
                .is_some_and(|s| matches!(s.status, SourceStatus::Disconnected | SourceStatus::Error(_) | SourceStatus::SignInRequired(_)))
        });
        stuck.then(|| ("Reconnect".into(), ConnectAction::Reconnect(space.id.clone())))
    }

    /// Press Connect or Reconnect.
    pub fn press_connect(&mut self, action: ConnectAction, cx: &mut Context<Self>) {
        match action {
            ConnectAction::Secret(server_id) => self.open_secret_prompt(&server_id, cx),
            ConnectAction::Reconnect(space_id) => {
                let files: Vec<String> = self
                    .active_project()
                    .and_then(|p| p.log_space(&space_id))
                    .map(|s| s.files.iter().map(|f| f.file_id.clone()).collect())
                    .unwrap_or_default();
                for id in files {
                    if let Some(source) = self.live.sources.get(&id) {
                        if matches!(source.status, SourceStatus::Disconnected | SourceStatus::Error(_) | SourceStatus::SignInRequired(_)) {
                            source.handle.send(SourceCommand::Reconnect);
                        }
                    }
                }
            }
        }
        cx.notify();
    }

    /// The source of one Remote File.
    pub(crate) fn spawn_remote_source(
        &mut self,
        server_id: &str,
        path: &str,
        name: &str,
        from_glob: Option<&str>,
        start_time: Option<String>,
        cx: &mut Context<Self>,
    ) -> Option<crate::live::SourceHandle> {
        self.remote.servers.get(server_id)?;
        let hub = self.remote_hub(cx);
        let glob_inodes = from_glob.map(|id| self.live.globs.inodes_of(id));
        Some(
            RemoteFileSource {
                hub,
                servers: self.remote.shared_servers.clone(),
                server_id: server_id.to_string(),
                path: path.to_string(),
                name: name.to_string(),
                window: self.live.window,
                start_time,
                glob_inodes,
            }
            .spawn(),
        )
    }
}

impl AppView {
    /// Start reading the output of a Remote Command.
    pub(crate) fn spawn_command_source(
        &mut self,
        server_id: &str,
        target: &RemoteTarget,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Option<crate::live::SourceHandle> {
        let RemoteTarget::Command { command, preset, restart_on_exit } = target else { return None };
        self.remote.servers.get(server_id)?;
        let hub = self.remote_hub(cx);
        Some(
            RemoteCommandSource {
                hub,
                servers: self.remote.shared_servers.clone(),
                server_id: server_id.to_string(),
                command: command.clone(),
                preset: crate::remote::command::unedited_preset(command, preset.as_ref()),
                restart_on_exit: *restart_on_exit,
                name: name.to_string(),
                window: self.live.window,
            }
            .spawn(),
        )
    }
}

/// What the Connect button does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectAction {
    /// Ask for the Server's password or passphrase.
    Secret(String),
    /// Try again.
    Reconnect(String),
}

impl AppView {
    pub fn add_remote_files_dialog(&mut self, space_id: &str, cx: &mut Context<Self>) {
        self.open_remote_picker(super::remote_picker::PickerTarget::AddTo(space_id.to_string()), cx);
    }
}

impl AppView {
    /// Drop a remote file chosen for the New Log Space.
    pub fn new_space_remove_remote(&mut self, index: usize, cx: &mut Context<Self>) {
        let files = self.remote.new_space_files.len();
        if index < files {
            self.remote.new_space_files.remove(index);
        } else if index - files < self.remote.new_space_globs.len() {
            self.remote.new_space_globs.remove(index - files);
        }
        cx.notify();
    }
}

/// The Rotated Files next to the files about to be downloaded, to tick.
pub struct DownloadOptions {
    pub space_name: String,
    pub files: Vec<crate::workspace::log_space::LogFile>,
    /// Per file, its Rotated Files newest first and which are ticked.
    pub rotated: Vec<Vec<crate::remote::rotated::RotatedFile>>,
    pub ticked: Vec<Vec<bool>>,
}

/// A download in progress, or just finished.
pub struct DownloadState {
    /// The Log Space the files came from (names the new Imported Log Space).
    pub space_name: String,
    pub dir: PathBuf,
    /// Local names, in order.
    pub names: Vec<String>,
    pub progress: Arc<crate::remote::download::Progress>,
    /// `None` while running.
    pub outcome: Option<Result<Vec<String>, String>>,
}

impl AppView {
    pub fn download_file(&mut self, space_id: &str, file_id: &str, cx: &mut Context<Self>) {
        let Some(space) = self.active_project().and_then(|p| p.log_space(space_id)).cloned() else { return };
        let files: Vec<_> = space.files.iter().filter(|f| f.file_id == file_id).cloned().collect();
        self.begin_download(space.name, files, cx);
    }

    pub fn download_space(&mut self, space_id: &str, cx: &mut Context<Self>) {
        let Some(space) = self.active_project().and_then(|p| p.log_space(space_id)).cloned() else { return };
        // A file that is Gone from its Server cannot be downloaded, and a
        // Remote Command has no file to download: say so before downloading.
        let (commands, rest): (Vec<_>, Vec<_>) = space.files.iter().cloned().partition(|f| f.is_remote_command());
        if !commands.is_empty() {
            self.notice = Some(match commands.len() {
                1 => "1 remote command will be skipped".to_string(),
                n => format!("{n} remote commands will be skipped"),
            });
            cx.notify();
        }
        let files: Vec<_> = rest
            .into_iter()
            .filter(|f| {
                self.live.sources.get(&f.file_id).is_none_or(|s| s.status != crate::live::SourceStatus::Gone)
            })
            .collect();
        self.begin_download(space.name, files, cx);
    }

    /// Look for the Rotated Files of the files; when there are some, the
    /// user ticks the ones to take along (none by default), then picks the
    /// folder.
    fn begin_download(&mut self, space_name: String, files: Vec<crate::workspace::log_space::LogFile>, cx: &mut Context<Self>) {
        if files.is_empty() || self.remote.download.is_some() || self.remote.download_options.is_some() {
            return;
        }
        let hub = self.remote_hub(cx);
        let store = self.remote.servers.clone();
        let targets: Vec<Option<(String, String)>> = files
            .iter()
            .map(|f| {
                let r = f.remote.as_ref()?;
                Some((r.server_id.clone(), r.path()?.to_string()))
            })
            .collect();
        let task = hub.spawn({
            let hub = hub.clone();
            async move {
                let mut found = Vec::new();
                for target in targets {
                    let Some((server_id, path)) = target else {
                        found.push(Vec::new());
                        continue;
                    };
                    let listed = async {
                        let session = hub.session(&store, &server_id).await.map_err(|e| e.message())?;
                        let sftp = session.open_sftp().await?;
                        crate::remote::rotated::list_rotated(&sftp, &path).await
                    }
                    .await;
                    // A folder that cannot be listed just has none to offer.
                    found.push(listed.unwrap_or_default());
                }
                found
            }
        });
        cx.spawn(async move |this, cx| {
            let rotated = task.await.unwrap_or_default();
            this.update(cx, |this, cx| {
                if rotated.iter().all(Vec::is_empty) {
                    this.pick_folder_and_download(space_name, files, Vec::new(), cx);
                    return;
                }
                let ticked = rotated.iter().map(|r| vec![false; r.len()]).collect();
                this.remote.download_options = Some(DownloadOptions { space_name, files, rotated, ticked });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle_download_rotated(&mut self, file: usize, rotated: usize, cx: &mut Context<Self>) {
        if let Some(options) = self.remote.download_options.as_mut() {
            if let Some(t) = options.ticked.get_mut(file).and_then(|r| r.get_mut(rotated)) {
                *t = !*t;
            }
        }
        cx.notify();
    }

    pub fn cancel_download_options(&mut self, cx: &mut Context<Self>) {
        self.remote.download_options = None;
        cx.notify();
    }

    /// The ticks are made: pick the folder and download.
    pub fn confirm_download_options(&mut self, cx: &mut Context<Self>) {
        let Some(options) = self.remote.download_options.take() else { return };
        let mut extras = Vec::new();
        for (fi, file) in options.files.iter().enumerate() {
            let Some(remote) = file.remote.as_ref() else { continue };
            let Some(path) = remote.path() else { continue };
            let folder = crate::remote::browse::dirname(path);
            for (ri, rotated) in options.rotated[fi].iter().enumerate() {
                if options.ticked[fi][ri] {
                    extras.push((remote.server_id.clone(), crate::remote::browse::join(&folder, &rotated.name)));
                }
            }
        }
        self.pick_folder_and_download(options.space_name, options.files, extras, cx);
        cx.notify();
    }

    fn pick_folder_and_download(
        &mut self,
        space_name: String,
        files: Vec<crate::workspace::log_space::LogFile>,
        extras: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        if files.is_empty() || self.remote.download.is_some() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await else { return };
            let dir = folder.path().to_path_buf();
            this.update(cx, |this, cx| this.start_download(space_name, files, extras, dir, cx)).ok();
        })
        .detach();
    }

    /// Download the files, and the Rotated Files `extras` (`(server id,
    /// path)`), into `dir`; a Rotated File keeps its name and its compression.
    pub fn start_download(
        &mut self,
        space_name: String,
        files: Vec<crate::workspace::log_space::LogFile>,
        extras: Vec<(String, String)>,
        dir: PathBuf,
        cx: &mut Context<Self>,
    ) {
        use crate::remote::download::{DownloadError, Progress, download_file, download_names};
        use std::sync::atomic::Ordering;
        let mut jobs: Vec<(String, String, String)> = files
            .iter()
            .filter_map(|f| {
                // A Remote Command has no file to download.
                let r = f.remote.as_ref()?;
                let server = self.remote.servers.get(&r.server_id)?;
                Some((r.server_id.clone(), server.name.clone(), r.path()?.to_string()))
            })
            .collect();
        for (server_id, path) in extras {
            if let Some(server) = self.remote.servers.get(&server_id) {
                jobs.push((server_id, server.name.clone(), path));
            }
        }
        let existing: std::collections::HashSet<String> = std::fs::read_dir(&dir)
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
            .unwrap_or_default();
        let pairs: Vec<(String, String)> = jobs.iter().map(|(_, s, p)| (s.clone(), p.clone())).collect();
        let names = download_names(&pairs, &existing);
        let progress = Arc::new(Progress::default());
        self.remote.download = Some(DownloadState {
            space_name,
            dir: dir.clone(),
            names: names.clone(),
            progress: progress.clone(),
            outcome: None,
        });
        let hub = self.remote_hub(cx);
        let store = self.remote.servers.clone();
        let task = hub.spawn({
            let (hub, progress, dir, names) = (hub.clone(), progress.clone(), dir, names);
            async move {
                // Size first, so the bar has a total.
                let mut sessions = Vec::new();
                let mut total = 0u64;
                for (server_id, _, path) in &jobs {
                    let session = hub.session(&store, server_id).await.map_err(|e| e.message())?;
                    let sftp = session.open_sftp().await?;
                    if let Ok(meta) = sftp.metadata(path.clone()).await {
                        total += meta.len();
                    }
                    sessions.push(sftp);
                }
                progress.total.store(total, Ordering::Relaxed);
                let mut saved = Vec::new();
                for (i, ((_, _, path), sftp)) in jobs.iter().zip(&sessions).enumerate() {
                    let dest = dir.join(&names[i]);
                    match download_file(sftp, path, &dest, &progress).await {
                        Ok(_) => saved.push(dest.to_string_lossy().to_string()),
                        Err(DownloadError::Cancelled) => return Err("Download cancelled".to_string()),
                        Err(DownloadError::Failed(e)) => return Err(e),
                    }
                    progress.files_done.fetch_add(1, Ordering::Relaxed);
                }
                Ok(saved)
            }
        });
        // Redraw while it runs, then take the result.
        cx.spawn(async move |this, cx| {
            let mut task = task;
            let outcome = loop {
                let tick = cx.background_executor().timer(std::time::Duration::from_millis(200));
                if let Some(done) = race(&mut task, tick).await {
                    break match done {
                        Ok(result) => result,
                        Err(e) => Err(e.to_string()),
                    };
                }
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
            };
            this.update(cx, |this, cx| {
                if let Some(download) = this.remote.download.as_mut() {
                    download.outcome = Some(outcome);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn cancel_download(&mut self, cx: &mut Context<Self>) {
        if let Some(download) = &self.remote.download {
            download.progress.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        cx.notify();
    }

    /// Close the download panel; with `open`, the files become an Imported
    /// Log Space.
    pub fn finish_download(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(download) = self.remote.download.take() else { return };
        if open {
            if let Some(Ok(paths)) = download.outcome {
                let base = format!("{} (downloaded)", download.space_name);
                let mut name = base.clone();
                let mut n = 2;
                while self.active_project().is_some_and(|p| p.has_space_named(&name)) {
                    name = format!("{base} {n}");
                    n += 1;
                }
                // A compressed Rotated File opens as an Archive of one file.
                let sources: Vec<crate::workspace::log_space::ImportSource> = paths
                    .into_iter()
                    .map(|path| match crate::archive::detect(std::path::Path::new(&path)) {
                        Some(kind) if kind.is_single_file() => {
                            let entry = crate::archive::gz_entry_name(std::path::Path::new(&path));
                            crate::workspace::log_space::ImportSource::Entry { archive: path, entry }
                        }
                        _ => crate::workspace::log_space::ImportSource::Disk(path),
                    })
                    .collect();
                self.create_log_space(&name, LogSpaceKind::Imported, &sources, cx);
            }
        }
        cx.notify();
    }
}

/// Wait for `a` or for `tick`, whichever comes first: `Some(output)` when `a`
/// finished. `a` is polled to its end only once, so its output is not lost.
async fn race<A, B>(a: &mut A, tick: B) -> Option<A::Output>
where
    A: std::future::Future + Unpin,
    B: std::future::Future,
{
    use std::pin::pin;
    use std::task::Poll;
    let mut tick = pin!(tick);
    std::future::poll_fn(|cx| {
        if let Poll::Ready(output) = std::pin::Pin::new(&mut *a).poll(cx) {
            return Poll::Ready(Some(output));
        }
        if tick.as_mut().poll(cx).is_ready() {
            return Poll::Ready(None);
        }
        Poll::Pending
    })
    .await
}
