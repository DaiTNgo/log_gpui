//! Needs a local sshd (see `remote_hub_test.rs`): `SSH_TEST_DIR`, `SSH_TEST_PORT`.

use log_gpui::live::{BreakKind, LineSource, RemoteFileSource, SourceCommand, SourceEvent, SourceHandle};
use log_gpui::live::remote::ResumeFrom;
use log_gpui::remote::*;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Env {
    hub: Arc<RemoteHub>,
    store: ServerStore,
    server_id: String,
    dir: tempfile::TempDir,
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
    Some(Env { hub, store, server_id, dir: tempfile::tempdir().unwrap() })
}

impl Env {
    fn source(&self, name: &str, window: usize) -> (SourceHandle, String) {
        self.source_from(name, window, None)
    }

    fn source_from(&self, name: &str, window: usize, resume: Option<ResumeFrom>) -> (SourceHandle, String) {
        let path = self.dir.path().join(name).to_string_lossy().to_string();
        let handle = RemoteFileSource {
            glob_inodes: None,
            resume,
            hub: self.hub.clone(),
            servers: Arc::new(Mutex::new(self.store.clone())),
            server_id: self.server_id.clone(),
            path: path.clone(),
            name: format!("web1:{path}"),
            window,
            start_time: None,
        }
        .spawn();
        (handle, path)
    }
}

fn next(h: &SourceHandle, mut want: impl FnMut(&SourceEvent) -> bool) -> SourceEvent {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match h.events.try_recv() {
            Ok(e) if want(&e) => return e,
            Ok(_) => {}
            Err(_) => {
                assert!(Instant::now() < deadline, "timed out");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn numbered(range: std::ops::RangeInclusive<usize>) -> String {
    range.map(|i| format!("line {i}\n")).collect()
}

#[test]
fn opens_with_the_tail_then_backfills_up_to_the_window_with_the_real_total() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("big.log");
    std::fs::write(&path, numbered(1..=12_000)).unwrap();
    let (h, _) = env.source("big.log", 8_000);
    let SourceEvent::Tail { lines, .. } = next(&h, |e| matches!(e, SourceEvent::Tail { .. })) else { unreachable!() };
    assert_eq!(lines.len(), 1_000);
    assert_eq!(lines.first().unwrap(), "line 11001");
    assert_eq!(lines.last().unwrap(), "line 12000");
    // The total and the backfill arrive in whatever order.
    let (mut loaded, mut total, mut oldest) = (1_000, None, String::new());
    while loaded < 8_000 || total.is_none() {
        match next(&h, |e| matches!(e, SourceEvent::Backfill { .. } | SourceEvent::TotalLines(_))) {
            SourceEvent::Backfill { lines, .. } => {
                loaded += lines.len();
                oldest = lines.first().cloned().unwrap();
            }
            SourceEvent::TotalLines(t) => total = Some(t),
            _ => {}
        }
    }
    assert_eq!(total, Some(Some(12_000)));
    // Backfill stops at the window (8,000 lines in all), never beyond.
    assert_eq!(loaded, 8_000);
    assert_eq!(oldest, "line 4001");
    std::thread::sleep(Duration::from_millis(1_200));
    while let Ok(e) = h.events.try_recv() {
        assert!(!matches!(e, SourceEvent::Backfill { .. }), "loaded beyond the window");
    }
    h.close();
}

#[test]
fn small_file_shows_every_line_and_new_lines_arrive_without_partial_ones() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("small.log");
    std::fs::write(&path, numbered(1..=500)).unwrap();
    let (h, _) = env.source("small.log", 100_000);
    let SourceEvent::Tail { lines, .. } = next(&h, |e| matches!(e, SourceEvent::Tail { .. })) else { unreachable!() };
    assert_eq!(lines.len(), 500);
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(b"ERROR db").unwrap();
    std::thread::sleep(Duration::from_millis(1_200));
    while let Ok(e) = h.events.try_recv() {
        assert!(!matches!(e, SourceEvent::Append { .. }), "partial line shown: {e:?}");
    }
    f.write_all(b" down\nnext\n").unwrap();
    let SourceEvent::Append { lines, .. } = next(&h, |e| matches!(e, SourceEvent::Append { .. })) else { unreachable!() };
    assert_eq!(lines, vec!["ERROR db down", "next"]);
    h.close();
}

#[test]
fn pause_loses_nothing_and_rotation_truncation_and_deletion_are_breaks() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("app.log");
    std::fs::write(&path, numbered(1..=3)).unwrap();
    let (h, _) = env.source("app.log", 100_000);
    next(&h, |e| matches!(e, SourceEvent::Tail { .. }));
    // Pause, write 5,000 lines, resume.
    h.send(SourceCommand::Pause);
    std::thread::sleep(Duration::from_millis(700));
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(numbered(4..=5_003).as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(1_200));
    while let Ok(e) = h.events.try_recv() {
        assert!(!matches!(e, SourceEvent::Append { .. }), "read while paused");
    }
    h.send(SourceCommand::Resume);
    let mut got = 0;
    while got < 5_000 {
        let SourceEvent::Append { lines, .. } = next(&h, |e| matches!(e, SourceEvent::Append { .. })) else { unreachable!() };
        got += lines.len();
    }
    assert_eq!(got, 5_000);

    // Truncate.
    std::fs::OpenOptions::new().write(true).truncate(true).open(&path).unwrap();
    next(&h, |e| *e == SourceEvent::Break { kind: BreakKind::Truncated });
    std::fs::write(&path, "after truncate\n").unwrap();
    assert_eq!(
        next(&h, |e| matches!(e, SourceEvent::Append { .. })),
        SourceEvent::Append { lines: vec!["after truncate".into()], bytes: vec![15], streams: None }
    );

    // Rotate: the old tail is read first, then the Break, then the new file.
    let mut old = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    old.write_all(b"old tail\n").unwrap();
    std::fs::rename(&path, env.dir.path().join("app.log.1")).unwrap();
    std::fs::write(&path, "fresh\n").unwrap();
    let mut seen = Vec::new();
    while seen.len() < 3 {
        match next(&h, |e| matches!(e, SourceEvent::Append { .. } | SourceEvent::Break { .. })) {
            SourceEvent::Append { lines, .. } => seen.push(lines.join(",")),
            SourceEvent::Break { kind } => seen.push(format!("{kind:?}")),
            _ => {}
        }
    }
    assert_eq!(seen, vec!["old tail", "Rotated", "fresh"]);

    // Delete: a Break and waiting; a new file is a new generation.
    std::fs::remove_file(&path).unwrap();
    next(&h, |e| *e == SourceEvent::Break { kind: BreakKind::Deleted });
    next(&h, |e| matches!(e, SourceEvent::Status(s) if s.label() == "Waiting for file"));
    std::fs::write(&path, "reborn\n").unwrap();
    assert_eq!(
        next(&h, |e| matches!(e, SourceEvent::Append { .. })),
        SourceEvent::Append { lines: vec!["reborn".into()], bytes: vec![7], streams: None }
    );
    h.close();
}

