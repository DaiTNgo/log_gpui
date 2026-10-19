//! Live Globs against a local sshd (see `remote_hub_test.rs`): `SSH_TEST_DIR`,
//! `SSH_TEST_PORT`. The test loop does what the app does with a watcher's
//! listings: `plan_glob`, then a source per file that was added.

use log_gpui::live::remote::GlobInodes;
use log_gpui::live::{BreakKind, LineSource, RemoteFileSource, SourceEvent, SourceHandle, SourceStatus};
use log_gpui::remote::glob_plan::{Listing, SpaceFile, plan_glob};
use log_gpui::remote::glob_watcher::{GlobEvent, GlobWatcher, WatchCommand, WatcherHandle};
use log_gpui::remote::*;
use log_gpui::workspace::log_space::LiveGlob;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Env {
    hub: Arc<RemoteHub>,
    servers: Arc<Mutex<ServerStore>>,
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
    Some(Env { hub, servers: Arc::new(Mutex::new(store)), server_id, dir: tempfile::tempdir().unwrap() })
}

/// A Live Glob being run the way the app runs one.
struct Harness {
    watcher: WatcherHandle,
    events: async_channel::Receiver<GlobEvent>,
    glob: LiveGlob,
    inodes: GlobInodes,
    /// Files added so far, by path, with their sources.
    files: Vec<(String, SourceHandle)>,
    limit: usize,
}

fn glob_for(env: &Env, pattern: &str) -> LiveGlob {
    LiveGlob {
        glob_id: "glob-1".into(),
        server_id: env.server_id.clone(),
        dir: env.dir.path().to_string_lossy().to_string(),
        pattern: pattern.into(),
        excluded: Vec::new(),
        stopped: false,
    }
}

impl Harness {
    fn start(env: &Env, pattern: &str) -> Self {
        let glob = glob_for(env, pattern);
        let (tx, events) = async_channel::unbounded();
        let watcher = GlobWatcher {
            hub: env.hub.clone(),
            servers: env.servers.clone(),
            server_id: env.server_id.clone(),
            dir: glob.dir.clone(),
            patterns: vec![pattern.to_string()],
            interval: Duration::from_millis(500),
            events: tx,
        }
        .spawn();
        Self { watcher, events, glob, inodes: GlobInodes::default(), files: Vec::new(), limit: 50 }
    }

    fn space_files(&self) -> Vec<SpaceFile> {
        self.files.iter().map(|(p, _)| SpaceFile { path: p.clone(), from_glob: true, gone: false }).collect()
    }

    /// Wait for a listing and act on it; returns what was added.
    fn step(&mut self, env: &Env) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let event = match self.events.try_recv() {
                Ok(event) => event,
                Err(_) => {
                    assert!(Instant::now() < deadline, "no listing");
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }
            };
            let GlobEvent::Listing { entries, inodes, archives, .. } = event else { continue };
            // Same seeding as the app.
            {
                let mut known = self.inodes.lock().unwrap();
                for (path, _) in &self.files {
                    let name = path.rsplit('/').next().unwrap();
                    if let Some(i) = inodes.get(name) {
                        known.entry(*i).or_insert_with(|| path.clone());
                    }
                }
            }
            let read = self.inodes.lock().unwrap().clone();
            let plan = plan_glob(
                &self.glob,
                &self.space_files(),
                &Listing { entries: &entries, inodes: &inodes, archives: &archives },
                &read,
                self.limit,
            );
            for path in &plan.add {
                let handle = RemoteFileSource {
                    hub: env.hub.clone(),
                    servers: env.servers.clone(),
                    server_id: env.server_id.clone(),
                    path: path.clone(),
                    name: format!("web1:{path}"),
                    window: 100_000,
                    start_time: None,
                    glob_inodes: Some(self.inodes.clone()),
                }
                .spawn();
                self.files.push((path.clone(), handle));
            }
            return plan.add;
        }
    }

    /// Step until `done` holds for what was added in one step, or time runs out.
    fn until(&mut self, env: &Env, mut done: impl FnMut(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done(self) {
            assert!(Instant::now() < deadline, "timed out");
            self.step(env);
        }
    }

    fn source(&self, path: &str) -> &SourceHandle {
        &self.files.iter().find(|(p, _)| p == path).expect("file added").1
    }

    fn paths(&self) -> Vec<String> {
        let mut paths: Vec<String> = self.files.iter().map(|(p, _)| p.clone()).collect();
        paths.sort();
        paths
    }
}

