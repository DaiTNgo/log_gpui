//! `RemoteHub`: the SSH connections. It owns its own tokio runtime (russh
//! needs one; gpui has its own executor). A Server has one session shared by
//! all its Remote Files; a Server behind Jump Servers is reached through
//! `direct-tcpip` channels, each hop checked and logged in on its own.

use super::health::{HealthBoard, ServerState};
use super::known_hosts::{HostKeyStatus, KnownHosts};
use super::secrets::SecretStore;
use super::server::{AuthMethod, Server, ServerStore};
use russh::client::{self, Handle};
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate, load_secret_key};
use russh_sftp::client::SftpSession;
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

/// Why a connection could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    /// A password or passphrase is needed and not in the Keychain.
    NeedsSecret { server_id: String, server_name: String },
    AuthFailed { server_name: String },
    /// The host key is not the trusted one.
    HostKeyChanged { address: String },
    /// The user did not trust a new host key.
    HostKeyRejected { address: String },
    Network(String),
    Other(String),
}

impl ConnectError {
    pub fn message(&self) -> String {
        match self {
            ConnectError::NeedsSecret { server_name, .. } => format!("Password needed for {server_name}"),
            ConnectError::AuthFailed { .. } => "Authentication failed".into(),
            ConnectError::HostKeyChanged { .. } => "Host key changed".into(),
            ConnectError::HostKeyRejected { .. } => "Host key not trusted".into(),
            ConnectError::Network(e) | ConnectError::Other(e) => e.clone(),
        }
    }
}

/// A host key never seen before; the app asks the user and answers.
pub struct HostKeyRequest {
    pub server_name: String,
    pub address: String,
    /// `SHA256:...`
    pub fingerprint: String,
    pub reply: tokio::sync::oneshot::Sender<bool>,
}

struct ClientHandler {
    address: String,
    server_name: String,
    known: Arc<Mutex<KnownHosts>>,
    prompts: async_channel::Sender<HostKeyRequest>,
    /// Why the key was refused, read after a failed connect.
    refused: Arc<Mutex<Option<ConnectError>>>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        let PublicKeyOrCertificate::PublicKey { key, .. } = key else {
            return Ok(false);
        };
        let fingerprint = key.fingerprint(HashAlg::Sha256).to_string();
        let status = self.known.lock().unwrap().check(&self.address, &fingerprint);
        match status {
            HostKeyStatus::Trusted => Ok(true),
            HostKeyStatus::Changed { .. } => {
                *self.refused.lock().unwrap() =
                    Some(ConnectError::HostKeyChanged { address: self.address.clone() });
                Ok(false)
            }
            HostKeyStatus::Unknown => {
                let (reply, answer) = tokio::sync::oneshot::channel();
                let request = HostKeyRequest {
                    server_name: self.server_name.clone(),
                    address: self.address.clone(),
                    fingerprint: fingerprint.clone(),
                    reply,
                };
                if self.prompts.send(request).await.is_err() {
                    return Ok(false);
                }
                if answer.await.unwrap_or(false) {
                    self.known.lock().unwrap().trust(&self.address, &fingerprint);
                    Ok(true)
                } else {
                    *self.refused.lock().unwrap() =
                        Some(ConnectError::HostKeyRejected { address: self.address.clone() });
                    Ok(false)
                }
            }
        }
    }
}

/// A live connection to one Server (and the hops in front of it).
pub struct RemoteSession {
    pub server: Server,
    /// When this session was made.
    connected_at: Instant,
    /// Commands running through the hub's sessions, counted until their
    /// channel is closed.
    runs: Arc<AtomicUsize>,
    handle: Handle<ClientHandler>,
    /// The sessions the channel to this one runs through.
    _hops: Vec<Handle<ClientHandler>>,
}

/// What `exec` returned.
#[derive(Debug, Clone, Default)]
pub struct ExecOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_status: Option<u32>,
}

