//! Open in full in the real `AppView`, against a local sshd (see
//! `remote_hub_test.rs`): `SSH_TEST_DIR`, `SSH_TEST_PORT`. Skipped when unset.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::full_read::{Destination, FullReadRequest};
use log_gpui::remote::compress::Compression;
use log_gpui::remote::full_read::GenerationPlan;
use log_gpui::remote::{AuthMethod, Server};
use std::time::{Duration, Instant};

fn ssh() -> Option<(String, u16, String)> {
    let dir = std::env::var("SSH_TEST_DIR").ok()?;
    Some((
        format!("{dir}/userkey"),
        std::env::var("SSH_TEST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(2222),
        std::env::var("USER").ok()?,
    ))
}

fn add_server(h: &mut Harness, name: &str) -> String {
    let (key, port, user) = ssh().unwrap();
    let mut server = Server::new(name, "127.0.0.1", &user, AuthMethod::Key { path: key });
    server.port = port;
    h.view.update(&mut h.cx, |app, _| {
        let id = app.remote.servers.save(server).unwrap();
        *app.remote.shared_servers.lock().unwrap() = app.remote.servers.clone();
        id
    })
}

fn wait_for(h: &mut Harness, mut done: impl FnMut(&log_gpui::app::AppView) -> bool) {
    h.cx.executor().allow_parking();
    let deadline = Instant::now() + Duration::from_secs(30);
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

fn stamp(i: u32) -> String {
    format!("2026-10-19 {:02}:{:02}:{:02} INFO line {i}\n", i / 3600, i / 60 % 60, i % 60)
}

fn plan(dir: &std::path::Path, distance: u32, name: &str) -> GenerationPlan {
    let path = dir.join(name);
    let size = std::fs::metadata(&path).unwrap().len();
    GenerationPlan {
        distance,
        name: name.into(),
        path: path.to_string_lossy().to_string(),
        size,
        modified: None,
        compression: Compression::None,
        start_byte: None,
        skip_lines: 0,
        older_dropped: false,
    }
}

#[gpui::test]
fn open_in_full_makes_an_imported_space_with_a_copy_that_opens_again(cx: &mut TestAppContext) {
    if ssh().is_none() {
        return;
    }
    let mut h = common::open_app(cx);
    let server_id = add_server(&mut h, "web1");
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("app.log"), (100..160).map(stamp).collect::<String>()).unwrap();
    std::fs::write(root.path().join("app.log.1"), (0..100).map(stamp).collect::<String>()).unwrap();
    let path = root.path().join("app.log").to_string_lossy().to_string();
    let request = FullReadRequest {
        server_id,
        server_name: "web1".into(),
        path: path.clone(),
        generations: vec![plan(root.path(), 1, "app.log.1"), plan(root.path(), 0, "app.log")],
        start: None,
        to: None,
        start_text: None,
        to_text: None,
        destination: Destination::NewSpace("Checkout (full)".into()),
    };
    let file_id = h.view.update(&mut h.cx, |app, cx| app.start_full_read(request, cx)).unwrap();
    wait_for(&mut h, |app| app.live.copy_reads.is_empty());

    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.all_entries.len(), 160);
        assert!(!app.live.follow, "opens with Follow off");
        let space = last_space(app);
        assert_eq!(space.name, "Checkout (full)");
        let file = space.file(&file_id).unwrap();
        assert!(file.is_remote_copy() && file.display_name == format!("web1:{path}"));
        assert_eq!(file.generation, 1);
        // The first row is line 1 of `app.log.1`; the file's own first line
        // follows with a Break between.
        assert_eq!(app.all_entries[0].index, 1);
        assert!(app.all_entries[100].break_before.is_some());
        assert_eq!(app.all_entries[100].index, 1);
    });

    // Reopening reads the folder, with no connection involved.
    h.view.update(&mut h.cx, |app, _| {
        app.parsed_files.remove(&file_id);
        let file = last_space(app).file(&file_id).unwrap().clone();
        app.ensure_copy_loaded(&file);
        let parsed = app.parsed_files.get(&file_id).unwrap();
        assert_eq!(parsed.entries.len(), 160);
        assert!(parsed.load_error.is_none());
        assert_eq!(last_space(app).file(&file_id).unwrap().generation, 1);
    });
}

