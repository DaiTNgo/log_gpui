//! Needs a local sshd (see `remote_hub_test.rs`): `SSH_TEST_DIR`, `SSH_TEST_PORT`.

use log_gpui::live::{BreakKind, LineSource, RemoteFileSource, SourceCommand, SourceEvent, SourceHandle};
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
        let path = self.dir.path().join(name).to_string_lossy().to_string();
        let handle = RemoteFileSource {
            glob_inodes: None,
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
fn load_more_reads_older_lines_above_the_oldest_held_one() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("more.log");
    std::fs::write(&path, numbered(1..=12_000)).unwrap();
    let (h, _) = env.source("more.log", 1_000);
    let SourceEvent::Tail { lines, bytes, end_offset, .. } =
        next(&h, |e| matches!(e, SourceEvent::Tail { .. })) else { unreachable!() };
    assert_eq!(lines.first().unwrap(), "line 11001");
    let oldest = end_offset - bytes.iter().map(|b| u64::from(*b)).sum::<u64>();
    let hash = log_gpui::workspace::log_space::line_hash("line 11001");
    h.send(SourceCommand::LoadMore { generation: 0, end_offset: oldest, lines: 500, first_hash: Some(hash), held_bytes: 0, first_line_no: None, inode: None });
    let SourceEvent::Older { lines, bytes, reached_start, .. } =
        next(&h, |e| matches!(e, SourceEvent::Older { .. } | SourceEvent::OlderUnavailable)) else {
            panic!("Load More was refused")
        };
    assert_eq!((lines.len(), reached_start), (500, false));
    assert_eq!(lines.first().unwrap(), "line 10501");
    assert_eq!(lines.last().unwrap(), "line 11000");
    assert_eq!(bytes.iter().map(|b| u64::from(*b)).sum::<u64>(), numbered(10_501..=11_000).len() as u64);

    // A line other than the one held at that offset: the file changed.
    h.send(SourceCommand::LoadMore {
        generation: 0,
        end_offset: oldest,
        lines: 10,
        first_hash: Some(log_gpui::workspace::log_space::line_hash("line 1")), held_bytes: 0, first_line_no: None, inode: None
    });
    assert_eq!(
        next(&h, |e| matches!(e, SourceEvent::Older { .. } | SourceEvent::OlderUnavailable)),
        SourceEvent::OlderUnavailable
    );

    // Up to the start of the file.
    let from = numbered(1..=300).len() as u64;
    h.send(SourceCommand::LoadMore { generation: 0, end_offset: from, lines: 5_000, first_hash: None, held_bytes: 0, first_line_no: None, inode: None });
    let SourceEvent::Older { lines, reached_start, .. } =
        next(&h, |e| matches!(e, SourceEvent::Older { .. } | SourceEvent::OlderUnavailable)) else {
            panic!("Load More was refused")
        };
    assert_eq!((lines.len(), reached_start), (300, true));
    assert_eq!(lines.first().unwrap(), "line 1");
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
fn cancel_drops_a_load_more_that_has_not_started() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("cancel.log");
    std::fs::write(&path, numbered(1..=3_000)).unwrap();
    let (h, _) = env.source("cancel.log", 1_000);
    let SourceEvent::Tail { bytes, end_offset, .. } = next(&h, |e| matches!(e, SourceEvent::Tail { .. })) else {
        unreachable!()
    };
    let oldest = end_offset - bytes.iter().map(|b| u64::from(*b)).sum::<u64>();
    h.send(SourceCommand::LoadMore { generation: 0, end_offset: oldest, lines: 100, first_hash: None, held_bytes: 0, first_line_no: None, inode: None });
    h.send(SourceCommand::CancelLoadMore);
    // Either the batch was read before the cancel arrived (kept), or an empty
    // answer says it was dropped: the app stops waiting in both cases.
    let SourceEvent::Older { lines, .. } = next(&h, |e| matches!(e, SourceEvent::Older { .. })) else { unreachable!() };
    assert!(lines.is_empty() || lines.len() == 100);
    h.close();
}

