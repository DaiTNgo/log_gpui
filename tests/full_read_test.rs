//! Open in full against a local sshd (`SSH_TEST_DIR`, `SSH_TEST_PORT`).

use async_channel::Receiver;
use log_gpui::remote::compress::Compression;
use log_gpui::remote::copy::{self, StopReason};
use log_gpui::remote::full_read::{GenerationPlan, ReadControl, ReadEvent, ReadPlan, run};
use log_gpui::remote::*;
use std::io::Write;
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
    format!("2026-10-19 {:02}:{:02}:{:02} INFO line {i}\n", i / 3600, i / 60 % 60, i % 60)
}

fn gzip(text: &str) -> Vec<u8> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(text.as_bytes()).unwrap();
    enc.finish().unwrap()
}

impl Env {
    fn path(&self, name: &str) -> String {
        self.dir.path().join(name).to_string_lossy().to_string()
    }

    fn generation(&self, distance: u32, name: &str) -> GenerationPlan {
        let path = self.path(name);
        let bytes = std::fs::read(&path).unwrap();
        let compression = Compression::detect(&bytes[..bytes.len().min(6)]);
        GenerationPlan {
            distance,
            name: name.into(),
            path,
            size: bytes.len() as u64,
            modified: None,
            compression,
            start_byte: None,
            skip_lines: 0,
            older_dropped: false,
        }
    }

    fn plan(&self, generations: Vec<GenerationPlan>, start: Option<f64>, to: Option<f64>, limit: usize) -> ReadPlan {
        ReadPlan {
            server_id: self.server_id.clone(),
            server_name: "web1".into(),
            path: self.path("app.log"),
            generations,
            start,
            to,
            start_text: None,
            to_text: None,
            limit_lines: limit,
            dir: copy::create_dir(self.copies.path()).unwrap(),
        }
    }

    fn start(&self, plan: ReadPlan) -> (Receiver<ReadEvent>, Arc<ReadControl>) {
        let (tx, rx) = async_channel::unbounded();
        let ctl = Arc::new(ReadControl::default());
        self.hub.spawn(run(self.hub.clone(), Arc::new(Mutex::new(self.store.clone())), plan, tx, ctl.clone()));
        (rx, ctl)
    }
}