#[gpui::test]
fn cancel_keeps_the_lines_and_ends_with_a_break(cx: &mut TestAppContext) {
    if ssh().is_none() {
        return;
    }
    let mut h = common::open_app(cx);
    let server_id = add_server(&mut h, "web1");
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("app.log"), (0..400_000).map(|i| stamp(i % 80_000)).collect::<String>()).unwrap();
    let path = root.path().join("app.log").to_string_lossy().to_string();
    let request = FullReadRequest {
        server_id,
        server_name: "web1".into(),
        path,
        generations: vec![plan(root.path(), 0, "app.log")],
        start: None,
        to: None,
        start_text: None,
        to_text: None,
        destination: Destination::NewSpace("Big (full)".into()),
    };
    let file_id = h.view.update(&mut h.cx, |app, cx| app.start_full_read(request, cx)).unwrap();
    h.view.update(&mut h.cx, |app, _| app.set_copy_reads_paused_for_test(&file_id, true));
    h.view.update(&mut h.cx, |app, _| {
        app.cancel_copy_read(&file_id);
        app.set_copy_reads_paused_for_test(&file_id, false);
    });
    wait_for(&mut h, |app| app.live.copy_reads.is_empty());
    h.view.read_with(&h.cx, |app, _| {
        let parsed = app.parsed_files.get(&file_id).unwrap();
        let last = parsed.entries.last();
        if let Some(last) = last {
            let mark = last.break_after.as_ref().expect("a Break below the last line");
            assert_eq!(mark.label, "read cancelled");
        }
        assert!(parsed.entries.len() < 400_000);
    });
}

fn last_space(app: &log_gpui::app::AppView) -> &log_gpui::workspace::log_space::LogSpace {
    app.workspace_state.active_project().unwrap().log_spaces.last().unwrap()
}

#[gpui::test]
fn removing_the_copy_deletes_its_folder_and_orphans_are_swept(cx: &mut TestAppContext) {
    if ssh().is_none() {
        return;
    }
    let mut h = common::open_app(cx);
    let server_id = add_server(&mut h, "web1");
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("app.log"), (0..50).map(stamp).collect::<String>()).unwrap();
    let request = FullReadRequest {
        server_id,
        server_name: "web1".into(),
        path: root.path().join("app.log").to_string_lossy().to_string(),
        generations: vec![plan(root.path(), 0, "app.log")],
        start: None,
        to: None,
        start_text: None,
        to_text: None,
        destination: Destination::NewSpace("Copy".into()),
    };
    let file_id = h.view.update(&mut h.cx, |app, cx| app.start_full_read(request, cx)).unwrap();
    wait_for(&mut h, |app| app.live.copy_reads.is_empty());
    let (space_id, dir) = h.view.read_with(&h.cx, |app, _| {
        let space = last_space(app);
        (space.id.clone(), std::path::PathBuf::from(space.file(&file_id).unwrap().copy.clone().unwrap()))
    });
    assert!(dir.join("gen-0.log").exists());
    // A folder nobody holds is swept.
    let orphan = log_gpui::remote::copy::create_dir(dir.parent().unwrap()).unwrap();
    let swept = h.view.read_with(&h.cx, |app, _| log_gpui::remote::copy::sweep(&app.copies_root_for_test(), &app.copy_dirs_in_use_for_test()));
    assert_eq!(swept, 1);
    assert!(!orphan.exists() && dir.exists());
    h.view.update(&mut h.cx, |app, cx| app.remove_file_from_space(&space_id, &file_id, cx));
    assert!(!dir.exists());
}