fn wait_event(h: &SourceHandle, mut want: impl FnMut(&SourceEvent) -> bool) -> SourceEvent {
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

fn path_in(env: &Env, name: &str) -> String {
    env.dir.path().join(name).to_string_lossy().to_string()
}

#[test]
fn a_new_matching_file_is_added_within_ten_seconds() {
    let Some(env) = env() else { return };
    std::fs::write(env.dir.path().join("a.log"), "one\n").unwrap();
    std::fs::write(env.dir.path().join("skip.txt"), "x\n").unwrap();
    let mut h = Harness::start(&env, "*.log");
    h.until(&env, |h| h.paths() == vec![path_in(&env, "a.log")]);
    std::fs::write(env.dir.path().join("b.log"), "two\n").unwrap();
    h.until(&env, |h| h.paths().len() == 2);
    assert_eq!(h.paths(), vec![path_in(&env, "a.log"), path_in(&env, "b.log")]);
    h.watcher.stop();
}

#[test]
fn logrotate_with_a_wide_glob_adds_only_the_live_file() {
    let Some(env) = env() else { return };
    let log = env.dir.path().join("app.log");
    std::fs::write(&log, "old 1\nold 2\n").unwrap();
    std::fs::write(env.dir.path().join("app.log.2.gz"), [0x1f, 0x8b, 8, 0, 0, 0, 0, 0]).unwrap();
    let mut h = Harness::start(&env, "app.log*");
    h.until(&env, |h| !h.paths().is_empty());
    assert_eq!(h.paths(), vec![path_in(&env, "app.log")]);
    let source = h.source(&path_in(&env, "app.log")).clone();
    wait_event(&source, |e| matches!(e, SourceEvent::Tail { .. }));
    // logrotate: rename, create anew.
    std::fs::rename(&log, env.dir.path().join("app.log.1")).unwrap();
    std::fs::write(&log, "new 1\n").unwrap();
    wait_event(&source, |e| matches!(e, SourceEvent::Break { kind: BreakKind::Rotated }));
    // Several listings later, app.log.1 was never added.
    for _ in 0..3 {
        assert!(h.step(&env).is_empty());
    }
    assert_eq!(h.paths(), vec![path_in(&env, "app.log")]);
    h.watcher.stop();
}

#[test]
fn a_renamed_file_is_not_read_twice() {
    let Some(env) = env() else { return };
    let current = env.dir.path().join("current.log");
    std::fs::write(&current, "batch 1\n").unwrap();
    let mut h = Harness::start(&env, "*.log");
    h.until(&env, |h| !h.paths().is_empty());
    let source = h.source(&path_in(&env, "current.log")).clone();
    wait_event(&source, |e| matches!(e, SourceEvent::Inode(Some(_))));
    // Not a Rotated File by name: the job renames it and starts over.
    std::fs::rename(&current, env.dir.path().join("old-batch.log")).unwrap();
    std::fs::write(&current, "batch 2\n").unwrap();
    wait_event(&source, |e| matches!(e, SourceEvent::Break { kind: BreakKind::Rotated }));
    for _ in 0..3 {
        assert!(h.step(&env).is_empty(), "old-batch.log must not be added");
    }
    assert_eq!(h.paths(), vec![path_in(&env, "current.log")]);
    h.watcher.stop();
}

#[test]
fn a_deleted_file_goes_gone_and_a_file_at_its_path_is_a_new_generation() {
    let Some(env) = env() else { return };
    let path = env.dir.path().join("a.log");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    let mut h = Harness::start(&env, "*.log");
    h.until(&env, |h| !h.paths().is_empty());
    let source = h.source(&path_in(&env, "a.log")).clone();
    wait_event(&source, |e| matches!(e, SourceEvent::Status(SourceStatus::Live)));
    std::fs::remove_file(&path).unwrap();
    wait_event(&source, |e| matches!(e, SourceEvent::Break { kind: BreakKind::Deleted }));
    wait_event(&source, |e| matches!(e, SourceEvent::Status(SourceStatus::Gone)));
    std::fs::write(&path, "fresh\n").unwrap();
    wait_event(&source, |e| matches!(e, SourceEvent::Status(SourceStatus::Live)));
    h.watcher.stop();
}

#[test]
fn a_file_created_while_the_watcher_was_off_is_read_from_its_start_after_resuming() {
    let Some(env) = env() else { return };
    std::fs::write(env.dir.path().join("a.log"), "one\n").unwrap();
    let mut h = Harness::start(&env, "*.log");
    h.until(&env, |h| h.paths().len() == 1);
    // Paused or stopped: the watcher is stopped and no file is added.
    h.watcher.stop();
    let body: String = (1..=200).map(|i| format!("line {i}\n")).collect();
    std::fs::write(env.dir.path().join("c.log"), body).unwrap();
    std::thread::sleep(Duration::from_millis(1_500));
    assert_eq!(h.paths().len(), 1);
    // Resume: a watcher lists at once and the new file is read in full.
    let mut resumed = Harness::start(&env, "*.log");
    resumed.files = std::mem::take(&mut h.files);
    resumed.inodes = h.inodes.clone();
    resumed.until(&env, |h| h.paths().len() == 2);
    let c = path_in(&env, "c.log");
    let SourceEvent::Tail { lines, .. } = wait_event(resumed.source(&c), |e| matches!(e, SourceEvent::Tail { .. })) else {
        unreachable!()
    };
    assert_eq!(lines.len(), 200);
    assert_eq!(lines[0], "line 1");
    resumed.watcher.stop();
}

#[test]
fn a_missing_folder_is_reported_and_the_watcher_goes_on_when_it_appears() {
    let Some(env) = env() else { return };
    let sub = env.dir.path().join("later");
    let (tx, events) = async_channel::unbounded();
    let watcher = GlobWatcher {
        hub: env.hub.clone(),
        servers: env.servers.clone(),
        server_id: env.server_id.clone(),
        dir: sub.to_string_lossy().to_string(),
        patterns: vec!["*.log".into()],
        interval: Duration::from_millis(300),
        events: tx,
    }
    .spawn();
    let wait = |want: fn(&GlobEvent) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match events.try_recv() {
                Ok(e) if want(&e) => return e,
                Ok(_) => {}
                Err(_) => {
                    assert!(Instant::now() < deadline, "timed out");
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    };
    wait(|e| matches!(e, GlobEvent::FolderMissing { .. }));
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("x.log"), "x\n").unwrap();
    let GlobEvent::Listing { entries, .. } = wait(|e| matches!(e, GlobEvent::Listing { .. })) else { unreachable!() };
    assert_eq!(entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["x.log"]);
    watcher.send(WatchCommand::SetInterval(Duration::from_millis(200)));
    watcher.stop();
}