impl RemoteSession {
    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    pub async fn open_sftp(&self) -> Result<SftpSession, String> {
        let channel = self.handle.channel_open_session().await.map_err(|e| e.to_string())?;
        channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
        SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())
    }

    /// Run a command and collect its output.
    pub async fn exec(&self, command: &str) -> Result<ExecOutput, String> {
        let mut channel = self.handle.channel_open_session().await.map_err(|e| e.to_string())?;
        channel.exec(true, command).await.map_err(|e| e.to_string())?;
        let mut out = ExecOutput::default();
        while let Some(msg) = channel.wait().await {
            match msg {
                russh::ChannelMsg::Data { data } => out.stdout.extend_from_slice(&data),
                russh::ChannelMsg::ExtendedData { data, ext: 1 } => out.stderr.extend_from_slice(&data),
                russh::ChannelMsg::ExitStatus { exit_status } => out.exit_status = Some(exit_status),
                _ => {}
            }
        }
        Ok(out)
    }

    /// Start a command and read its output as it comes: no terminal, and its
    /// input is closed at once so a command that reads it sees the end.
    pub async fn exec_stream(&self, command: &str) -> Result<ExecStream, String> {
        let channel = self.handle.channel_open_session().await.map_err(|e| e.to_string())?;
        channel.exec(true, command).await.map_err(|e| e.to_string())?;
        channel.eof().await.map_err(|e| e.to_string())?;
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(ExecStream { channel, exit_status: None, ended: false, runs: self.runs.clone() })
    }
}

/// How long a command gets to end after `signal TERM` before its channel is
/// closed anyway.
pub const TERMINATE_GRACE: Duration = Duration::from_secs(2);

/// What a running command produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecEvent {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
    /// The command ended (or the channel closed): its exit status, `None`
    /// when it was killed by a signal or the connection dropped. Last event.
    Exited(Option<u32>),
}

/// A command running on a Server, read through an exec channel.
pub struct ExecStream {
    channel: russh::Channel<russh::client::Msg>,
    exit_status: Option<u32>,
    ended: bool,
    runs: Arc<AtomicUsize>,
}

impl Drop for ExecStream {
    fn drop(&mut self) {
        self.runs.fetch_sub(1, Ordering::SeqCst);
    }
}

impl ExecStream {
    /// The next piece of output, then `Exited`, then `None`.
    pub async fn next(&mut self) -> Option<ExecEvent> {
        if self.ended {
            return None;
        }
        loop {
            match self.channel.wait().await {
                Some(russh::ChannelMsg::Data { data }) => return Some(ExecEvent::Stdout(data.to_vec())),
                // Extended data type 1 is stderr.
                Some(russh::ChannelMsg::ExtendedData { data, ext: 1 }) => {
                    return Some(ExecEvent::Stderr(data.to_vec()));
                }
                Some(russh::ChannelMsg::ExitStatus { exit_status }) => self.exit_status = Some(exit_status),
                Some(russh::ChannelMsg::Close) | None => {
                    self.ended = true;
                    return Some(ExecEvent::Exited(self.exit_status));
                }
                Some(_) => {}
            }
        }
    }

    /// Ask the command to stop (`signal TERM`), give it `TERMINATE_GRACE` to
    /// end, then close the channel. `true` when the command was seen to end;
    /// `false` when it may still be running on the Server.
    pub async fn terminate(mut self) -> bool {
        if self.ended {
            return true;
        }
        let _ = self.channel.signal(russh::Sig::TERM).await;
        let ended = tokio::time::timeout(TERMINATE_GRACE, async {
            while let Some(event) = self.next().await {
                if matches!(event, ExecEvent::Exited(_)) {
                    return true;
                }
            }
            true
        })
        .await
        .unwrap_or(false);
        let _ = self.channel.close().await;
        ended
    }
}

