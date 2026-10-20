//! Keyboard-interactive sign-in against an in-process russh server, so no
//! sshd with PAM is needed: a key followed by a one-time code, and the code
//! alone. Also the ssh-agent sign-in against a throwaway `ssh-agent`.

use log_gpui::remote::*;
use russh::server::{Auth, Msg, Response, Server as _, Session};
use russh::{Channel, MethodKind, MethodSet};
use std::borrow::Cow;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

const CODE: &str = "123456";

#[derive(Clone)]
struct Otp {
    /// Whether the public key alone is a partial success.
    key_first: bool,
}

impl russh::server::Server for Otp {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        self.clone()
    }
}

impl russh::server::Handler for Otp {
    type Error = russh::Error;

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host_to_connect: &str,
        port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let target = format!("{host_to_connect}:{port_to_connect}");
        let Ok(mut upstream) = tokio::net::TcpStream::connect(target).await else { return Ok(()) };
        reply.accept().await;
        tokio::spawn(async move {
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut upstream).await;
        });
        Ok(())
    }

    async fn auth_publickey(&mut self, _user: &str, _key: &russh::keys::PublicKey) -> Result<Auth, Self::Error> {
        if self.key_first {
            Ok(Auth::Reject {
                proceed_with_methods: Some(MethodSet::from(&[MethodKind::KeyboardInteractive][..])),
                partial_success: true,
            })
        } else {
            Ok(Auth::reject())
        }
    }

    async fn auth_keyboard_interactive<'a>(
        &'a mut self,
        _user: &str,
        _submethods: &str,
        response: Option<Response<'a>>,
    ) -> Result<Auth, Self::Error> {
        match response {
            None => Ok(Auth::Partial {
                name: Cow::Borrowed("Two-factor"),
                instructions: Cow::Borrowed("Use your authenticator"),
                prompts: Cow::Owned(vec![(Cow::Borrowed("Verification code:"), false)]),
            }),
            Some(mut answers) => match answers.next() {
                Some(a) if a.as_ref() == CODE.as_bytes() => Ok(Auth::Accept),
                _ => Ok(Auth::reject()),
            },
        }
    }
}

fn keygen(dir: &Path, name: &str) -> String {
    let path = dir.join(name);
    let status = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", ""])
        .arg("-f")
        .arg(&path)
        .status()
        .expect("ssh-keygen");
    assert!(status.success());
    path.display().to_string()
}

/// Starts the server on the hub's runtime; returns its port.
fn start_server(hub: &RemoteHub, dir: &Path, key_first: bool) -> u16 {
    let host_key = russh::keys::load_secret_key(keygen(dir, "hostkey"), None).unwrap();
    let config = Arc::new(russh::server::Config {
        methods: MethodSet::from(&[MethodKind::PublicKey, MethodKind::KeyboardInteractive][..]),
        auth_rejection_time: std::time::Duration::from_millis(1),
        auth_rejection_time_initial: Some(std::time::Duration::from_millis(0)),
        keys: vec![host_key],
        ..Default::default()
    });
    let listener = hub.handle().block_on(tokio::net::TcpListener::bind(("127.0.0.1", 0))).unwrap();
    let port = listener.local_addr().unwrap().port();
    hub.spawn(async move {
        let mut server = Otp { key_first };
        let _ = server.run_on_socket(config, &listener).await;
    });
    port
}

/// A second server, behind the first, that wants only the key.
fn start_plain_server(hub: &RemoteHub, dir: &Path) -> u16 {
    let host_key = russh::keys::load_secret_key(keygen(dir, "hostkey2"), None).unwrap();
    let config = Arc::new(russh::server::Config {
        methods: MethodSet::from(&[MethodKind::PublicKey][..]),
        auth_rejection_time: std::time::Duration::from_millis(1),
        auth_rejection_time_initial: Some(std::time::Duration::from_millis(0)),
        keys: vec![host_key],
        ..Default::default()
    });
    let listener = hub.handle().block_on(tokio::net::TcpListener::bind(("127.0.0.1", 0))).unwrap();
    let port = listener.local_addr().unwrap().port();
    hub.spawn(async move {
        let mut server = Plain;
        let _ = server.run_on_socket(config, &listener).await;
    });
    port
}

#[derive(Clone)]
struct Plain;

impl russh::server::Server for Plain {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        Plain
    }
}

impl russh::server::Handler for Plain {
    type Error = russh::Error;

    async fn auth_publickey(&mut self, _user: &str, _key: &russh::keys::PublicKey) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }
}