#[test]
fn load_more_after_clear_reads_the_cleared_lines_again() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("after_clear.log");
    std::fs::write(&path, numbered(1..=5_000)).unwrap();
    let (h, _) = env.source("after_clear.log", 1_000);
    let SourceEvent::Tail { end_offset, .. } = next(&h, |e| matches!(e, SourceEvent::Tail { .. })) else {
        unreachable!()
    };
    // The Clear Point is where the file ended; new lines come after it.
    h.send(SourceCommand::Clear);
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(b"new 1\nnew 2\n").unwrap();
    let SourceEvent::Append { lines, .. } = next(&h, |e| matches!(e, SourceEvent::Append { .. })) else {
        unreachable!()
    };
    assert_eq!(lines, vec!["new 1", "new 2"]);
    let hash = log_gpui::workspace::log_space::line_hash("new 1");
    h.send(SourceCommand::LoadMore { generation: 0, end_offset, lines: 200, first_hash: Some(hash), held_bytes: 0, first_line_no: None, inode: None });
    let SourceEvent::Older { lines, reached_start, .. } =
        next(&h, |e| matches!(e, SourceEvent::Older { .. } | SourceEvent::OlderUnavailable)) else {
            panic!("Load More was refused")
        };
    assert_eq!(lines.len(), 200);
    assert_eq!(lines.first().unwrap(), "line 4801");
    assert_eq!(lines.last().unwrap(), "line 5000");
    assert!(!reached_start);
    h.close();
}

#[test]
fn load_more_after_a_rotation_reads_the_old_generation_from_the_rotated_file_and_the_new_one_from_the_file() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("rot.log");
    std::fs::write(&path, numbered(1..=3_000)).unwrap();
    let (h, _) = env.source("rot.log", 1_000);
    let SourceEvent::Tail { bytes, end_offset, .. } = next(&h, |e| matches!(e, SourceEvent::Tail { .. })) else {
        unreachable!()
    };
    let old_start = end_offset - bytes.iter().map(|b| u64::from(*b)).sum::<u64>();
    std::fs::rename(&path, env.dir.path().join("rot.log.1")).unwrap();
    std::fs::write(&path, numbered(1..=50)).unwrap();
    next(&h, |e| *e == SourceEvent::Break { kind: BreakKind::Rotated });
    next(&h, |e| matches!(e, SourceEvent::Append { .. }));
    // The held lines of the old Generation are in a Rotated File now: they are
    // read from there (`old_start` means nothing for it; the bytes held do).
    h.send(SourceCommand::LoadMore {
        generation: 0,
        end_offset: old_start,
        lines: 100,
        first_hash: None,
        held_bytes: 0,
        first_line_no: None,
        inode: None,
    });
    let SourceEvent::Older { lines, .. } =
        next(&h, |e| matches!(e, SourceEvent::Older { .. } | SourceEvent::OlderUnavailable)) else {
            panic!("the Rotated File was not read")
        };
    assert_eq!(lines.last().unwrap(), "line 3000");
    // The new Generation reads up to its first line and says so.
    h.send(SourceCommand::LoadMore {
        generation: 1,
        end_offset: numbered(1..=10).len() as u64,
        lines: 100,
        first_hash: None, held_bytes: 0, first_line_no: None, inode: None
    });
    let SourceEvent::Older { lines, reached_start, .. } =
        next(&h, |e| matches!(e, SourceEvent::Older { .. } | SourceEvent::OlderUnavailable)) else {
            panic!("Load More was refused")
        };
    assert_eq!((lines.len(), reached_start), (10, true));
    h.close();
}

/// A file whose first 3,000 lines were rotated away: the app holds the last
/// 1,000 of them (Generation 0), the file now has fresh lines (Generation 1).
fn rotated_env(env: &Env, name: &str, gzip: bool) -> (SourceHandle, u64, usize) {
    let path = env.dir.path().join(name);
    std::fs::write(&path, numbered(1..=3_000)).unwrap();
    let (h, _) = env.source(name, 1_000);
    let SourceEvent::Tail { bytes, .. } = next(&h, |e| matches!(e, SourceEvent::Tail { .. })) else {
        unreachable!()
    };
    let held: u64 = bytes.iter().map(|b| u64::from(*b)).sum();
    let rotated = env.dir.path().join(format!("{name}.1"));
    std::fs::rename(&path, &rotated).unwrap();
    std::fs::write(&path, numbered(1..=5)).unwrap();
    if gzip {
        assert!(std::process::Command::new("gzip").arg(&rotated).status().unwrap().success());
    }
    next(&h, |e| *e == SourceEvent::Break { kind: BreakKind::Rotated });
    (h, held, 2_001)
}

