//! Rotated Files read as earlier Generations. Needs a local sshd (see
//! `remote_hub_test.rs`): `SSH_TEST_DIR`, `SSH_TEST_PORT`.

use log_gpui::live::{LineSource, RemoteFileSource, SourceCommand, SourceEvent, SourceHandle};
use log_gpui::remote::*;
use std::io::Write;
use std::time::SystemTime;
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


fn set_age(path: &std::path::Path, secs_ago: u64) {
    let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(secs_ago)).unwrap();
}

fn gzip(text: &str) -> Vec<u8> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(text.as_bytes()).unwrap();
    enc.finish().unwrap()
}

/// Every event until `done` says so, in order.
fn collect_until(h: &SourceHandle, mut done: impl FnMut(&SourceEvent) -> bool) -> Vec<SourceEvent> {
    let mut out = Vec::new();
    next(h, |e| {
        out.push(e.clone());
        done(e)
    });
    out
}

#[test]
fn right_after_logrotate_the_newest_plain_rotated_file_fills_the_window() {
    let Some(env) = env() else { return };
    let d = env.dir.path();
    std::fs::write(d.join("app.log"), numbered(10_001..=10_300)).unwrap();
    std::fs::write(d.join("app.log.1"), numbered(1..=10_000)).unwrap();
    std::fs::write(d.join("app.log.2.gz"), gzip(&numbered(1..=50))).unwrap();
    set_age(&d.join("app.log.1"), 60);
    set_age(&d.join("app.log.2.gz"), 3_600);
    let (h, _) = env.source("app.log", 1_000);
    let events = collect_until(&h, |e| matches!(e, SourceEvent::EarlierAvailable(_)));
    let tail = events.iter().find_map(|e| match e {
        SourceEvent::Tail { lines, .. } => Some(lines.len()),
        _ => None,
    });
    assert_eq!(tail, Some(300));
    let earlier: Vec<&SourceEvent> = events.iter().filter(|e| matches!(e, SourceEvent::Earlier { .. })).collect();
    assert!(!earlier.is_empty());
    let SourceEvent::Earlier { start, last_line_no, .. } = earlier[0] else { unreachable!() };
    assert_eq!(start.as_ref().unwrap().name, "app.log.1");
    assert_eq!(*last_line_no, Some(10_000));
    let total: usize = earlier
        .iter()
        .map(|e| match e {
            SourceEvent::Earlier { lines, .. } => lines.len(),
            _ => 0,
        })
        .sum();
    // 300 lines of the file and 700 of `app.log.1` fill the window of 1,000.
    assert_eq!(total, 700);
    let SourceEvent::Earlier { lines, .. } = earlier.last().unwrap() else { unreachable!() };
    assert_eq!(lines.first().unwrap(), "line 9301");
    assert_eq!(lines.last().unwrap(), "line 10000");
    // The window is full inside `app.log.1`: no button.
    assert!(matches!(events.last().unwrap(), SourceEvent::EarlierAvailable(None)));
    h.close();
}

#[test]
fn a_small_plain_rotated_file_is_read_whole_and_the_next_one_is_offered() {
    let Some(env) = env() else { return };
    let d = env.dir.path();
    std::fs::write(d.join("app.log"), numbered(1..=10)).unwrap();
    std::fs::write(d.join("app.log.1"), numbered(1..=20)).unwrap();
    std::fs::write(d.join("app.log.2.gz"), gzip(&numbered(1..=30))).unwrap();
    set_age(&d.join("app.log.1"), 60);
    set_age(&d.join("app.log.2.gz"), 3_600);
    let (h, _) = env.source("app.log", 100_000);
    let events = collect_until(&h, |e| matches!(e, SourceEvent::EarlierAvailable(_)));
    let SourceEvent::EarlierAvailable(Some(next_file)) = events.last().unwrap() else { panic!("{events:?}") };
    assert_eq!(next_file.name, "app.log.2.gz");
    assert!(next_file.compressed);
    h.close();
}