fn all_events(rx: &Receiver<ReadEvent>) -> Vec<ReadEvent> {
    let mut out = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match rx.try_recv() {
            Ok(e) => {
                let end = matches!(e, ReadEvent::Done(_) | ReadEvent::Failed { .. });
                out.push(e);
                if end {
                    return out;
                }
            }
            Err(_) => {
                assert!(std::time::Instant::now() < deadline, "timed out: {out:?}");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn lines_of(events: &[ReadEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            ReadEvent::Lines { lines, .. } => Some(lines.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn time(i: u32) -> f64 {
    log_gpui::live::line_time::line_timestamp(&stamp(i)).unwrap()
}

#[test]
fn reads_a_whole_file_with_a_plain_and_a_compressed_rotated_file_oldest_first() {
    let Some(env) = env() else { return };
    std::fs::write(env.path("app.log"), (200..300).map(stamp).collect::<String>()).unwrap();
    std::fs::write(env.path("app.log.1"), (100..200).map(stamp).collect::<String>()).unwrap();
    std::fs::write(env.path("app.log.2.gz"), gzip(&(0..100).map(stamp).collect::<String>())).unwrap();
    let gens = vec![env.generation(2, "app.log.2.gz"), env.generation(1, "app.log.1"), env.generation(0, "app.log")];
    let plan = env.plan(gens, None, None, 2_000_000);
    let dir = plan.dir.clone();
    let (rx, _) = env.start(plan);
    let events = all_events(&rx);
    let got = lines_of(&events);
    assert_eq!(got.len(), 300);
    assert_eq!(got[0], stamp(0).trim_end());
    assert_eq!(got[299], stamp(299).trim_end());
    let starts: Vec<(usize, u32, String)> = events
        .iter()
        .filter_map(|e| match e {
            ReadEvent::GenerationStart { index, distance, name, .. } => Some((*index, *distance, name.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        starts,
        [(0, 2, "app.log.2.gz".to_string()), (1, 1, "app.log.1".to_string()), (2, 0, "app.log".to_string())]
    );
    let ReadEvent::Done(meta) = events.last().unwrap() else { panic!("{:?}", events.last()) };
    assert!(meta.is_complete());
    assert_eq!(meta.total_lines(), 300);
    assert_eq!(std::fs::read_to_string(dir.join("gen-1.log")).unwrap().lines().count(), 100);
    assert_eq!(copy::read_meta(&dir).unwrap(), *meta);
}

#[test]
fn a_time_range_in_the_middle_of_a_big_file_reads_only_that_range_with_real_line_numbers() {
    let Some(env) = env() else { return };
    std::fs::write(env.path("app.log"), (0..20_000).map(stamp).collect::<String>()).unwrap();
    let plan = env.plan(vec![env.generation(0, "app.log")], Some(time(7_000)), Some(time(7_999)), 2_000_000);
    let (rx, _) = env.start(plan);
    let events = all_events(&rx);
    let got = lines_of(&events);
    // One second of slack on the start; the end is exact.
    assert_eq!(got.first().unwrap(), stamp(6_999).trim_end());
    assert_eq!(got.last().unwrap(), stamp(7_999).trim_end());
    let first_no = events.iter().find_map(|e| match e {
        ReadEvent::FirstLineNo { first_line_no, .. } => Some(*first_line_no),
        _ => None,
    });
    assert_eq!(first_no, Some(Some(7_000)));
}

#[test]
fn a_compressed_file_keeps_only_the_time_range_and_counts_the_first_line() {
    let Some(env) = env() else { return };
    std::fs::write(env.path("app.log.1.gz"), gzip(&(0..3_000).map(stamp).collect::<String>())).unwrap();
    std::fs::write(env.path("app.log"), "x\n").unwrap();
    let plan = env.plan(vec![env.generation(1, "app.log.1.gz")], Some(time(1_000)), Some(time(1_099)), 2_000_000);
    let (rx, _) = env.start(plan);
    let events = all_events(&rx);
    let got = lines_of(&events);
    assert_eq!(got.len(), 101);
    assert_eq!(got.first().unwrap(), stamp(999).trim_end());
    let counted: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            ReadEvent::FirstLineNo { first_line_no, .. } => Some(*first_line_no),
            _ => None,
        })
        .collect();
    assert_eq!(counted, [Some(1_000)]);
}

#[test]
fn the_full_read_limit_stops_the_read_and_says_so() {
    let Some(env) = env() else { return };
    std::fs::write(env.path("app.log"), (0..5_000).map(stamp).collect::<String>()).unwrap();
    std::fs::write(env.path("app.log.1.gz"), gzip(&(0..5_000).map(stamp).collect::<String>())).unwrap();
    let plan = env.plan(vec![env.generation(1, "app.log.1.gz"), env.generation(0, "app.log")], None, None, 7_000);
    let (rx, _) = env.start(plan);
    let events = all_events(&rx);
    assert_eq!(lines_of(&events).len(), 7_000);
    assert!(events.iter().any(|e| matches!(e, ReadEvent::Stopped(StopReason::FullReadLimit))));
    let ReadEvent::Done(meta) = events.last().unwrap() else { panic!() };
    assert!(!meta.is_complete());
    assert_eq!(meta.generations.last().unwrap().stop, Some(StopReason::FullReadLimit));
}

#[test]
fn cancel_keeps_what_was_read_and_pause_loses_nothing() {
    let Some(env) = env() else { return };
    std::fs::write(env.path("app.log"), (0..300_000).map(stamp).collect::<String>()).unwrap();
    let plan = env.plan(vec![env.generation(0, "app.log")], None, None, 2_000_000);
    let dir = plan.dir.clone();
    let (rx, ctl) = env.start(plan);
    ctl.set_paused(true);
    std::thread::sleep(Duration::from_millis(1_500));
    let mut seen = 0;
    while let Ok(e) = rx.try_recv() {
        if let ReadEvent::Lines { lines, .. } = e {
            seen += lines.len();
        }
    }
    // Paused after at most the first block.
    std::thread::sleep(Duration::from_millis(500));
    let before = rx.len();
    assert_eq!(before, 0, "lines arrived while paused");
    ctl.set_paused(false);
    ctl.cancel();
    let events = all_events(&rx);
    let kept = seen + lines_of(&events).len();
    assert!(events.iter().any(|e| matches!(e, ReadEvent::Stopped(StopReason::Cancelled))));
    let ReadEvent::Done(meta) = events.last().unwrap() else { panic!() };
    assert!(!meta.is_complete());
    assert_eq!(meta.total_lines(), kept);
    assert_eq!(std::fs::read_to_string(dir.join("gen-0.log")).unwrap().lines().count(), kept);
}

#[test]
fn read_newest_lines_cuts_the_oldest_and_says_older_lines_were_not_read() {
    use log_gpui::remote::full_read::{estimate_generations, fit_newest, total_estimate};
    let Some(env) = env() else { return };
    std::fs::write(env.path("app.log"), (5_000..6_000).map(stamp).collect::<String>()).unwrap();
    std::fs::write(env.path("app.log.1"), (0..5_000).map(stamp).collect::<String>()).unwrap();
    std::fs::write(env.path("app.log.2.gz"), gzip(&(0..100).map(stamp).collect::<String>())).unwrap();
    let mut plan_gens = vec![env.generation(2, "app.log.2.gz"), env.generation(1, "app.log.1"), env.generation(0, "app.log")];
    // Estimate over the real connection.
    let session = env.hub.handle().block_on(async {
        let store = env.store.clone();
        env.hub.session(&store, &env.server_id).await.unwrap()
    });
    let estimates = env.hub.handle().block_on(async {
        let sftp = session.open_sftp().await.unwrap();
        estimate_generations(&sftp, &plan_gens, None, None).await.unwrap()
    });
    let total = total_estimate(&estimates);
    assert!(total.approximate && total.over(2_000));
    fit_newest(&mut plan_gens, &estimates, 2_000);
    let plan = env.plan(plan_gens, None, None, 2_000);
    let (rx, _) = env.start(plan);
    let events = all_events(&rx);
    let got = lines_of(&events);
    // About 2,000 lines: all of the file and the end of `app.log.1`, whole lines.
    assert!(got.len() > 1_500 && got.len() <= 2_000, "{}", got.len());
    assert_eq!(got.last().unwrap(), stamp(5_999).trim_end());
    assert!(got.first().unwrap().contains("INFO line"));
    let ReadEvent::Done(meta) = events.last().unwrap() else { panic!() };
    assert_eq!(meta.generations.len(), 2);
    assert!(meta.generations[0].older_not_read);
    // The number of the first line read is the real one in `app.log.1`.
    let first = meta.generations[0].first_line_no.unwrap();
    assert!(got.first().unwrap().ends_with(&format!("line {}", first - 1)));
}

/// Drop every session of the test sshd (its listener stays up).
fn drop_sessions(env_dir: &str) -> bool {
    let Ok(pid) = std::fs::read_to_string(format!("{env_dir}/sshd.pid")) else { return false };
    std::process::Command::new("pkill").args(["-P", pid.trim()]).status().map(|s| s.success()).unwrap_or(false)
}

#[test]
fn a_lost_connection_is_picked_up_again_and_no_line_is_missing_or_repeated() {
    let Some(env) = env() else { return };
    let key_dir = std::env::var("SSH_TEST_DIR").unwrap();
    let total = 600_000u32;
    std::fs::write(env.path("app.log"), (0..total).map(|i| stamp(i % 80_000)).collect::<String>()).unwrap();
    let plan = env.plan(vec![env.generation(0, "app.log")], None, None, 2_000_000);
    let (rx, ctl) = env.start(plan);
    // Wait for the first lines, hold the read, drop the connection, go on.
    let mut events = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while !events.iter().any(|e| matches!(e, ReadEvent::Lines { .. })) {
        while let Ok(e) = rx.try_recv() {
            events.push(e);
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    ctl.set_paused(true);
    std::thread::sleep(Duration::from_millis(300));
    assert!(drop_sessions(&key_dir), "no session of the test sshd to drop");
    std::thread::sleep(Duration::from_millis(500));
    ctl.set_paused(false);
    events.extend(all_events(&rx));
    let got = lines_of(&events);
    assert_eq!(got.len(), total as usize);
    for (i, line) in got.iter().enumerate().step_by(997) {
        assert_eq!(line, stamp(i as u32 % 80_000).trim_end(), "line {i}");
    }
    let ReadEvent::Done(meta) = events.last().unwrap() else { panic!("{:?}", events.last()) };
    assert!(meta.is_complete());
}