struct Rig {
    hub: Arc<RemoteHub>,
    _dir: tempfile::TempDir,
    port: u16,
    key: String,
}

fn rig(key_first: bool) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let key = keygen(dir.path(), "userkey");
    let (hub, host_prompts) = RemoteHub::new(Arc::new(MemorySecrets::default()), Arc::new(Mutex::new(KnownHosts::default())));
    std::thread::spawn(move || {
        while let Ok(request) = host_prompts.recv_blocking() {
            let _ = request.reply.send(true);
        }
    });
    let port = start_server(&hub, dir.path(), key_first);
    Rig { hub, _dir: dir, port, key }
}

/// Answers every Sign-in Prompt with `answer`, or cancels on `None`; counts
/// them and keeps what was asked.
fn answer_sign_ins(hub: &RemoteHub, answer: Option<&'static str>) -> Arc<Mutex<Vec<(String, String, Vec<(String, bool)>)>>> {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let (requests, log) = (hub.sign_in_requests(), asked.clone());
    std::thread::spawn(move || {
        while let Ok(request) = requests.recv_blocking() {
            log.lock().unwrap().push((request.server_name.clone(), request.instructions.clone(), request.prompts.clone()));
            let _ = request.reply.send(answer.map(|a| vec![a.to_string()]));
        }
    });
    asked
}

fn store_with(server: Server) -> (ServerStore, String) {
    let mut store = ServerStore::default();
    let id = store.save(server).unwrap();
    (store, id)
}

fn connect(rig: &Rig, store: &ServerStore, id: &str, interactive: bool) -> Result<(), ConnectError> {
    let (hub, store, id) = (rig.hub.clone(), store.clone(), id.to_string());
    rig.hub.handle().block_on(async move { hub.session_with(&store, &id, interactive).await.map(|_| ()) })
}

fn key_server(rig: &Rig) -> Server {
    let mut server = Server::new("web1", "127.0.0.1", "me", AuthMethod::Key { path: rig.key.clone() });
    server.port = rig.port;
    server
}

#[test]
fn a_key_that_is_only_a_partial_success_goes_on_to_the_one_time_code() {
    let rig = rig(true);
    let asked = answer_sign_ins(&rig.hub, Some(CODE));
    let (store, id) = store_with(key_server(&rig));
    assert_eq!(connect(&rig, &store, &id, true), Ok(()));
    let asked = asked.lock().unwrap();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].0, "web1");
    assert_eq!(asked[0].1, "Use your authenticator");
    assert_eq!(asked[0].2, vec![("Verification code:".to_string(), false)]);
    assert!(rig.hub.needs_sign_in_prompt(store.get(&id).unwrap()));
}

#[test]
fn a_wrong_code_is_an_authentication_failure_not_a_wrong_key() {
    let rig = rig(true);
    answer_sign_ins(&rig.hub, Some("000000"));
    let (store, id) = store_with(key_server(&rig));
    let result = connect(&rig, &store, &id, true);
    assert!(matches!(result, Err(ConnectError::AuthFailed { .. })), "{result:?}");
}

#[test]
fn cancelling_the_prompt_waits_for_connect() {
    let rig = rig(true);
    answer_sign_ins(&rig.hub, None);
    let (store, id) = store_with(key_server(&rig));
    let result = connect(&rig, &store, &id, true);
    assert!(matches!(result, Err(ConnectError::SignInRequired { .. })), "{result:?}");
}

#[test]
fn a_connection_made_by_itself_does_not_ask() {
    let rig = rig(true);
    let asked = answer_sign_ins(&rig.hub, Some(CODE));
    let mut server = key_server(&rig);
    // The last connection needed a prompt.
    server.needs_sign_in_prompt = true;
    let (store, id) = store_with(server);
    let result = connect(&rig, &store, &id, false);
    assert!(matches!(result, Err(ConnectError::SignInRequired { .. })), "{result:?}");
    assert!(asked.lock().unwrap().is_empty());
    // Pressing Connect asks.
    assert_eq!(connect(&rig, &store, &id, true), Ok(()));
    assert_eq!(asked.lock().unwrap().len(), 1);
}

#[test]
fn an_unexpected_prompt_without_the_user_is_not_asked() {
    let rig = rig(true);
    let asked = answer_sign_ins(&rig.hub, Some(CODE));
    let (store, id) = store_with(key_server(&rig));
    let result = connect(&rig, &store, &id, false);
    assert!(matches!(result, Err(ConnectError::SignInRequired { .. })), "{result:?}");
    assert!(asked.lock().unwrap().is_empty());
}