#[test]
fn a_compressed_newest_rotated_file_is_not_read_until_asked() {
    let Some(env) = env() else { return };
    let d = env.dir.path();
    std::fs::write(d.join("app.log"), numbered(1..=300)).unwrap();
    std::fs::write(d.join("app.log-20261018.gz"), gzip(&numbered(1..=5_000))).unwrap();
    let (h, _) = env.source("app.log", 1_000);
    let events = collect_until(&h, |e| matches!(e, SourceEvent::EarlierAvailable(_)));
    assert!(!events.iter().any(|e| matches!(e, SourceEvent::Earlier { .. })));
    let SourceEvent::EarlierAvailable(Some(file)) = events.last().unwrap() else { panic!("{events:?}") };
    assert_eq!(file.name, "app.log-20261018.gz");
    assert!(file.size > 0);
    h.send(SourceCommand::LoadEarlier { room: 700 });
    let events = collect_until(&h, |e| matches!(e, SourceEvent::EarlierAvailable(_)));
    let SourceEvent::Earlier { start, lines, last_line_no, reached_start, .. } =
        events.iter().find(|e| matches!(e, SourceEvent::Earlier { .. })).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(start.as_ref().unwrap().name, "app.log-20261018.gz");
    assert_eq!(lines.len(), 700);
    assert_eq!(lines.first().unwrap(), "line 4301");
    assert_eq!(lines.last().unwrap(), "line 5000");
    assert_eq!(*last_line_no, Some(5_000));
    assert!(!reached_start);
    h.close();
}

#[test]
fn a_file_that_already_fills_the_window_reads_nothing_of_the_rotated_one() {
    let Some(env) = env() else { return };
    let d = env.dir.path();
    std::fs::write(d.join("app.log"), numbered(1..=3_000)).unwrap();
    std::fs::write(d.join("app.log.1"), numbered(1..=100)).unwrap();
    let (h, _) = env.source("app.log", 2_000);
    // The window fills inside the file: nothing is read of `app.log.1` and
    // there is no button (the top line is not the first of its Generation).
    std::thread::sleep(Duration::from_millis(2_500));
    while let Ok(e) = h.events.try_recv() {
        assert!(!matches!(e, SourceEvent::Earlier { .. } | SourceEvent::EarlierAvailable(_)), "{e:?}");
    }
    h.close();
}

#[test]
fn every_compression_kind_is_read_by_its_first_bytes_not_its_name() {
    let Some(env) = env() else { return };
    let kinds: [(&str, &[u8]); 4] = [
        ("zst", include_bytes!("../src/remote/testdata/t.zst")),
        ("xz", include_bytes!("../src/remote/testdata/t.xz")),
        ("bz2", include_bytes!("../src/remote/testdata/t.bz2")),
        ("1", include_bytes!("../src/remote/testdata/t.gz")),
    ];
    for (ext, bytes) in kinds {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("app.log"), "2026-10-19 10:00:00 INFO now\n").unwrap();
        // `.1` is really gzip: compressed, so it is only read when asked.
        let rotated = if ext == "1" { "app.log.1".to_string() } else { format!("app.log.1.{ext}") };
        std::fs::write(d.path().join(&rotated), bytes).unwrap();
        let env2 = Env { dir: d, hub: env.hub.clone(), store: env.store.clone(), server_id: env.server_id.clone() };
        let (h, _) = env2.source("app.log", 1_000);
        collect_until(&h, |e| matches!(e, SourceEvent::EarlierAvailable(Some(_))));
        h.send(SourceCommand::LoadEarlier { room: 100 });
        let events = collect_until(&h, |e| matches!(e, SourceEvent::Earlier { .. }));
        let SourceEvent::Earlier { lines, last_line_no, reached_start, .. } = events.last().unwrap() else {
            unreachable!()
        };
        assert_eq!(lines, &["first line", "second line", "third line"], "{rotated}");
        assert_eq!(*last_line_no, Some(3));
        assert!(*reached_start);
        h.close();
    }
}

#[test]
fn an_unsupported_compression_is_reported_and_not_read() {
    let Some(env) = env() else { return };
    let d = env.dir.path();
    std::fs::write(d.join("app.log"), numbered(1..=5)).unwrap();
    let mut lz4 = vec![0x04, 0x22, 0x4d, 0x18, 0x64, 0x40];
    lz4.extend_from_slice(&[0; 40]);
    std::fs::write(d.join("app.log.1.lz4"), &lz4).unwrap();
    // `.lz4` is not a rotation suffix the app knows; `app.log.1` with lz4 bytes is.
    std::fs::write(d.join("app.log.1"), &lz4).unwrap();
    let (h, _) = env.source("app.log", 1_000);
    let events = collect_until(&h, |e| matches!(e, SourceEvent::EarlierAvailable(_)));
    assert!(!events.iter().any(|e| matches!(e, SourceEvent::Earlier { .. })));
    h.send(SourceCommand::LoadEarlier { room: 100 });
    let events = collect_until(&h, |e| matches!(e, SourceEvent::EarlierAvailable(_)));
    assert!(events.iter().any(|e| matches!(e, SourceEvent::Problem(m) if m.contains("not supported"))));
    h.close();
}

