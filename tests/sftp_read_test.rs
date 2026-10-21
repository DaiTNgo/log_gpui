//! SFTP reads transfer only what they use, against a local sshd
//! (`SSH_TEST_DIR`, `SSH_TEST_PORT`). Bytes are counted on the SFTP channels
//! of the hub (`RemoteHub::sftp_bytes`).

use log_gpui::remote::compress::Compression;
use log_gpui::remote::copy;
use log_gpui::remote::full_read::{GenerationPlan, ReadControl, ReadEvent, ReadPlan, find_start_offset, run};
use log_gpui::remote::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Env {
    hub: Arc<RemoteHub>,
    store: ServerStore,
    server_id: String,
    dir: tempfile::TempDir,
    copies: tempfile::TempDir,
}

fn env() -> Option<Env> {
    let key_dir = std::env::var("SSH_TEST_DIR").ok()?;
    let port: u16 = std::env::var("SSH_TEST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(2222);
    let user = std::env::var("USER").ok()?;
    let mut server = Server::new("web1", "127.0.0.1", &user, AuthMethod::Key { path: format!("{key_dir}/userkey") });
    server.port = port;
    let mut store = ServerStore::default();
    let server_id = store.save(server).unwrap();
    let (hub, requests) = RemoteHub::new(Arc::new(MemorySecrets::default()), Arc::new(Mutex::new(KnownHosts::default())));
    std::thread::spawn(move || {
        while let Ok(req) = requests.recv_blocking() {
            let _ = req.reply.send(true);
        }
    });
    Some(Env { hub, store, server_id, dir: tempfile::tempdir().unwrap(), copies: tempfile::tempdir().unwrap() })
}

fn stamp(i: u32) -> String {
    format!("2026-10-19 {:02}:{:02}:{:02}.{:03} INFO line {i} {}\n", i / 3600 % 24, i / 60 % 60, i % 60, i % 1000, "x".repeat(60))
}

fn big_file(env: &Env, lines: u32) -> (String, u64) {
    let path = env.dir.path().join("app.log").to_string_lossy().to_string();
    std::fs::write(&path, (0..lines).map(stamp).collect::<String>()).unwrap();
    let size = std::fs::metadata(&path).unwrap().len();
    (path, size)
}

fn plan(env: &Env, path: &str, size: u64, start: Option<f64>, to: Option<f64>) -> ReadPlan {
    ReadPlan {
        server_id: env.server_id.clone(),
        server_name: "web1".into(),
        path: path.into(),
        generations: vec![GenerationPlan {
            distance: 0,
            name: "app.log".into(),
            path: path.into(),
            size,
            modified: None,
            compression: Compression::None,
            start_byte: None,
            skip_lines: 0,
            older_dropped: false,
        }],
        start,
        to,
        start_text: None,
        to_text: None,
        limit_lines: 2_000_000,
        dir: copy::create_dir(env.copies.path()).unwrap(),
    }
}

fn read_all(env: &Env, plan: ReadPlan) {
    let (tx, rx) = async_channel::unbounded();
    env.hub.spawn(run(env.hub.clone(), Arc::new(Mutex::new(env.store.clone())), plan, tx, Arc::new(ReadControl::default())));
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        match rx.try_recv() {
            Ok(ReadEvent::Done(_)) => return,
            Ok(ReadEvent::Failed { .. }) => panic!("read failed"),
            Ok(_) => {}
            Err(_) => {
                assert!(std::time::Instant::now() < deadline, "timed out");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

#[test]
fn a_full_read_transfers_about_the_size_of_the_file() {
    let Some(env) = env() else { return };
    let (path, size) = big_file(&env, 180_000);
    assert!(size > 16_000_000);
    read_all(&env, plan(&env, &path, size, None, None));
    let got = env.hub.sftp_bytes();
    assert!(got >= size, "read {got} of {size}");
    assert!(got < size + size / 5, "transferred {got} bytes for a file of {size}");
}

#[test]
fn a_starting_time_search_transfers_far_less_than_the_file() {
    let Some(env) = env() else { return };
    let (path, size) = big_file(&env, 180_000);
    // Search near the end: the read of the range itself is small.
    let t = log_gpui::live::line_time::line_timestamp(&stamp(86_000)).unwrap();
    let to = log_gpui::live::line_time::line_timestamp(&stamp(86_100)).unwrap();
    read_all(&env, plan(&env, &path, size, Some(t), Some(to)));
    let got = env.hub.sftp_bytes();
    assert!(got < 2_500_000, "transferred {got} bytes for a search in a file of {size}");
}

#[test]
fn random_reads_transfer_about_what_they_ask() {
    let Some(env) = env() else { return };
    let (path, size) = big_file(&env, 180_000);
    let hub = env.hub.clone();
    let (store, id) = (env.store.clone(), env.server_id.clone());
    let wanted = log_gpui::live::line_time::line_timestamp(&stamp(40_000)).unwrap();
    let h = env.hub.spawn(async move {
        let session = hub.session(&store, &id).await.unwrap();
        let sftp = session.open_sftp_random().await.unwrap();
        let file = sftp.open(path).await.unwrap();
        let handle = tokio::runtime::Handle::current();
        let before = hub.sftp_bytes();
        tokio::task::spawn_blocking(move || {
            let mut src = log_gpui::remote::full_read::SftpReadAt { handle, file };
            find_start_offset(&mut src, size, wanted).unwrap();
        })
        .await
        .unwrap();
        hub.sftp_bytes() - before
    });
    let probes = loop {
        if h.is_finished() {
            break env.hub.handle().block_on(h).unwrap();
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    // About 15 probes of 64 KB, and one packet of slack for each.
    assert!(probes < 2_500_000, "transferred {probes} bytes");
}

#[test]
fn a_live_tail_and_backfill_transfer_about_the_lines_they_show() {
    use log_gpui::live::{LineSource, RemoteFileSource, SourceEvent};
    let Some(env) = env() else { return };
    let (path, size) = big_file(&env, 180_000);
    let h = RemoteFileSource {
        glob_inodes: None,
        hub: env.hub.clone(),
        servers: Arc::new(Mutex::new(env.store.clone())),
        server_id: env.server_id.clone(),
        path: path.clone(),
        name: format!("web1:{path}"),
        window: 8_000,
        start_time: None,
    }
    .spawn();
    let (mut loaded, deadline) = (0usize, std::time::Instant::now() + Duration::from_secs(30));
    while loaded < 8_000 {
        match h.events.try_recv() {
            Ok(SourceEvent::Tail { lines, .. }) | Ok(SourceEvent::Backfill { lines, .. }) => loaded += lines.len(),
            Ok(_) => {}
            Err(_) => {
                assert!(std::time::Instant::now() < deadline, "timed out");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    h.close();
    // 8,000 lines of about 100 bytes, and one 64 KB packet of slack per block.
    let got = env.hub.sftp_bytes();
    assert!(got < 2_000_000, "transferred {got} bytes for 8,000 lines of a file of {size}");
}