#[test]
fn keyboard_interactive_alone_signs_in() {
    let rig = rig(false);
    answer_sign_ins(&rig.hub, Some(CODE));
    let mut server = Server::new("web1", "127.0.0.1", "me", AuthMethod::KeyboardInteractive);
    server.port = rig.port;
    let (store, id) = store_with(server);
    assert_eq!(connect(&rig, &store, &id, true), Ok(()));
}

#[test]
fn test_chain_reports_each_step_and_closes_the_session() {
    let rig = rig(true);
    answer_sign_ins(&rig.hub, Some(CODE));
    let mut server = key_server(&rig);
    server.id = "test-0".into();
    let steps = Arc::new(Mutex::new(Vec::new()));
    let log = steps.clone();
    let hub = rig.hub.clone();
    let result = rig.hub.handle().block_on(async move {
        hub.test_chain(vec![server], Default::default(), &move |step| log.lock().unwrap().push(step)).await
    });
    assert_eq!(result, Ok(()));
    let steps = steps.lock().unwrap();
    let kinds: Vec<_> = steps.iter().map(|s| s.kind).collect();
    // This server has no SFTP subsystem.
    assert_eq!(kinds, vec![TestStepKind::Tcp, TestStepKind::HostKey, TestStepKind::Auth, TestStepKind::Sftp]);
    assert!(steps[..3].iter().all(|s| s.result.is_ok()));
    assert_eq!(steps[3].result, Err("SFTP not available".to_string()));
}

#[test]
fn test_chain_marks_the_first_failing_step() {
    let rig = rig(true);
    let mut server = key_server(&rig);
    server.id = "test-0".into();
    server.port = 1; // nothing listens here
    let steps = Arc::new(Mutex::new(Vec::new()));
    let log = steps.clone();
    let hub = rig.hub.clone();
    let result = rig.hub.handle().block_on(async move {
        hub.test_chain(vec![server], Default::default(), &move |step| log.lock().unwrap().push(step)).await
    });
    assert!(result.is_err());
    let steps = steps.lock().unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].kind, TestStepKind::Tcp);
    assert!(steps[0].result.is_err());
}

/// A throwaway `ssh-agent` holding the given keys.
struct Agent {
    socket: String,
    pid: String,
}

impl Agent {
    fn start(dir: &Path, keys: &[&str]) -> Agent {
        let socket = dir.join("agent.sock").display().to_string();
        let out = Command::new("ssh-agent").args(["-a", &socket]).output().expect("ssh-agent");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let pid = text
            .split("SSH_AGENT_PID=")
            .nth(1)
            .and_then(|r| r.split(';').next())
            .unwrap_or_default()
            .to_string();
        for key in keys {
            let status = Command::new("ssh-add").arg(key).env("SSH_AUTH_SOCK", &socket).output().expect("ssh-add");
            assert!(status.status.success(), "ssh-add: {}", String::from_utf8_lossy(&status.stderr));
        }
        Agent { socket, pid }
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        let _ = Command::new("kill").arg(&self.pid).status();
    }
}

fn agent_server(rig: &Rig, socket: &str, preferred: Option<String>) -> Server {
    let mut server = Server::new(
        "web1",
        "127.0.0.1",
        "me",
        AuthMethod::Agent { socket: AgentSocket::Path(socket.to_string()), preferred_key: preferred },
    );
    server.port = rig.port;
    server
}

#[test]
fn the_agent_signs_in_and_then_the_one_time_code_follows() {
    let rig = rig(true);
    let asked = answer_sign_ins(&rig.hub, Some(CODE));
    let agent = Agent::start(rig._dir.path(), &[&rig.key]);
    let (store, id) = store_with(agent_server(&rig, &agent.socket, None));
    assert_eq!(connect(&rig, &store, &id, true), Ok(()));
    assert_eq!(asked.lock().unwrap().len(), 1);
}

#[test]
fn a_missing_agent_socket_is_reported_with_its_path() {
    let rig = rig(true);
    let (store, id) = store_with(agent_server(&rig, "/nonexistent/agent.sock", None));
    let result = connect(&rig, &store, &id, true);
    assert_eq!(result.clone().unwrap_err().message(), "ssh-agent not available at /nonexistent/agent.sock", "{result:?}");
}

