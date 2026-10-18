//! `RemoteHub`: the SSH connections. It owns its own tokio runtime (russh
//! needs one; gpui has its own executor). A Server has one session shared by
//! all its Remote Files; a Server behind Jump Servers is reached through
//! `direct-tcpip` channels, each hop checked and logged in on its own.

use super::known_hosts::{HostKeyStatus, KnownHosts};
use super::secrets::SecretStore;
use super::server::{AuthMethod, Server, ServerStore};
use russh::client::{self, Handle};
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate, load_secret_key};
use russh_sftp::client::SftpSession;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Runtime;

/// How often a session is kept alive.
pub const KEEPALIVE: Duration = Duration::from_secs(30);

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
    handle: Handle<ClientHandler>,
    /// The sessions the channel to this one runs through.
    _hops: Vec<Handle<ClientHandler>>,
}

/// What `exec` returned.
#[derive(Debug, Clone, Default)]
pub struct ExecOutput {
    pub stdout: Vec<u8>,
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
                russh::ChannelMsg::ExitStatus { exit_status } => out.exit_status = Some(exit_status),
                _ => {}
            }
        }
        Ok(out)
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
        });
        (hub, requests)
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
    }

    /// The shared session to a Server, connecting if needed.
    pub async fn session(&self, store: &ServerStore, server_id: &str) -> Result<Arc<RemoteSession>, ConnectError> {
        {
            let sessions = self.sessions.lock().await;
            if let Some(s) = sessions.get(server_id) {
                if !s.is_closed() {
                    return Ok(s.clone());
                }
            }
        }
        let chain: Vec<Server> = store
            .chain(server_id)
            .map_err(|e| ConnectError::Other(e.message()))?
            .into_iter()
            .cloned()
            .collect();
        let session = Arc::new(self.connect_chain(&chain).await?);
        self.sessions.lock().await.insert(server_id.to_string(), session.clone());
        Ok(session)
    }

    async fn connect_chain(&self, chain: &[Server]) -> Result<RemoteSession, ConnectError> {
        let config = Arc::new(client::Config {
            keepalive_interval: Some(KEEPALIVE),
            keepalive_max: 3,
            nodelay: true,
            ..Default::default()
        });
        let mut hops: Vec<Handle<ClientHandler>> = Vec::new();
        for (i, server) in chain.iter().enumerate() {
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
                return Ok(RemoteSession { server: server.clone(), handle, _hops: hops });
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