pub struct RemoteHub {
    runtime: Option<Runtime>,
    secrets: Arc<dyn SecretStore>,
    known_hosts: Arc<Mutex<KnownHosts>>,
    prompts: async_channel::Sender<HostKeyRequest>,
    /// One session per Server id, shared by every file on it.
    sessions: Arc<tokio::sync::Mutex<HashMap<String, Arc<RemoteSession>>>>,
    /// Secrets typed this run and not remembered.
    typed_secrets: Arc<Mutex<HashMap<String, String>>>,
    /// Connection state, latency and errors per Server.
    health: HealthBoard,
    /// Commands (Runs) not yet ended on a Server.
    runs: Arc<AtomicUsize>,
    /// Held while one Server is being connected, so files that ask at the
    /// same time share one session (and one host key question).
    connecting: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl RemoteHub {
    pub fn new(
        secrets: Arc<dyn SecretStore>,
        known_hosts: Arc<Mutex<KnownHosts>>,
    ) -> (Arc<Self>, async_channel::Receiver<HostKeyRequest>) {
        let (prompts, requests) = async_channel::unbounded();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("remote-hub")
            .enable_all()
            .build()
            .expect("tokio runtime");
        let hub = Arc::new(Self {
            runtime: Some(runtime),
            secrets,
            known_hosts,
            prompts,
            sessions: Arc::default(),
            typed_secrets: Arc::default(),
            health: HealthBoard::new(),
            runs: Arc::default(),
            connecting: Mutex::default(),
        });
        (hub, requests)
    }

    /// How many commands are still running through the hub's sessions: a
    /// Run counts until `terminate` has finished with it.
    pub fn running_commands(&self) -> usize {
        self.runs.load(Ordering::SeqCst)
    }

    /// The health of every Server (`changes()` says when it changed).
    pub fn health(&self) -> &HealthBoard {
        &self.health
    }

    fn rt(&self) -> &Runtime {
        self.runtime.as_ref().expect("runtime lives until drop")
    }

    /// Run a future on the hub's runtime (gpui cannot poll tokio's I/O).
    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.rt().spawn(future)
    }

    pub fn handle(&self) -> tokio::runtime::Handle {
        self.rt().handle().clone()
    }

    pub fn known_hosts(&self) -> Arc<Mutex<KnownHosts>> {
        self.known_hosts.clone()
    }

    /// A password or passphrase typed by the user for this run; kept in the
    /// Keychain too when the Server asks to remember it.
    pub fn provide_secret(&self, server: &Server, secret: &str) {
        self.typed_secrets.lock().unwrap().insert(server.id.clone(), secret.to_string());
        if server.remember_secret {
            let _ = self.secrets.set(&server.secret_account(), secret);
        }
    }

    fn secret_for(&self, server: &Server) -> Option<String> {
        self.typed_secrets
            .lock()
            .unwrap()
            .get(&server.id)
            .cloned()
            .or_else(|| self.secrets.get(&server.secret_account()))
    }

    /// Close and forget the session of a Server (it was edited or removed):
    /// the files on it lose their connection and connect again with the
    /// Server's new settings.
    pub async fn drop_session(&self, server_id: &str) {
        let session = self.sessions.lock().await.remove(server_id);
        if let Some(session) = session {
            let _ = session.handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
        }
        self.health.update(server_id, |h| {
            h.clear_session();
            if h.state == ServerState::Connected {
                h.state = ServerState::Disconnected;
            }
        });
    }