#[test]
fn a_path_that_does_not_exist_yet_waits_and_a_denied_file_reports_it() {
    let Some(env) = env() else { return };
    let (h, path) = env.source("later.log", 100_000);
    next(&h, |e| matches!(e, SourceEvent::Status(s) if s.label() == "Waiting for file"));
    std::fs::write(&path, "hello\n").unwrap();
    let SourceEvent::Tail { lines, .. } = next(&h, |e| matches!(e, SourceEvent::Tail { .. })) else { unreachable!() };
    assert_eq!(lines, vec!["hello"]);
    h.close();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let secret = env.dir.path().join("secret.log");
        std::fs::write(&secret, "x\n").unwrap();
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).unwrap();
        let (h, p) = env.source("secret.log", 100_000);
        let e = next(&h, |e| matches!(e, SourceEvent::Status(s) if s.label().starts_with("Permission denied")));
        assert!(matches!(e, SourceEvent::Status(s) if s.label() == format!("Permission denied: web1:{p}")));
        h.close();
    }
}

#[test]
fn a_dropped_connection_comes_back_and_continues_from_the_same_byte() {
    let Some(env) = env() else { return };
    let key_dir = std::env::var("SSH_TEST_DIR").unwrap();
    let path = env.dir.path().join("flaky.log");
    std::fs::write(&path, numbered(1..=10)).unwrap();
    let (h, _) = env.source("flaky.log", 100_000);
    next(&h, |e| matches!(e, SourceEvent::Tail { .. }));
    next(&h, |e| matches!(e, SourceEvent::Status(s) if s.label() == "Live"));
    // Drop every session of the test sshd (its listener stays up).
    let pid = std::fs::read_to_string(format!("{key_dir}/sshd.pid")).unwrap();
    let status = std::process::Command::new("pkill").args(["-P", pid.trim()]).status().unwrap();
    assert!(status.success() || status.code() == Some(1));
    // While it is down, the server writes 300 lines.
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(numbered(11..=310).as_bytes()).unwrap();
    next(&h, |e| *e == SourceEvent::Break { kind: BreakKind::Disconnected });
    next(&h, |e| matches!(e, SourceEvent::Status(s) if s.label() == "Reconnecting"));
    next(&h, |e| *e == SourceEvent::Break { kind: BreakKind::Reconnected });
    let mut got = 0;
    let mut first = None;
    while got < 300 {
        let SourceEvent::Append { lines, .. } = next(&h, |e| matches!(e, SourceEvent::Append { .. })) else { unreachable!() };
        first.get_or_insert(lines[0].clone());
        got += lines.len();
    }
    assert_eq!(first.as_deref(), Some("line 11"), "no line lost, none repeated");
    assert_eq!(got, 300);
    h.close();
}

