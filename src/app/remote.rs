//! Logic: Servers, the SSH hub, host key and password prompts, and what
//! connects Remote Files to their sources.

use super::AppView;
use crate::live::{LineSource, RemoteFileSource, SourceCommand, SourceStatus};
use crate::remote::{
    AuthMethod, HostKeyRequest, Keychain, KnownHosts, RemoteHub, SecretStore, Server, ServerError, ServerStore,
};
use crate::workspace::log_space::LogSpaceKind;
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
    pub picker: Option<super::remote_picker::RemotePicker>,
    pub download: Option<DownloadState>,
    /// Remote Files chosen for the New Log Space being made, as
    /// `(server id, server name, path)`.
    pub new_space_files: Vec<(String, String, String)>,
    pub secrets: Arc<dyn SecretStore>,
}

impl RemoteUi {
    pub fn load() -> Self {
        Self::with_paths(&servers_path(), &known_hosts_path(), Arc::new(Keychain))
    }

    pub fn with_paths(servers: &std::path::Path, known: &std::path::Path, secrets: Arc<dyn SecretStore>) -> Self {
        let store = ServerStore::load(servers);
        Self {
            shared_servers: Arc::new(Mutex::new(store.clone())),
            servers: store,
            hub: None,
            known_hosts: Arc::new(Mutex::new(KnownHosts::load(known))),
            host_prompts: VecDeque::new(),
            secret_prompt: None,
            picker: None,
            download: None,
            new_space_files: Vec::new(),
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
        hub
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
                            file.display_name = format!("{}:{}", server.name, remote.path);
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
                .is_some_and(|s| matches!(s.status, SourceStatus::Disconnected | SourceStatus::Error(_)))
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
                        if matches!(source.status, SourceStatus::Disconnected | SourceStatus::Error(_)) {
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
        cx: &mut Context<Self>,
    ) -> Option<crate::live::SourceHandle> {
        self.remote.servers.get(server_id)?;
        let hub = self.remote_hub(cx);
        Some(
            RemoteFileSource {
                hub,
                servers: self.remote.shared_servers.clone(),
                server_id: server_id.to_string(),
                path: path.to_string(),
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
        if index < self.remote.new_space_files.len() {
            self.remote.new_space_files.remove(index);
        }
        cx.notify();
    }
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
        self.pick_folder_and_download(space.name, files, cx);
    }

    pub fn download_space(&mut self, space_id: &str, cx: &mut Context<Self>) {
        let Some(space) = self.active_project().and_then(|p| p.log_space(space_id)).cloned() else { return };
        let files = space.files.clone();
        self.pick_folder_and_download(space.name, files, cx);
    }

    fn pick_folder_and_download(
        &mut self,
        space_name: String,
        files: Vec<crate::workspace::log_space::LogFile>,
        cx: &mut Context<Self>,
    ) {
        if files.is_empty() || self.remote.download.is_some() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await else { return };
            let dir = folder.path().to_path_buf();
            this.update(cx, |this, cx| this.start_download(space_name, files, dir, cx)).ok();
        })
        .detach();
    }

    fn start_download(
        &mut self,
        space_name: String,
        files: Vec<crate::workspace::log_space::LogFile>,
        dir: PathBuf,
        cx: &mut Context<Self>,
    ) {
        use crate::remote::download::{DownloadError, Progress, download_file, download_names};
        use std::sync::atomic::Ordering;
        let jobs: Vec<(String, String, String)> = files
            .iter()
            .filter_map(|f| {
                let r = f.remote.as_ref()?;
                let server = self.remote.servers.get(&r.server_id)?;
                Some((r.server_id.clone(), server.name.clone(), r.path.clone()))
            })
            .collect();
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
            loop {
                let tick = cx.background_executor().timer(std::time::Duration::from_millis(200));
                futures_lite_select(&mut task, tick).await;
                if task.is_finished() {
                    break;
                }
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
            }
            let outcome = match task.await {
                Ok(result) => result,
                Err(e) => Err(e.to_string()),
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
                let sources: Vec<crate::workspace::log_space::ImportSource> =
                    paths.into_iter().map(crate::workspace::log_space::ImportSource::Disk).collect();
                self.create_log_space(&name, LogSpaceKind::Imported, &sources, cx);
            }
        }
        cx.notify();
    }
}

/// Wait for whichever of the two finishes first.
async fn futures_lite_select<A, B>(a: &mut A, b: B)
where
    A: std::future::Future + Unpin,
    B: std::future::Future,
{
    use std::pin::pin;
    use std::task::Poll;
    let mut b = pin!(b);
    std::future::poll_fn(|cx| {
        if std::pin::Pin::new(&mut *a).poll(cx).is_ready() {
            return Poll::Ready(());
        }
        if b.as_mut().poll(cx).is_ready() {
            return Poll::Ready(());
        }
        Poll::Pending
    })
    .await
}
