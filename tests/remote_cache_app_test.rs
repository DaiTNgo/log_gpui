//! The Remote Cache in the real `AppView` (openspec change
//! `remote-local-cache`, first stage), against a local sshd (see
//! `remote_hub_test.rs`): `SSH_TEST_DIR`, `SSH_TEST_PORT`. Skipped when
//! unset. Leaving a tab keeps what its Remote File holds; choosing the tab
//! again shows it at once and reads on only what was written since.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::remote::{AuthMethod, Server};
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};
use std::io::Write;
use std::time::{Duration, Instant};

fn add_server(h: &mut Harness) -> Option<String> {
    let dir = std::env::var("SSH_TEST_DIR").ok()?;
    let port = std::env::var("SSH_TEST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(2222);
    let mut server = Server::new("web1", "127.0.0.1", &std::env::var("USER").ok()?, AuthMethod::Key { path: format!("{dir}/userkey") });
    server.port = port;
    Some(h.view.update(&mut h.cx, |app, _| {
        let id = app.remote.servers.save(server).unwrap();
        *app.remote.shared_servers.lock().unwrap() = app.remote.servers.clone();
        id
    }))
}

/// Pump the live sources, trusting host keys, until `done`.
fn wait_for(h: &mut Harness, what: &str, mut done: impl FnMut(&log_gpui::app::AppView) -> bool) {
    h.cx.executor().allow_parking();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        h.view.update(&mut h.cx, |app, cx| {
            if let Some(request) = app.remote.host_prompts.pop_front() {
                let _ = request.reply.send(true);
            }
            app.pump_live(cx);
        });
        h.cx.run_until_parked();
        if h.view.read_with(&h.cx, |app, _| done(app)) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn numbered(range: std::ops::RangeInclusive<usize>) -> String {
    range.map(|i| format!("2026-09-05 {:02}:{:02}:{:02} [INFO] [api] line {i}\n", 10 + i / 3600, i / 60 % 60, i % 60)).collect()
}

fn rows(app: &log_gpui::app::AppView) -> Vec<(usize, String)> {
    app.rows_snapshot().iter().map(|e| (e.index, e.raw_line.to_string())).collect()
}

#[gpui::test]
fn a_remote_file_chosen_again_shows_its_kept_lines_and_reads_on_only_the_new_ones(cx: &mut TestAppContext) {
    chosen_again(cx, false);
}

/// The same, with the lines read through the cache while the tab is open
/// (stage 2): no text in memory.
#[gpui::test]
fn read_through_its_cache_a_remote_file_chosen_again_reads_on_only_the_new_ones(cx: &mut TestAppContext) {
    chosen_again(cx, true);
}

fn chosen_again(cx: &mut TestAppContext, through_cache: bool) {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, _| app.remote_cache_index = through_cache);
    let Some(server_id) = add_server(&mut h) else { return };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.log");
    std::fs::write(&path, numbered(1..=3_000)).unwrap();
    let remote_path = path.to_string_lossy().to_string();
    let space = h.view.update(&mut h.cx, |app, cx| {
        let project = app.workspace_state.active_project_mut().unwrap();
        let space = project.create_log_space("Remote", LogSpaceKind::Live).unwrap();
        space.add_remote_files(&[(server_id.clone(), "web1".into(), remote_path.clone())]).unwrap();
        let id = space.id.clone();
        app.open_log_space_tab(&id, cx);
        id
    });
    wait_for(&mut h, "every line, numbered, and a Checkpoint", |app| {
        let cached = app.live.cached.values().any(|c| c.cache().meta().generations.last().is_some_and(|g| g.checkpoint.head_hash.is_some()));
        app.view.len() == 3_000 && app.rows_snapshot()[0].index == 1 && (!app.live.checkpoints.is_empty() || cached)
    });

    // Another tab: the Remote File's tab goes Dormant and its lines are kept.
    let other = h.dir.path().join("local.log");
    std::fs::write(&other, numbered(1..=10)).unwrap();
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Local", LogSpaceKind::Static, &[ImportSource::Disk(other.to_string_lossy().into_owned())], cx)
    });
    h.cx.run_until_parked();
    let cached = h.view.read_with(&h.cx, |app, _| app.remote_cache_root_for_test());
    assert!(walk_lines(&cached) >= 3_000, "kept on disk");

    // Written on the Server meanwhile, then the tab is chosen again.
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(numbered(3_001..=3_005).as_bytes()).unwrap();
    h.view.update(&mut h.cx, |app, cx| {
        let at = app.workspace_state.active_project().unwrap().working_set.tabs().iter().position(|t| t.target.space_id() == Some(space.as_str())).unwrap();
        app.switch_to_tab(at, cx);
    });
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.view.len(), 3_000, "shown at once, before the Server answers");
        assert_eq!(app.rows_snapshot()[0].index, 1, "with the numbers they have on the Server");
    });
    wait_for(&mut h, "the five new lines", |app| app.view.len() >= 3_005);
    h.view.read_with(&h.cx, |app, _| {
        let want: Vec<(usize, String)> = numbered(1..=3_005).lines().enumerate().map(|(i, l)| (i + 1, l.to_string())).collect();
        let have = rows(app);
        let first_diff = have.iter().zip(&want).position(|(a, b)| a != b);
        assert_eq!((have.len(), first_diff), (want.len(), None), "nothing read twice, nothing missing: {:?}", first_diff.map(|i| (&have[i], &want[i])));
        if through_cache {
            let parsed = app.parsed_files.values().find(|p| p.indexed.is_some() && p.live.is_none()).expect("read through its cache");
            assert!(parsed.entries.is_empty() && parsed.lines.is_empty(), "no text in memory");
        }
    });
}

/// Lines in the `.log` files under `dir`.
fn walk_lines(dir: &std::path::Path) -> usize {
    let mut n = 0;
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = entry.path();
        if p.is_dir() {
            n += walk_lines(&p);
        } else if p.extension().is_some_and(|e| e == "log") {
            n += std::fs::read_to_string(&p).unwrap_or_default().lines().count();
        }
    }
    n
}