#[test]
fn clear_stops_reading_older_lines_on_its_own() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("clear.log");
    // Long enough that reading it all back takes many batches.
    std::fs::write(&path, numbered(1..=400_000)).unwrap();
    let (h, _) = env.source("clear.log", 400_000);
    next(&h, |e| matches!(e, SourceEvent::Tail { .. }));
    h.send(SourceCommand::Clear);
    std::thread::sleep(Duration::from_millis(2_500));
    let mut loaded = 1_000;
    while let Ok(e) = h.events.try_recv() {
        if let SourceEvent::Backfill { lines, .. } = e {
            loaded += lines.len();
        }
    }
    assert!(loaded < 200_000, "the window was filled after Clear: {loaded}");
    // New lines still come.
    let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(b"after clear\n").unwrap();
    let SourceEvent::Append { lines, .. } = next(&h, |e| matches!(e, SourceEvent::Append { .. })) else { unreachable!() };
    assert_eq!(lines, vec!["after clear".to_string()]);
    h.close();
}

#[test]
fn clear_while_paused_shows_lines_written_meanwhile_under_the_clear_point() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("paused.log");
    std::fs::write(&path, numbered(1..=10)).unwrap();
    let (h, _) = env.source("paused.log", 100_000);
    next(&h, |e| matches!(e, SourceEvent::Tail { .. }));
    next(&h, |e| matches!(e, SourceEvent::Status(s) if s.label() == "Live"));
    h.send(SourceCommand::Pause);
    h.send(SourceCommand::Clear);
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(numbered(11..=13).as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(1_200));
    while let Ok(e) = h.events.try_recv() {
        assert!(!matches!(e, SourceEvent::Append { .. } | SourceEvent::Backfill { .. }), "read while paused: {e:?}");
    }
    h.send(SourceCommand::Resume);
    let SourceEvent::Append { lines, bytes, .. } = next(&h, |e| matches!(e, SourceEvent::Append { .. })) else {
        unreachable!()
    };
    assert_eq!(lines, vec!["line 11", "line 12", "line 13"]);
    assert_eq!(bytes, vec![8, 8, 8]);
    h.close();
}

#[test]
fn following_a_quiet_file_starts_no_process_on_the_server_for_each_poll() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("quiet.log");
    std::fs::write(&path, numbered(1..=3)).unwrap();
    let (h, _) = env.source("quiet.log", 100_000);
    next(&h, |e| matches!(e, SourceEvent::Tail { .. }));
    std::thread::sleep(Duration::from_millis(500));
    let before = execs(&env);
    // Six seconds is twelve polls; the inode is looked at only now and then.
    std::thread::sleep(Duration::from_secs(6));
    let during = execs(&env) - before;
    assert!(during <= 3, "{during} processes started in 6 seconds");
    // Growth is still followed.
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(b"more\n").unwrap();
    assert_eq!(
        next(&h, |e| matches!(e, SourceEvent::Append { .. })),
        SourceEvent::Append { lines: vec!["more".into()], bytes: vec![5], streams: None }
    );
    h.close();
}

#[test]
fn a_rotation_to_a_bigger_file_is_found_by_the_slower_inode_check() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("grow.log");
    std::fs::write(&path, numbered(1..=3)).unwrap();
    let (h, _) = env.source("grow.log", 100_000);
    next(&h, |e| matches!(e, SourceEvent::Tail { .. }));
    // The new file is already bigger than the old one: the size alone shows nothing.
    std::fs::rename(&path, env.dir.path().join("grow.log.1")).unwrap();
    std::fs::write(&path, numbered(100..=140)).unwrap();
    next(&h, |e| *e == SourceEvent::Break { kind: BreakKind::Rotated });
    let SourceEvent::Append { lines, .. } = next(&h, |e| matches!(e, SourceEvent::Append { .. })) else { unreachable!() };
    assert_eq!(lines.first().map(String::as_str), Some("line 100"));
    h.close();
}

