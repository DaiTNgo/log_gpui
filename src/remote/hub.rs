//! `RemoteHub`: the SSH connections. It owns its own tokio runtime (russh
//! needs one; gpui has its own executor). A Server has one session shared by
//! all its Remote Files; a Server behind Jump Servers is reached through
//! `direct-tcpip` channels, each hop checked and logged in on its own.

use super::health::{HealthBoard, ServerState};
use super::known_hosts::{HostKeyStatus, KnownHosts};
use super::secrets::SecretStore;
use super::server::{AgentTarget, AuthMethod, Server, ServerStore};
use russh::client::{self, Handle, KeyboardInteractiveAuthResponse};
use russh::keys::agent::AgentIdentity;
use russh::keys::agent::client::{AgentClient, AgentStream};
use russh::keys::{PublicKey, load_public_key};
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate, load_secret_key};
use russh::MethodKind;
use russh_sftp::client::SftpSession;
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
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
    /// The Server asks questions (Sign-in Prompts) and the user is not
    /// there to answer, or cancelled the dialog: waits for Connect.
    SignInRequired { server_id: String, server_name: String },
    /// The ssh-agent socket is missing or does not answer.
    AgentUnavailable { server_name: String, path: String },
    /// No key of the agent was accepted (or the agent did not sign).
    AgentRejected { server_name: String, detail: String },
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
            ConnectError::SignInRequired { server_name, .. } => format!("Sign-in required: {server_name}"),
            ConnectError::AgentUnavailable { path, .. } => format!("ssh-agent not available at {path}"),
            ConnectError::AgentRejected { detail, .. } => format!("Authentication failed: {detail}"),
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

/// A question the Server asks while signing in (keyboard-interactive); the
/// app shows it and answers. `reply` gets `None` when the user cancels.
pub struct SignInRequest {
    pub server_id: String,
    pub server_name: String,
    /// The name and instructions the server sent (either may be empty).
    pub name: String,
    pub instructions: String,
    /// Each question and whether the answer is shown while typing.
    pub prompts: Vec<(String, bool)>,
    pub reply: tokio::sync::oneshot::Sender<Option<Vec<String>>>,
}

/// One step of Test connection, for one Server of the chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStepKind {
    Tcp,
    HostKey,
    Auth,
    Sftp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestStep {
    /// Index in the chain, outermost Jump Server first.
    pub hop: usize,
    pub server_name: String,
    pub kind: TestStepKind,
    pub result: Result<String, String>,
    pub elapsed: Duration,
}

type Reporter<'a> = Option<&'a (dyn Fn(TestStep) + Send + Sync)>;

/// Most keys of an agent tried for one sign-in (a server cuts the connection
/// after `MaxAuthTries`, 6 by default).
const MAX_AGENT_KEYS: usize = 5;
/// How long the agent gets to sign (a touch, a 1Password or Touch ID approval).
const AGENT_SIGN_TIMEOUT: Duration = Duration::from_secs(60);

/// Overwrite a secret in place before it is dropped.
fn wipe(secret: &mut String) {
    // SAFETY: writing zeros keeps the string valid UTF-8.
    unsafe { secret.as_bytes_mut() }.fill(0);
    secret.clear();
}

struct ClientHandler {
    /// Set once the Server's host key was presented (the TCP connection works).
    key_checked: Arc<std::sync::atomic::AtomicBool>,
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
        self.key_checked.store(true, Ordering::SeqCst);
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
    /// Bytes received on the SFTP channels of the hub.
    sftp_bytes: Arc<AtomicU64>,
    /// Processes started through this session.
    execs: AtomicUsize,
    handle: Handle<ClientHandler>,
    /// The sessions the channel to this one runs through.
    _hops: Vec<Handle<ClientHandler>>,
}

/// Largest read of a random-access SFTP session.
const RANDOM_READ_LEN: u32 = 64 * 1024;

/// A stream that counts what it receives.
struct CountingStream<S> {
    inner: S,
    count: Arc<AtomicU64>,
}

impl<S: AsyncRead + Unpin> AsyncRead for CountingStream<S> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let poll = std::pin::Pin::new(&mut self.inner).poll_read(cx, buf);
        if poll.is_ready() {
            self.count.fetch_add((buf.filled().len() - before) as u64, Ordering::Relaxed);
        }
        poll
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for CountingStream<S> {
    fn poll_write(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &[u8]) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// What `exec` returned.
#[derive(Debug, Clone, Default)]
pub struct ExecOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_status: Option<u32>,
}