    /// The shared session to a Server, connecting if needed.
    pub async fn session(&self, store: &ServerStore, server_id: &str) -> Result<Arc<RemoteSession>, ConnectError> {
        if let Some(session) = self.live_session(server_id).await {
            return Ok(session);
        }
        // One connection at a time per Server; those who waited use its session.
        let gate = self.connecting.lock().unwrap().entry(server_id.to_string()).or_default().clone();
        let _connecting = gate.lock().await;
        if let Some(session) = self.live_session(server_id).await {
            return Ok(session);
        }
        let chain: Vec<Server> = store
            .chain(server_id)
            .map_err(|e| ConnectError::Other(e.message()))?
            .into_iter()
            .cloned()
            .collect();
        self.health.update(server_id, |h| {
            h.state = if h.ever_connected { ServerState::Reconnecting } else { ServerState::Connecting };
        });
        let session = match self.connect_chain(&chain).await {
            Ok(session) => Arc::new(session),
            Err(e) => {
                self.health.update(server_id, |h| {
                    h.state = match e {
                        ConnectError::NeedsSecret { .. }
                        | ConnectError::HostKeyChanged { .. }
                        | ConnectError::HostKeyRejected { .. }
                        | ConnectError::AuthFailed { .. } => ServerState::Waiting,
                        _ => ServerState::Disconnected,
                    };
                    h.record_error(e.message());
                });
                return Err(e);
            }
        };
        let since = session.connected_at;
        let server = session.server.clone();
        let hops: Vec<String> = chain[..chain.len() - 1].iter().map(|s| s.name.clone()).collect();
        self.health.update(server_id, |h| {
            if h.ever_connected {
                h.reconnects += 1;
            }
            h.ever_connected = true;
            h.state = ServerState::Connected;
            h.connected_since = Some(since);
            h.clear_samples();
            h.keepalive = (Duration::from_secs(u64::from(server.keepalive_interval_secs)), server.keepalive_max_missed);
            h.hops = hops;
        });
        self.sessions.lock().await.insert(server_id.to_string(), session.clone());
        self.spawn_latency(&session, server_id.to_string());
        Ok(session)
    }

    async fn live_session(&self, server_id: &str) -> Option<Arc<RemoteSession>> {
        let sessions = self.sessions.lock().await;
        sessions.get(server_id).filter(|s| !s.is_closed()).cloned()
    }

    /// Measure the round trip of the session every keepalive interval, by a
    /// `keepalive@openssh.com` global request that waits for its reply
    /// (`REQUEST_FAILURE` is a reply too). Ends with the session.
    fn spawn_latency(&self, session: &Arc<RemoteSession>, server_id: String) {
        let weak = Arc::downgrade(session);
        let board = self.health.clone();
        let interval = Duration::from_secs(u64::from(session.server.keepalive_interval_secs));
        let since = session.connected_at;
        self.spawn(async move {
            let lost = |board: &HealthBoard, why: &str| {
                board.update(&server_id, |h| {
                    // Only the session this task watches may be marked lost.
                    if h.connected_since == Some(since) {
                        h.clear_session();
                        h.state = ServerState::Disconnected;
                        h.record_error(why);
                    }
                });
            };
            let mut wait = Duration::from_secs(1);
            loop {
                tokio::time::sleep(wait).await;
                wait = interval;
                let Some(session) = weak.upgrade() else { return };
                if session.is_closed() {
                    lost(&board, "Connection lost");
                    return;
                }
                let started = Instant::now();
                let reply = tokio::time::timeout(
                    interval,
                    session.handle.send_global_request("keepalive@openssh.com", &[], true),
                )
                .await;
                match reply {
                    Ok(Ok(_)) | Ok(Err(russh::Error::RequestDenied)) => {
                        let round_trip = started.elapsed();
                        board.update(&server_id, |h| {
                            if h.connected_since == Some(since) {
                                h.record_latency(round_trip);
                            }
                        });
                    }
                    Ok(Err(e)) => {
                        lost(&board, &e.to_string());
                        return;
                    }
                    // No answer yet: the session's own keepalive decides.
                    Err(_) => {}
                }
            }
        });
    }