/// Where the file stands, with the hash of its own bytes (openspec change
/// `remote-local-cache`, task 3.1): what the cache keeps to tell next time
/// whether it is the same file.
#[test]
fn a_checkpoint_hashes_the_first_and_last_bytes_read() {
    use log_gpui::remote::cache::fnv64;
    let Some(env) = env() else { return };
    let path = env.dir.path().join("cp.log");
    let text = numbered(1..=2_000);
    std::fs::write(&path, &text).unwrap();
    let (h, _) = env.source("cp.log", 100_000);
    let SourceEvent::Checkpoint { server_end, head_hash, tail_hash, generation, .. } =
        next(&h, |e| matches!(e, SourceEvent::Checkpoint { .. }))
    else {
        unreachable!()
    };
    let bytes = text.as_bytes();
    assert_eq!((generation, server_end), (0, bytes.len() as u64));
    assert_eq!(head_hash, fnv64(&bytes[..4096]));
    assert_eq!(tail_hash, fnv64(&bytes[bytes.len() - 4096..]));

    // A half-written line is not part of it.
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(b"line 2001\nline 20").unwrap();
    let SourceEvent::Checkpoint { server_end, tail_hash, .. } = next(&h, |e| matches!(e, SourceEvent::Checkpoint { .. })) else {
        unreachable!()
    };
    let all = std::fs::read(&path).unwrap();
    let end = all.len() - "line 20".len();
    assert_eq!(server_end, end as u64);
    assert_eq!(tail_hash, fnv64(&all[end - 4096..end]));
    h.close();
}

/// The Checkpoint of `cp.log` once it holds `text`, as the cache keeps it.
fn checkpoint_of(env: &Env, name: &str) -> ResumeFrom {
    let (h, _) = env.source(name, 100_000);
    let SourceEvent::Checkpoint { generation, inode, server_end, head_hash, tail_hash } =
        next(&h, |e| matches!(e, SourceEvent::Checkpoint { .. }))
    else {
        unreachable!()
    };
    h.close();
    ResumeFrom { generation, inode, server_end, head_hash, tail_hash }
}

/// Read on from a Checkpoint (task 3.2): only what was written since comes.
#[test]
fn resuming_the_same_file_reads_only_what_was_written_since() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("re.log");
    std::fs::write(&path, numbered(1..=3_000)).unwrap();
    let from = checkpoint_of(&env, "re.log");
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(numbered(3_001..=3_005).as_bytes()).unwrap();
    let (h, _) = env.source_from("re.log", 100_000, Some(from));
    let first = next(&h, |e| matches!(e, SourceEvent::Append { .. } | SourceEvent::Tail { .. } | SourceEvent::Break { .. }));
    let SourceEvent::Append { lines, .. } = first else { panic!("no Tail, no Break: {first:?}") };
    assert_eq!(lines, (3_001..=3_005).map(|i| format!("line {i}")).collect::<Vec<_>>());
    std::thread::sleep(Duration::from_millis(800));
    while let Ok(e) = h.events.try_recv() {
        assert!(!matches!(e, SourceEvent::Backfill { .. } | SourceEvent::Tail { .. }), "nothing read again: {e:?}");
    }
    h.close();
}

/// Another file at the path, or the same one written over: the kept lines
/// stay above a Break and the file is read as a new one.
#[test]
fn resuming_another_file_is_a_break_then_its_tail() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("rot.log");
    std::fs::write(&path, numbered(1..=3_000)).unwrap();
    let from = checkpoint_of(&env, "rot.log");

    // Rotated: a new file (new inode) at the path.
    std::fs::rename(&path, env.dir.path().join("rot.log.1")).unwrap();
    std::fs::write(&path, numbered(1..=10)).unwrap();
    let (h, _) = env.source_from("rot.log", 100_000, Some(from.clone()));
    let e = next(&h, |e| matches!(e, SourceEvent::Break { .. } | SourceEvent::Tail { .. } | SourceEvent::Append { .. }));
    assert!(matches!(e, SourceEvent::Break { kind: BreakKind::Rotated }), "{e:?}");
    let SourceEvent::Tail { lines, .. } = next(&h, |e| matches!(e, SourceEvent::Tail { .. })) else { unreachable!() };
    assert_eq!(lines.len(), 10);
    h.close();

    // Written over in place, as long: the same inode, other bytes.
    let path = env.dir.path().join("over.log");
    std::fs::write(&path, numbered(1..=3_000)).unwrap();
    let from = checkpoint_of(&env, "over.log");
    let text = numbered(1..=3_000).replace("line 1\n", "LINE 1\n");
    let mut f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    f.write_all(text.as_bytes()).unwrap();
    drop(f);
    let (h, _) = env.source_from("over.log", 100_000, Some(from));
    let e = next(&h, |e| matches!(e, SourceEvent::Break { .. } | SourceEvent::Tail { .. } | SourceEvent::Append { .. }));
    assert!(matches!(e, SourceEvent::Break { kind: BreakKind::Truncated }), "{e:?}");
    next(&h, |e| matches!(e, SourceEvent::Tail { .. }));
    h.close();
}

/// Processes started on the Server through the shared session so far.
fn execs(env: &Env) -> usize {
    let (hub, store, id) = (env.hub.clone(), env.store.clone(), env.server_id.clone());
    hub.clone().handle().block_on(async move { hub.session(&store, &id).await.unwrap().exec_count() })
}
