//! Load more, Open in full, the Live Window, "N lines held" and Clear are
//! for Remote Files only (openspec change `static-live-log-spaces`, 4.2): a
//! local file of a Live Log Space is read whole from disk. The mixed test
//! needs the local sshd (`SSH_TEST_DIR`, `SSH_TEST_PORT`); it is skipped
//! when unset.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::remote::{AuthMethod, Server};
use std::time::{Duration, Instant};

fn wait_for(h: &mut Harness, mut done: impl FnMut(&log_gpui::app::AppView) -> bool) {
    h.cx.executor().allow_parking();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        h.cx.run_until_parked();
        let finished = h.view.update(&mut h.cx, |app, cx| {
            if let Some(request) = app.remote.host_prompts.pop_front() {
                let _ = request.reply.send(true);
            }
            app.pump_live(cx);
            done(app)
        });
        if finished {
            return;
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn active_space(app: &log_gpui::app::AppView) -> &log_gpui::workspace::log_space::LogSpace {
    let project = app.workspace_state.active_project().unwrap();
    let id = project.working_set.active_tab().unwrap().target.space_id().unwrap();
    project.log_space(id).unwrap()
}

#[gpui::test]
fn a_live_space_of_local_files_has_no_remote_items(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_live_lines(cx, &common::sample_lines(30));
    wait_for(&mut h, |app| app.view.len() == 30);
    let space_id = h.view.read_with(&h.cx, |app, _| {
        let space = active_space(app);
        assert!(app.active_space_is_live(), "Live/Pause and Follow stay");
        assert!(!space.has_remote(), "no Open in full, Download all or Clear all files");
        assert!(space.files.iter().all(|f| app.hold_info(&f.file_id).is_none()), "no Load more, Live Window or lines held");
        space.id.clone()
    });
    h.view.update(&mut h.cx, |app, cx| app.request_clear_all_files(&space_id, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.view.len(), 30, "Clear drops no local line"));
}

fn stamp(i: u32) -> String {
    format!("2026-10-19 10:{:02}:{:02} INFO line {i}\n", i / 60 % 60, i % 60)
}

#[gpui::test]
fn clear_in_a_mixed_live_space_drops_only_the_remote_lines(cx: &mut TestAppContext) {
    let Ok(dir) = std::env::var("SSH_TEST_DIR") else { return };
    let port = std::env::var("SSH_TEST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(2222);
    let mut h = common::open_app(cx);
    let mut server = Server::new("web1", "127.0.0.1", &std::env::var("USER").unwrap(), AuthMethod::Key { path: format!("{dir}/userkey") });
    server.port = port;
    let server_id = h.view.update(&mut h.cx, |app, _| {
        let id = app.remote.servers.save(server).unwrap();
        *app.remote.shared_servers.lock().unwrap() = app.remote.servers.clone();
        id
    });
    let root = tempfile::tempdir().unwrap();
    let (remote, local) = (root.path().join("app.log"), root.path().join("api.log"));
    std::fs::write(&remote, (0..20).map(stamp).collect::<String>()).unwrap();
    std::fs::write(&local, (100..110).map(stamp).collect::<String>()).unwrap();
    let remote_path = remote.to_string_lossy().to_string();
    let space_id = h
        .view
        .update(&mut h.cx, |app, cx| app.create_remote_log_space("Mixed", vec![(server_id, "web1".into(), remote_path)], vec![], vec![], cx))
        .unwrap();
    h.view.update(&mut h.cx, |app, cx| app.add_files_to_space(&space_id, vec![local.to_string_lossy().to_string()], cx));
    wait_for(&mut h, |app| app.view.len() == 30);
    let (remote_id, local_id) = h.view.read_with(&h.cx, |app, _| {
        let space = active_space(app);
        assert!(space.has_remote());
        let id = |remote: bool| space.files.iter().find(|f| f.remote.is_some() == remote).unwrap().file_id.clone();
        (id(true), id(false))
    });
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.hold_info(&remote_id).is_some(), "the Remote File holds a Live Window");
        assert!(app.hold_info(&local_id).is_none(), "the local file, in its File View too, has none of it");
    });
    h.view.update(&mut h.cx, |app, cx| app.request_clear_all_files(&space_id, cx));
    wait_for(&mut h, |app| app.parsed_files[&remote_id].is_empty());
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.view.len(), 10, "every line of api.log stays");
        assert!(app.parsed_files[&local_id].indexed.is_some());
    });
}