impl RemoteSession {
    /// How many processes this session started on the Server.
    pub fn exec_count(&self) -> usize {
        self.execs.load(Ordering::SeqCst)
    }

    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    /// An SFTP session for reading a file from front to back: reads are
    /// pipelined, so it must not be moved around (every seek throws away the
    /// requests in flight, which the Server still sends).
    pub async fn open_sftp(&self) -> Result<SftpSession, String> {
        self.open_sftp_with(russh_sftp::client::Config::default()).await
    }

    /// An SFTP session for reading blocks here and there (backward blocks,
    /// timestamp probes): one request at a time and of one block at most,
    /// so a read transfers about what it asks for.
    pub async fn open_sftp_random(&self) -> Result<SftpSession, String> {
        self.open_sftp_with(russh_sftp::client::Config {
            max_packet_len: RANDOM_READ_LEN + 13,
            max_concurrent_reads: 1,
            ..Default::default()
        })
        .await
    }

    async fn open_sftp_with(&self, cfg: russh_sftp::client::Config) -> Result<SftpSession, String> {
        let stream = CountingStream { inner: self.open_sftp_stream().await?, count: self.sftp_bytes.clone() };
        SftpSession::new_with_config(stream, cfg).await.map_err(|e| e.to_string())
    }

    /// The byte stream of a new `sftp` subsystem channel (for a raw SFTP
    /// session that wants to choose its own read sizes).
    pub async fn open_sftp_stream(&self) -> Result<russh::ChannelStream<client::Msg>, String> {
        let channel = self.handle.channel_open_session().await.map_err(|e| e.to_string())?;
        channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
        Ok(channel.into_stream())
    }

