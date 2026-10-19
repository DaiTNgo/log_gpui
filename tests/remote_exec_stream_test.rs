//! Needs a local sshd (see `remote_hub_test.rs`): `SSH_TEST_DIR`, `SSH_TEST_PORT`.

use log_gpui::remote::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Env {
    hub: Arc<RemoteHub>,
    store: ServerStore,
    server_id: String,
}

fn env_with(configure: impl FnOnce(&mut Server)) -> Option<Env> {
    let key_dir = std::env::var("SSH_TEST_DIR").ok()?;
    let port: u16 = std::env::var("SSH_TEST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(2222);
    let user = std::env::var("USER").ok()?;
    let mut server = Server::new("web1", "127.0.0.1", &user, AuthMethod::Key { path: format!("{key_dir}/userkey") });
    server.port = port;
    configure(&mut server);
    let mut store = ServerStore::default();
    let server_id = store.save(server).unwrap();
    let (hub, requests) = RemoteHub::new(Arc::new(MemorySecrets::default()), Arc::new(Mutex::new(KnownHosts::default())));
    std::thread::spawn(move || {
        while let Ok(req) = requests.recv_blocking() {
            let _ = req.reply.send(true);
        }
    });
    Some(Env { hub, store, server_id })
}

fn env() -> Option<Env> {
    env_with(|_| {})
}

/// Everything a command printed, until it exited.
async fn collect(stream: &mut ExecStream) -> (String, String, Option<u32>) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    loop {
        match tokio::time::timeout(Duration::from_secs(10), stream.next()).await.expect("no event for 10 s") {
            Some(ExecEvent::Stdout(b)) => out.extend(b),
            Some(ExecEvent::Stderr(b)) => err.extend(b),
            Some(ExecEvent::Exited(code)) => {
                return (String::from_utf8_lossy(&out).into(), String::from_utf8_lossy(&err).into(), code);
            }
            None => panic!("stream ended without Exited"),
        }
    }
}

#[test]
fn exec_stream_keeps_stdout_and_stderr_apart_and_reports_the_exit_code() {
    let Some(env) = env() else { return };
    let hub = env.hub.clone();
    let (out, err, code) = hub.handle().block_on(async {
        let session = hub.session(&env.store, &env.server_id).await.unwrap();
        let mut stream = session.exec_stream("echo out; echo err 1>&2; exit 3").await.unwrap();
        collect(&mut stream).await
    });
    assert_eq!((out.as_str(), err.as_str(), code), ("out\n", "err\n", Some(3)));
}

#[test]
fn exec_stream_closes_the_input_so_a_reading_command_ends() {
    let Some(env) = env() else { return };
    let hub = env.hub.clone();
    let (out, _, code) = hub.handle().block_on(async {
        let session = hub.session(&env.store, &env.server_id).await.unwrap();
        let mut stream = session.exec_stream("cat; echo done").await.unwrap();
        collect(&mut stream).await
    });
    assert_eq!((out.as_str(), code), ("done\n", Some(0)));
}

#[test]
fn terminate_stops_the_process_on_the_server() {
    let Some(env) = env() else { return };
    let hub = env.hub.clone();
    let marker = format!("exec-stream-test-{}", std::process::id());
    let (ended, left) = hub.handle().block_on(async {
        let session = hub.session(&env.store, &env.server_id).await.unwrap();
        // `exec -a` is not portable; a unique argument finds the process.
        let stream = session.exec_stream(&format!("sleep 60 # {marker}")).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        let started = Instant::now();
        let ended = stream.terminate().await;
        assert!(started.elapsed() < Duration::from_secs(3));
        tokio::time::sleep(Duration::from_millis(300)).await;
        let found = session.exec(&format!("pgrep -f '{marker}' | grep -v $$ || true")).await.unwrap();
        (ended, String::from_utf8_lossy(&found.stdout).trim().to_string())
    });
    assert!(ended, "the command was seen to end");
    assert!(left.is_empty(), "no process left: {left:?}");
}

#[test]
fn latency_is_measured_and_a_new_session_counts_as_a_reconnect() {
    let Some(env) = env_with(|s| s.keepalive_interval_secs = 5) else { return };
    let hub = env.hub.clone();
    hub.handle().block_on(async {
        hub.session(&env.store, &env.server_id).await.unwrap();
    });
    let health = hub.health().get(&env.server_id);
    assert_eq!(health.state, ServerState::Connected);
    assert!(health.latency.is_none(), "not measured yet right after connecting");
    assert_eq!(health.keepalive, (Duration::from_secs(5), 3));
    assert_eq!(health.reconnects, 0);

    let deadline = Instant::now() + Duration::from_secs(8);
    while hub.health().get(&env.server_id).latency.is_none() {
        assert!(Instant::now() < deadline, "no latency measured");
        std::thread::sleep(Duration::from_millis(100));
    }
    let measured = hub.health().get(&env.server_id);
    assert!(measured.latency_avg.is_some());
    assert!(measured.latency.unwrap() < Duration::from_secs(1));

    // The session is dropped and made again.
    hub.handle().block_on(async {
        hub.drop_session(&env.server_id).await;
    });
    assert_eq!(hub.health().get(&env.server_id).state, ServerState::Disconnected);
    hub.handle().block_on(async {
        hub.session(&env.store, &env.server_id).await.unwrap();
    });
    let again = hub.health().get(&env.server_id);
    assert_eq!((again.state, again.reconnects), (ServerState::Connected, 1));
    assert!(again.latency.is_none(), "measurements restart with the session");
}