#[test]
fn load_more_reads_a_plain_rotated_file_and_offers_the_next_one_at_its_start() {
    let Some(env) = env() else { return };
    std::fs::write(env.dir.path().join("rot_plain.log.2"), numbered(1..=10)).unwrap();
    let (h, held, first_no) = rotated_env(&env, "rot_plain.log", false);
    let hash = log_gpui::workspace::log_space::line_hash("line 2001");
    let ask = |lines: usize| SourceCommand::LoadMore {
        generation: 0,
        end_offset: 0,
        lines,
        first_hash: Some(hash),
        held_bytes: held,
        first_line_no: Some(first_no),
        inode: None,
    };
    h.send(ask(500));
    let SourceEvent::Older { lines, reached_start, generation, .. } =
        next(&h, |e| matches!(e, SourceEvent::Older { .. } | SourceEvent::OlderUnavailable)) else {
            panic!("Load More of the Rotated File was refused")
        };
    assert_eq!((generation, lines.len(), reached_start), (0, 500, false));
    assert_eq!(lines.first().unwrap(), "line 1501");
    assert_eq!(lines.last().unwrap(), "line 2000");

    // A line other than the held one: not the file the lines came from.
    h.send(SourceCommand::LoadMore {
        generation: 0,
        end_offset: 0,
        lines: 10,
        first_hash: Some(log_gpui::workspace::log_space::line_hash("line 1")),
        held_bytes: held,
        first_line_no: Some(first_no),
        inode: None,
    });
    assert_eq!(
        next(&h, |e| matches!(e, SourceEvent::Older { .. } | SourceEvent::OlderUnavailable)),
        SourceEvent::OlderUnavailable
    );

    // Up to its first line: the next Rotated File is offered.
    h.send(ask(5_000));
    let SourceEvent::Older { lines, reached_start, .. } =
        next(&h, |e| matches!(e, SourceEvent::Older { .. } | SourceEvent::OlderUnavailable)) else {
            panic!("Load More of the Rotated File was refused")
        };
    assert_eq!((lines.len(), reached_start), (2_000, true));
    let SourceEvent::EarlierAvailable(next_file) = next(&h, |e| matches!(e, SourceEvent::EarlierAvailable(_))) else {
        unreachable!()
    };
    assert_eq!(next_file.map(|f| f.name), Some("rot_plain.log.2".to_string()));
    h.close();
}

#[test]
fn load_more_decodes_a_compressed_rotated_file_and_keeps_the_lines_before_the_held_ones() {
    let Some(env) = env() else { return };
    let (h, held, first_no) = rotated_env(&env, "rot_gz.log", true);
    let hash = log_gpui::workspace::log_space::line_hash("line 2001");
    h.send(SourceCommand::LoadMore {
        generation: 0,
        end_offset: 0,
        lines: 300,
        first_hash: Some(hash),
        held_bytes: held,
        first_line_no: Some(first_no),
        inode: None,
    });
    let SourceEvent::Older { lines, reached_start, .. } =
        next(&h, |e| matches!(e, SourceEvent::Older { .. } | SourceEvent::OlderUnavailable)) else {
            panic!("Load More of the compressed file was refused")
        };
    assert_eq!((lines.len(), reached_start), (300, false));
    assert_eq!(lines.first().unwrap(), "line 1701");
    assert_eq!(lines.last().unwrap(), "line 2000");
    h.close();
}

/// Processes started on the Server through the shared session so far.
fn execs(env: &Env) -> usize {
    let (hub, store, id) = (env.hub.clone(), env.store.clone(), env.server_id.clone());
    hub.clone().handle().block_on(async move { hub.session(&store, &id).await.unwrap().exec_count() })
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