#[test]
fn a_starting_time_stops_the_backward_read_and_the_rotated_file() {
    let Some(env) = env() else { return };
    let d = env.dir.path();
    let stamp = |i: u32| format!("2026-10-19 {:02}:{:02}:00 INFO line {i}\n", i / 60, i % 60);
    let text = |r: std::ops::Range<u32>| r.map(stamp).collect::<String>();
    // 10:00 is line 600. The file holds 9:50–10:09, the rotated one 9:00–9:49.
    std::fs::write(d.join("app.log"), text(590..610)).unwrap();
    std::fs::write(d.join("app.log.1"), text(540..590)).unwrap();
    set_age(&d.join("app.log.1"), 60);
    let path = d.join("app.log").to_string_lossy().to_string();
    let handle = RemoteFileSource {
        glob_inodes: None,
        hub: env.hub.clone(),
        servers: Arc::new(Mutex::new(env.store.clone())),
        server_id: env.server_id.clone(),
        path: path.clone(),
        name: format!("web1:{path}"),
        window: 100_000,
        start_time: Some("2026-10-19 09:45".into()),
    }
    .spawn();
    let events = collect_until(&handle, |e| matches!(e, SourceEvent::EarlierAvailable(_)));
    let SourceEvent::Tail { lines, .. } = &events[events.iter().position(|e| matches!(e, SourceEvent::Tail { .. })).unwrap()]
    else {
        unreachable!()
    };
    assert_eq!(lines.len(), 20);
    let earlier: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            SourceEvent::Earlier { lines, .. } => Some(lines.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    // 09:44 and 09:45 are within a second of the slack of 09:45:00? No: only
    // lines from 09:45 on are read.
    assert_eq!(earlier.first().unwrap(), &stamp(585).trim_end().to_string());
    assert_eq!(earlier.last().unwrap(), &stamp(589).trim_end().to_string());
    assert!(matches!(events.last().unwrap(), SourceEvent::EarlierAvailable(None)));
    handle.close();
}

#[test]
fn lines_since_the_starting_time_that_do_not_fit_the_window_are_counted() {
    let Some(env) = env() else { return };
    let d = env.dir.path();
    let stamp = |i: u32| format!("2026-10-19 {:02}:{:02}:{:02} INFO line {i}\n", i / 3600, i / 60 % 60, i % 60);
    std::fs::write(d.join("app.log"), (0..5_000).map(stamp).collect::<String>()).unwrap();
    let path = d.join("app.log").to_string_lossy().to_string();
    let spawn = |start: &str| {
        RemoteFileSource {
            glob_inodes: None,
            hub: env.hub.clone(),
            servers: Arc::new(Mutex::new(env.store.clone())),
            server_id: env.server_id.clone(),
            path: path.clone(),
            name: format!("web1:{path}"),
            window: 1_200,
            start_time: Some(start.into()),
        }
        .spawn()
    };
    // From 00:10:00 (line 600) on there are 4,400 lines; the window holds 1,200.
    let h = spawn("2026-10-19 00:10:00");
    let SourceEvent::StartTimeOverflow { lines, since } = next(&h, |e| matches!(e, SourceEvent::StartTimeOverflow { .. }))
    else {
        unreachable!()
    };
    // One second of slack lets the line at 00:09:59 in too.
    assert_eq!(lines, Some(4_401));
    assert_eq!(since, "2026-10-19 00:10:00");
    h.close();
    // A Starting Time that fits says nothing.
    let h = spawn("2026-10-19 01:10:00");
    std::thread::sleep(Duration::from_millis(2_500));
    while let Ok(e) = h.events.try_recv() {
        assert!(!matches!(e, SourceEvent::StartTimeOverflow { .. }), "{e:?}");
    }
    h.close();
}