#[test]
fn an_agent_without_keys_has_nothing_to_accept() {
    let rig = rig(true);
    let agent = Agent::start(rig._dir.path(), &[]);
    let (store, id) = store_with(agent_server(&rig, &agent.socket, None));
    let result = connect(&rig, &store, &id, true);
    assert!(matches!(result, Err(ConnectError::AgentRejected { .. })), "{result:?}");
}

#[test]
fn a_preferred_key_that_the_agent_lacks_is_not_replaced_by_another() {
    let rig = rig(true);
    let other = keygen(rig._dir.path(), "other");
    let agent = Agent::start(rig._dir.path(), &[&rig.key]);
    let (store, id) = store_with(agent_server(&rig, &agent.socket, Some(format!("{other}.pub"))));
    let result = connect(&rig, &store, &id, true);
    assert!(matches!(result, Err(ConnectError::AgentRejected { .. })), "{result:?}");
}

#[test]
fn the_preferred_key_is_found_by_its_pub_file() {
    let rig = rig(true);
    answer_sign_ins(&rig.hub, Some(CODE));
    let other = keygen(rig._dir.path(), "other");
    let agent = Agent::start(rig._dir.path(), &[&other, &rig.key]);
    let (store, id) = store_with(agent_server(&rig, &agent.socket, Some(format!("{}.pub", rig.key))));
    assert_eq!(connect(&rig, &store, &id, true), Ok(()));
}

#[test]
fn only_the_jump_server_asks_and_only_once() {
    let rig = rig(true);
    let asked = answer_sign_ins(&rig.hub, Some(CODE));
    let inner_port = start_plain_server(&rig.hub, rig._dir.path());
    let mut store = ServerStore::default();
    let mut bastion = key_server(&rig);
    bastion.name = "bastion".into();
    let bastion = store.save(bastion).unwrap();
    let mut db = Server::new("db", "127.0.0.1", "me", AuthMethod::Key { path: rig.key.clone() });
    db.port = inner_port;
    db.jump_server_id = Some(bastion);
    let db = store.save(db).unwrap();
    assert_eq!(connect(&rig, &store, &db, true), Ok(()));
    let asked = asked.lock().unwrap();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].0, "bastion");
}

#[test]
fn a_jump_server_that_needs_a_prompt_blocks_reconnecting_by_itself() {
    let rig = rig(true);
    let asked = answer_sign_ins(&rig.hub, Some(CODE));
    let inner_port = start_plain_server(&rig.hub, rig._dir.path());
    let mut store = ServerStore::default();
    let mut bastion = key_server(&rig);
    bastion.name = "bastion".into();
    bastion.needs_sign_in_prompt = true;
    let bastion = store.save(bastion).unwrap();
    let mut db = Server::new("db", "127.0.0.1", "me", AuthMethod::Key { path: rig.key.clone() });
    db.port = inner_port;
    db.jump_server_id = Some(bastion);
    let db = store.save(db).unwrap();
    let result = connect(&rig, &store, &db, false);
    match result {
        Err(ConnectError::SignInRequired { server_name, .. }) => assert_eq!(server_name, "bastion"),
        other => panic!("{other:?}"),
    }
    assert!(asked.lock().unwrap().is_empty());
}

#[test]
fn a_source_waits_for_connect_instead_of_asking_and_asks_only_after_it() {
    use log_gpui::live::{LineSource, RemoteFileSource, SourceCommand, SourceEvent, SourceStatus};
    let rig = rig(true);
    let asked = answer_sign_ins(&rig.hub, Some(CODE));
    let mut server = key_server(&rig);
    server.needs_sign_in_prompt = true;
    let (store, id) = store_with(server);
    let handle = RemoteFileSource {
        glob_inodes: None,
        hub: rig.hub.clone(),
        servers: Arc::new(Mutex::new(store)),
        server_id: id,
        path: "/var/log/app.log".into(),
        name: "web1:/var/log/app.log".into(),
        window: 100,
        start_time: None,
    }
    .spawn();
    let wait = |want: &dyn Fn(&SourceEvent) -> bool| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            match handle.events.try_recv() {
                Ok(e) if want(&e) => return,
                Ok(_) => {}
                Err(_) => {
                    assert!(std::time::Instant::now() < deadline, "timed out");
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
    };
    wait(&|e| matches!(e, SourceEvent::Status(SourceStatus::SignInRequired(name)) if name == "web1"));
    assert!(asked.lock().unwrap().is_empty());
    handle.send(SourceCommand::Reconnect);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while asked.lock().unwrap().is_empty() {
        assert!(std::time::Instant::now() < deadline, "no prompt after Reconnect");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    handle.close();
}