    async fn connect_chain(&self, chain: &[Server]) -> Result<RemoteSession, ConnectError> {
        let mut hops: Vec<Handle<ClientHandler>> = Vec::new();
        for (i, server) in chain.iter().enumerate() {
            // Each hop keeps itself alive with its own Server's settings.
            let config = Arc::new(client::Config {
                keepalive_interval: Some(Duration::from_secs(u64::from(server.keepalive_interval_secs))),
                keepalive_max: server.keepalive_max_missed as usize,
                nodelay: true,
                ..Default::default()
            });
            let refused = Arc::new(Mutex::new(None));
            let handler = ClientHandler {
                address: server.address(),
                server_name: server.name.clone(),
                known: self.known_hosts.clone(),
                prompts: self.prompts.clone(),
                refused: refused.clone(),
            };
            let connecting = match hops.last() {
                None => client::connect(config.clone(), (server.host.as_str(), server.port), handler).await,
                Some(previous) => {
                    let channel = previous
                        .channel_open_direct_tcpip(server.host.clone(), u32::from(server.port), "127.0.0.1", 0)
                        .await
                        .map_err(|e| ConnectError::Network(format!("{}: {e}", server.name)))?;
                    client::connect_stream(config.clone(), channel.into_stream(), handler).await
                }
            };
            let mut handle = match connecting {
                Ok(handle) => handle,
                Err(e) => {
                    if let Some(refusal) = refused.lock().unwrap().take() {
                        return Err(refusal);
                    }
                    return Err(ConnectError::Network(format!("{}: {e}", server.name)));
                }
            };
            self.authenticate(&mut handle, server).await?;
            if i + 1 == chain.len() {
                return Ok(RemoteSession { server: server.clone(), connected_at: Instant::now(), runs: self.runs.clone(), handle, _hops: hops });
            }
            hops.push(handle);
        }
        Err(ConnectError::Other("empty chain".into()))
    }

    async fn authenticate(&self, handle: &mut Handle<ClientHandler>, server: &Server) -> Result<(), ConnectError> {
        let needs_secret = || ConnectError::NeedsSecret { server_id: server.id.clone(), server_name: server.name.clone() };
        let result = match &server.auth {
            AuthMethod::Password => {
                let password = self.secret_for(server).ok_or_else(needs_secret)?;
                handle
                    .authenticate_password(server.user.clone(), password)
                    .await
                    .map_err(|e| ConnectError::Network(e.to_string()))?
            }
            AuthMethod::Key { path } => {
                let passphrase = self.secret_for(server);
                let key = match load_secret_key(path, passphrase.as_deref()) {
                    Ok(key) => key,
                    Err(e) => {
                        // An encrypted key without its passphrase asks for it.
                        let text = e.to_string().to_lowercase();
                        if passphrase.is_none() && (text.contains("encrypted") || text.contains("passphrase")) {
                            return Err(needs_secret());
                        }
                        let unreadable = text.contains("no such file") || text.contains("permission") || text.contains("not found");
                        if passphrase.is_some() && !unreadable {
                            return Err(ConnectError::AuthFailed { server_name: server.name.clone() });
                        }
                        return Err(ConnectError::Other(format!("Could not read the key {path}: {e}")));
                    }
                };
                let hash = handle
                    .best_supported_rsa_hash()
                    .await
                    .map_err(|e| ConnectError::Network(e.to_string()))?
                    .flatten();
                handle
                    .authenticate_publickey(server.user.clone(), PrivateKeyWithHashAlg::new(Arc::new(key), hash))
                    .await
                    .map_err(|e| ConnectError::Network(e.to_string()))?
            }
        };
        if result.success() {
            Ok(())
        } else {
            // A wrong typed secret is forgotten so the user is asked again.
            self.typed_secrets.lock().unwrap().remove(&server.id);
            Err(ConnectError::AuthFailed { server_name: server.name.clone() })
        }
    }
}

impl Drop for RemoteHub {
    fn drop(&mut self) {
        // The last reference may be dropped by a task on the runtime itself,
        // where waiting for the runtime to stop is not allowed.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