    /// Run a command and collect its output.
    pub async fn exec(&self, command: &str) -> Result<ExecOutput, String> {
        self.execs.fetch_add(1, Ordering::SeqCst);
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
        self.execs.fetch_add(1, Ordering::SeqCst);
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
    sign_ins: async_channel::Sender<SignInRequest>,
    sign_in_requests: async_channel::Receiver<SignInRequest>,
    /// Servers that needed Sign-in Prompts at their last connection (from
    /// the saved flag, updated on every successful connection).
    prompted: Mutex<HashMap<String, bool>>,
    /// One session per Server id, shared by every file on it.
    sessions: Arc<tokio::sync::Mutex<HashMap<String, Arc<RemoteSession>>>>,
    /// Secrets typed this run and not remembered.
    typed_secrets: Arc<Mutex<HashMap<String, String>>>,
    /// Connection state, latency and errors per Server.
    health: HealthBoard,
    /// Commands (Runs) not yet ended on a Server.
    runs: Arc<AtomicUsize>,
    /// Bytes received on the SFTP channels.
    sftp_bytes: Arc<AtomicU64>,
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
        let (sign_ins, sign_in_requests) = async_channel::unbounded();
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
            sign_ins,
            sign_in_requests,
            prompted: Mutex::default(),
            sessions: Arc::default(),
            typed_secrets: Arc::default(),
            health: HealthBoard::new(),
            runs: Arc::default(),
            sftp_bytes: Arc::default(),
            connecting: Mutex::default(),
        });
        (hub, requests)
    }

    /// The Sign-in Prompts to show the user (answer each through `reply`).
    pub fn sign_in_requests(&self) -> async_channel::Receiver<SignInRequest> {
        self.sign_in_requests.clone()
    }

    /// Whether the Server needed Sign-in Prompts last time (the app saves
    /// this in the Server).
    pub fn needs_sign_in_prompt(&self, server: &Server) -> bool {
        self.prompted.lock().unwrap().get(&server.id).copied().unwrap_or(server.needs_sign_in_prompt)
    }

    /// Bytes received on the SFTP channels of this hub so far.
    pub fn sftp_bytes(&self) -> u64 {
        self.sftp_bytes.load(Ordering::Relaxed)
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

    /// Forget a secret the Server refused, typed or remembered, so the user
    /// is asked again instead of it being tried over and over.
    fn forget_secret(&self, server: &Server) {
        self.typed_secrets.lock().unwrap().remove(&server.id);
        self.secrets.delete(&server.secret_account());
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
        self.session_with(store, server_id, true).await
    }

    /// Like `session`; with `interactive` false a Server that needs Sign-in
    /// Prompts is not asked but reported as `SignInRequired` (a connection
    /// made by itself must not pop up questions).
    pub async fn session_with(
        &self,
        store: &ServerStore,
        server_id: &str,
        interactive: bool,
    ) -> Result<Arc<RemoteSession>, ConnectError> {
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
        let session = match self.connect_chain(&chain, server_id, interactive, None).await {
            Ok(session) => Arc::new(session),
            Err(e) => {
                self.health.update(server_id, |h| {
                    h.state = match e {
                        ConnectError::NeedsSecret { .. }
                        | ConnectError::HostKeyChanged { .. }
                        | ConnectError::HostKeyRejected { .. }
                        | ConnectError::SignInRequired { .. }
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

    async fn connect_chain(
        &self,
        chain: &[Server],
        target_id: &str,
        interactive: bool,
        report: Reporter<'_>,
    ) -> Result<RemoteSession, ConnectError> {
        if !interactive {
            // Before any network traffic: nobody is there to answer.
            if let Some(server) = chain.iter().find(|s| self.needs_sign_in_prompt(s)) {
                return Err(ConnectError::SignInRequired { server_id: server.id.clone(), server_name: server.name.clone() });
            }
        }
        let say = |hop: usize, server: &Server, kind: TestStepKind, started: Instant, result: Result<String, String>| {
            if let Some(report) = report {
                report(TestStep { hop, server_name: server.name.clone(), kind, result, elapsed: started.elapsed() });
            }
        };
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
            let key_checked = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let handler = ClientHandler {
                key_checked: key_checked.clone(),
                address: server.address(),
                server_name: server.name.clone(),
                known: self.known_hosts.clone(),
                prompts: self.prompts.clone(),
                refused: refused.clone(),
            };
            let started = Instant::now();
            let connecting = match hops.last() {
                None => client::connect(config.clone(), (server.host.as_str(), server.port), handler).await,
                Some(previous) => {
                    let channel = match previous
                        .channel_open_direct_tcpip(server.host.clone(), u32::from(server.port), "127.0.0.1", 0)
                        .await
                    {
                        Ok(channel) => channel,
                        Err(e) => {
                            say(i, server, TestStepKind::Tcp, started, Err(e.to_string()));
                            return Err(ConnectError::Network(format!("{}: {e}", server.name)));
                        }
                    };
                    client::connect_stream(config.clone(), channel.into_stream(), handler).await
                }
            };
            let mut handle = match connecting {
                Ok(handle) => handle,
                Err(e) => {
                    let refusal = refused.lock().unwrap().take();
                    if key_checked.load(Ordering::SeqCst) {
                        say(i, server, TestStepKind::Tcp, started, Ok("connected".into()));
                        let why = refusal.as_ref().map_or_else(|| e.to_string(), ConnectError::message);
                        say(i, server, TestStepKind::HostKey, started, Err(why));
                    } else {
                        say(i, server, TestStepKind::Tcp, started, Err(network_text(&e)));
                    }
                    if let Some(refusal) = refusal {
                        return Err(refusal);
                    }
                    return Err(ConnectError::Network(format!("{}: {}", server.name, network_text(&e))));
                }
            };
            say(i, server, TestStepKind::Tcp, started, Ok("connected".into()));
            say(i, server, TestStepKind::HostKey, started, Ok("host key trusted".into()));
            let auth_started = Instant::now();
            match self.authenticate(&mut handle, server, target_id, interactive).await {
                Ok(()) => say(i, server, TestStepKind::Auth, auth_started, Ok("signed in".into())),
                Err(e) => {
                    say(i, server, TestStepKind::Auth, auth_started, Err(e.message()));
                    return Err(e);
                }
            }
            if i + 1 == chain.len() {
                return Ok(RemoteSession { server: server.clone(), connected_at: Instant::now(), runs: self.runs.clone(), sftp_bytes: self.sftp_bytes.clone(), execs: AtomicUsize::new(0), handle, _hops: hops });
            }
            hops.push(handle);
        }
        Err(ConnectError::Other("empty chain".into()))
    }

    /// Try the Server's own sign-in, then whatever the Server still asks for
    /// (keyboard-interactive after a key or the agent).
    async fn authenticate(
        &self,
        handle: &mut Handle<ClientHandler>,
        server: &Server,
        target_id: &str,
        interactive: bool,
    ) -> Result<(), ConnectError> {
        let auth_failed = || ConnectError::AuthFailed { server_name: server.name.clone() };
        let result = match &server.auth {
            AuthMethod::Password => self.auth_password(handle, server).await?,
            AuthMethod::Key { path } => self.auth_key(handle, server, path).await?,
            AuthMethod::Agent { socket, preferred_key } => {
                self.auth_agent(handle, server, target_id, &socket.target(), &socket.describe(), preferred_key.as_deref())
                    .await?
            }
            AuthMethod::KeyboardInteractive => {
                self.auth_keyboard_interactive(handle, server, interactive).await?;
                self.prompted.lock().unwrap().insert(server.id.clone(), true);
                return Ok(());
            }
        };
        match result {
            russh::client::AuthResult::Success => {
                self.prompted.lock().unwrap().insert(server.id.clone(), false);
                Ok(())
            }
            russh::client::AuthResult::Failure { remaining_methods, partial_success: true }
                if remaining_methods.contains(&MethodKind::KeyboardInteractive) =>
            {
                self.auth_keyboard_interactive(handle, server, interactive).await?;
                self.prompted.lock().unwrap().insert(server.id.clone(), true);
                Ok(())
            }
            russh::client::AuthResult::Failure { .. } => {
                if server.has_secret() {
                    self.forget_secret(server);
                }
                Err(auth_failed())
            }
        }
    }

    async fn auth_password(
        &self,
        handle: &mut Handle<ClientHandler>,
        server: &Server,
    ) -> Result<russh::client::AuthResult, ConnectError> {
        let password = self.secret_for(server).ok_or_else(|| needs_secret(server))?;
        handle
            .authenticate_password(server.user.clone(), password)
            .await
            .map_err(|e| ConnectError::Network(e.to_string()))
    }

    async fn auth_key(
        &self,
        handle: &mut Handle<ClientHandler>,
        server: &Server,
        path: &str,
    ) -> Result<russh::client::AuthResult, ConnectError> {
        let passphrase = self.secret_for(server);
        let key = match load_secret_key(path, passphrase.as_deref()) {
            Ok(key) => key,
            Err(e) => {
                // An encrypted key without its passphrase asks for it.
                let text = e.to_string().to_lowercase();
                if passphrase.is_none() && (text.contains("encrypted") || text.contains("passphrase")) {
                    return Err(needs_secret(server));
                }
                let unreadable = text.contains("no such file") || text.contains("permission") || text.contains("not found");
                if passphrase.is_some() && !unreadable {
                    self.forget_secret(server);
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
            .map_err(|e| ConnectError::Network(e.to_string()))
    }

    /// Ask the agent to sign with its keys, one after another (at most
    /// `MAX_AGENT_KEYS`; the one named by `preferred` alone when set). The
    /// private keys never leave the agent.
    async fn auth_agent(
        &self,
        handle: &mut Handle<ClientHandler>,
        server: &Server,
        target_id: &str,
        socket: &Option<AgentTarget>,
        shown_path: &str,
        preferred: Option<&str>,
    ) -> Result<russh::client::AuthResult, ConnectError> {
        let unavailable = || agent_unavailable(server, shown_path);
        let rejected = |detail: &str| ConnectError::AgentRejected { server_name: server.name.clone(), detail: detail.to_string() };
        let target = socket.as_ref().ok_or_else(unavailable)?;
        let mut agent = connect_agent(target).await.map_err(|_| unavailable())?;
        let identities = agent.request_identities().await.map_err(|_| unavailable())?;
        let mut keys: Vec<PublicKey> = identities
            .into_iter()
            .filter_map(|i| match i {
                AgentIdentity::PublicKey { key, .. } => Some(key),
                AgentIdentity::Certificate { .. } => None,
            })
            .collect();
        if let Some(wanted) = preferred.map(str::trim).filter(|w| !w.is_empty()) {
            let wanted_fingerprint = preferred_fingerprint(wanted);
            keys.retain(|k| Some(k.fingerprint(HashAlg::Sha256).to_string()) == wanted_fingerprint);
            if keys.is_empty() {
                return Err(rejected("the preferred key is not in the agent"));
            }
        }
        if keys.is_empty() {
            return Err(rejected("the agent has no keys"));
        }
        let more_than_tried = keys.len() > MAX_AGENT_KEYS;
        let hash = handle.best_supported_rsa_hash().await.map_err(|e| ConnectError::Network(e.to_string()))?.flatten();
        for key in keys.into_iter().take(MAX_AGENT_KEYS) {
            self.health.update(target_id, |h| h.state = ServerState::WaitingForAgent);
            let signing = tokio::time::timeout(
                AGENT_SIGN_TIMEOUT,
                handle.authenticate_publickey_with(server.user.clone(), key, hash, &mut agent),
            )
            .await;
            self.health.update(target_id, |h| {
                if h.state == ServerState::WaitingForAgent {
                    h.state = ServerState::Connecting;
                }
            });
            match signing {
                Err(_) => return Err(rejected("the agent did not answer within 60 seconds")),
                Ok(Err(e)) => return Err(ConnectError::Network(e.to_string())),
                Ok(Ok(russh::client::AuthResult::Failure { partial_success: false, .. })) => continue,
                Ok(Ok(result)) => return Ok(result),
            }
        }
        Err(rejected(if more_than_tried {
            "no agent key accepted (only the first 5 were tried; set a Preferred key)"
        } else {
            "no agent key accepted"
        }))
    }

    /// Show the Server's questions to the user until it accepts or refuses.
    /// Not interactive, or the dialog cancelled: `SignInRequired`.
    async fn auth_keyboard_interactive(
        &self,
        handle: &mut Handle<ClientHandler>,
        server: &Server,
        interactive: bool,
    ) -> Result<(), ConnectError> {
        let required = || ConnectError::SignInRequired { server_id: server.id.clone(), server_name: server.name.clone() };
        if !interactive {
            return Err(required());
        }
        let network = |e: russh::Error| ConnectError::Network(e.to_string());
        let mut response = handle
            .authenticate_keyboard_interactive_start(server.user.clone(), None)
            .await
            .map_err(network)?;
        loop {
            match response {
                KeyboardInteractiveAuthResponse::Success => return Ok(()),
                KeyboardInteractiveAuthResponse::Failure { .. } => {
                    return Err(ConnectError::AuthFailed { server_name: server.name.clone() });
                }
                KeyboardInteractiveAuthResponse::InfoRequest { name, instructions, prompts } => {
                    if prompts.is_empty() {
                        response = handle.authenticate_keyboard_interactive_respond(Vec::new()).await.map_err(network)?;
                        continue;
                    }
                    let (reply, answer) = tokio::sync::oneshot::channel();
                    let request = SignInRequest {
                        server_id: server.id.clone(),
                        server_name: server.name.clone(),
                        name,
                        instructions,
                        prompts: prompts.into_iter().map(|p| (p.prompt, p.echo)).collect(),
                        reply,
                    };
                    if self.sign_ins.send(request).await.is_err() {
                        return Err(required());
                    }
                    let Some(mut answers) = answer.await.ok().flatten() else { return Err(required()) };
                    response = handle
                        .authenticate_keyboard_interactive_respond(answers.clone())
                        .await
                        .map_err(network)?;
                    answers.iter_mut().for_each(wipe);
                }
            }
        }
    }

    /// Connect through the chain with the values given (not the shared
    /// sessions) and report each step of each Server; the connection is
    /// closed at the end. `secrets` are typed secrets by Server id, kept for
    /// this test only. Host key and Sign-in Prompts are asked as for a real
    /// connection.
    pub async fn test_chain(
        &self,
        chain: Vec<Server>,
        secrets: HashMap<String, String>,
        report: &(dyn Fn(TestStep) + Send + Sync),
    ) -> Result<(), ConnectError> {
        // The typed secrets live only as long as this test, even when it is
        // dropped half way (cancelled).
        struct Typed {
            map: Arc<Mutex<HashMap<String, String>>>,
            ids: Vec<String>,
        }
        impl Drop for Typed {
            fn drop(&mut self) {
                let mut map = self.map.lock().unwrap();
                for id in &self.ids {
                    map.remove(id);
                }
            }
        }
        let _typed = Typed { map: self.typed_secrets.clone(), ids: secrets.keys().cloned().collect() };
        self.typed_secrets.lock().unwrap().extend(secrets);
        let target = chain.last().map(|s| s.id.clone()).unwrap_or_default();
        let outcome = self.connect_chain(&chain, &target, true, Some(report)).await;
        // Keep what the real connection of an existing Server knows apart.
        for server in &chain {
            self.prompted.lock().unwrap().remove(&server.id);
        }
        let session = outcome?;
        let last = chain.len().saturating_sub(1);
        let name = chain[last].name.clone();
        let started = Instant::now();
        let sftp = match session.open_sftp().await {
            Ok(sftp) => {
                let _ = sftp.close().await;
                Ok("SFTP available".to_string())
            }
            Err(_) => Err("SFTP not available".to_string()),
        };
        report(TestStep { hop: last, server_name: name, kind: TestStepKind::Sftp, result: sftp, elapsed: started.elapsed() });
        let _ = session.handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
        Ok(())
    }
}

fn needs_secret(server: &Server) -> ConnectError {
    ConnectError::NeedsSecret { server_id: server.id.clone(), server_name: server.name.clone() }
}

/// An ssh-agent connection, whatever carries it.
type AgentConnection = AgentClient<Box<dyn AgentStream + Send + Unpin>>;

/// Connect to the agent `target` names: a Unix socket path.
#[cfg(unix)]
async fn connect_agent(target: &AgentTarget) -> Result<AgentConnection, russh::keys::Error> {
    match target {
        AgentTarget::Socket(path) => Ok(AgentClient::connect_uds(path).await?.dynamic()),
        AgentTarget::Pageant => Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Pageant is Windows only").into()),
    }
}

/// Connect to the agent `target` names: Pageant, or a named pipe.
#[cfg(windows)]
async fn connect_agent(target: &AgentTarget) -> Result<AgentConnection, russh::keys::Error> {
    let timed_out = |what: &str| -> russh::keys::Error { std::io::Error::new(std::io::ErrorKind::TimedOut, what.to_string()).into() };
    match target {
        AgentTarget::Pageant => {
            match tokio::time::timeout(Duration::from_secs(10), AgentClient::connect_pageant()).await {
                Ok(client) => Ok(client?.dynamic()),
                Err(_) => Err(timed_out("Pageant did not answer")),
            }
        }
        // `connect_named_pipe` retries for as long as the pipe is busy.
        AgentTarget::Socket(pipe) => {
            match tokio::time::timeout(Duration::from_secs(10), AgentClient::connect_named_pipe(pipe)).await {
                Ok(client) => Ok(client?.dynamic()),
                Err(_) => Err(timed_out("the agent pipe stayed busy")),
            }
        }
    }
}

/// The error of an ssh-agent that cannot be used (no socket, no answer, or
/// not supported on this platform).
fn agent_unavailable(server: &Server, shown_path: &str) -> ConnectError {
    ConnectError::AgentUnavailable { server_name: server.name.clone(), path: shown_path.to_string() }
}

/// The SHA256 fingerprint a Preferred key names: a `SHA256:` fingerprint as
/// typed, or the fingerprint of the `.pub` file at the path.
fn preferred_fingerprint(wanted: &str) -> Option<String> {
    if wanted.starts_with("SHA256:") {
        return Some(wanted.to_string());
    }
    let path = super::server::expand_home(wanted);
    load_public_key(path).ok().map(|k| k.fingerprint(HashAlg::Sha256).to_string())
}

/// A connection error in words (refused, not found…), without the library's
/// own prefix.
fn network_text(e: &russh::Error) -> String {
    match e {
        russh::Error::IO(io) if io.kind() == std::io::ErrorKind::ConnectionRefused => "Connection refused".into(),
        other => other.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unusable_agent_says_which_socket_was_tried() {
        let server = Server::new("web1", "10.0.0.1", "deploy", AuthMethod::Agent { socket: Default::default(), preferred_key: None });
        let error = agent_unavailable(&server, r"\\.\pipe\openssh-ssh-agent");
        assert_eq!(error.message(), r"ssh-agent not available at \\.\pipe\openssh-ssh-agent");
    }
}
