//! The file picker in the real `AppView`, against a local sshd (see
//! `remote_hub_test.rs`): `SSH_TEST_DIR`, `SSH_TEST_PORT`. Skipped when unset.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::remote_picker::{PickerTarget, Staged};
use log_gpui::remote::browse::{SortKey, SortOrder};
use log_gpui::remote::{AuthMethod, Server};
use log_gpui::workspace::log_space::LogSpaceKind;
use std::time::{Duration, Instant};

struct Ssh {
    key: String,
    port: u16,
    user: String,
}

fn ssh() -> Option<Ssh> {
    let dir = std::env::var("SSH_TEST_DIR").ok()?;
    Some(Ssh {
        key: format!("{dir}/userkey"),
        port: std::env::var("SSH_TEST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(2222),
        user: std::env::var("USER").ok()?,
    })
}

/// Add a Server to the app without touching the user's own config.
fn add_server(h: &mut Harness, ssh: &Ssh, name: &str) -> String {
    let mut server = Server::new(name, "127.0.0.1", &ssh.user, AuthMethod::Key { path: ssh.key.clone() });
    server.port = ssh.port;
    h.view.update(&mut h.cx, |app, _| {
        let id = app.remote.servers.save(server).unwrap();
        *app.remote.shared_servers.lock().unwrap() = app.remote.servers.clone();
        id
    })
}

/// Let background work finish: trust host keys and wait for `done`.
fn wait_for(h: &mut Harness, mut done: impl FnMut(&log_gpui::app::AppView) -> bool) {
    h.cx.executor().allow_parking();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        h.cx.run_until_parked();
        let finished = h.view.update(&mut h.cx, |app, _| {
            if let Some(request) = app.remote.host_prompts.pop_front() {
                let _ = request.reply.send(true);
            }
            done(app)
        });
        if finished {
            return;
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn picker_dir(h: &mut Harness) -> String {
    h.view.read_with(&h.cx, |app, _| app.remote.picker.as_ref().map(|p| p.dir.clone()).unwrap_or_default())
}

fn loaded(app: &log_gpui::app::AppView) -> bool {
    app.remote.picker.as_ref().is_some_and(|p| p.busy.is_none())
}

#[gpui::test]
fn picker_remembers_the_last_folder_sorts_filters_and_adds_on_two_servers(cx: &mut TestAppContext) {
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    let web1 = add_server(&mut h, &ssh, "web1");
    let web2 = add_server(&mut h, &ssh, "web2");
    let root = tempfile::tempdir().unwrap();
    let logs = root.path().join("logs");
    std::fs::create_dir(&logs).unwrap();
    std::fs::write(logs.join("app-1.log"), "x\n").unwrap();
    std::fs::write(logs.join("app-2.log"), "xx\n").unwrap();
    std::fs::write(logs.join("notes.txt"), "xxx\n").unwrap();
    std::fs::create_dir(logs.join("sub")).unwrap();
    let logs_path = logs.to_string_lossy().to_string();

    // Open at home, then go to `logs`.
    h.view.update(&mut h.cx, |app, cx| app.open_remote_picker(PickerTarget::NewSpace, cx));
    wait_for(&mut h, loaded);
    let home = picker_dir(&mut h);
    assert!(!home.is_empty() && home != logs_path);
    let target = logs_path.clone();
    h.view.update(&mut h.cx, |app, cx| app.picker_load(Some(target), cx));
    wait_for(&mut h, |app| loaded(app) && app.remote.picker.as_ref().unwrap().dir.ends_with("logs"));

    // Filter: substring, then glob; folders first; sort by size, descending.
    h.view.update(&mut h.cx, |app, _| app.remote.picker.as_mut().unwrap().filter = "APP".into());
    let names = h.view.read_with(&h.cx, |app, _| {
        app.remote.picker.as_ref().unwrap().visible_entries().iter().map(|e| e.name.clone()).collect::<Vec<_>>()
    });
    assert_eq!(names, ["app-1.log", "app-2.log"]);
    h.view.update(&mut h.cx, |app, _| app.remote.picker.as_mut().unwrap().filter = "*.txt".into());
    let names = h.view.read_with(&h.cx, |app, _| {
        app.remote.picker.as_ref().unwrap().visible_entries().iter().map(|e| e.name.clone()).collect::<Vec<_>>()
    });
    assert_eq!(names, ["notes.txt"]);
    h.view.update(&mut h.cx, |app, _| app.remote.picker.as_mut().unwrap().filter.clear());
    h.view.update(&mut h.cx, |app, cx| {
        app.picker_sort_by(SortKey::Size, cx);
        app.picker_sort_by(SortKey::Size, cx);
    });
    let (names, sort) = h.view.read_with(&h.cx, |app, _| {
        let p = app.remote.picker.as_ref().unwrap();
        (p.visible_entries().iter().map(|e| e.name.clone()).collect::<Vec<_>>(), p.sort)
    });
    assert_eq!(names, ["sub", "notes.txt", "app-2.log", "app-1.log"]);
    assert_eq!(sort, SortOrder { key: SortKey::Size, descending: true });

    // Close and reopen on the same Server: back in `logs`, with the sort kept.
    h.view.update(&mut h.cx, |app, cx| {
        app.close_remote_picker(cx);
        app.open_remote_picker(PickerTarget::NewSpace, cx);
    });
    wait_for(&mut h, |app| loaded(app) && app.remote.picker.as_ref().unwrap().dir.ends_with("logs"));
    let sort = h.view.read_with(&h.cx, |app, _| app.remote.picker.as_ref().unwrap().sort);
    assert_eq!(sort.key, SortKey::Size);

    // A remembered folder that is gone: home, with a note.
    let gone = root.path().join("gone");
    std::fs::create_dir(&gone).unwrap();
    let gone_path = gone.to_string_lossy().to_string();
    let id = web1.clone();
    h.view.update(&mut h.cx, |app, _| app.remote.picker_state.set_last_dir(&id, &gone_path));
    std::fs::remove_dir(&gone).unwrap();
    h.view.update(&mut h.cx, |app, cx| {
        app.close_remote_picker(cx);
        app.open_remote_picker(PickerTarget::NewSpace, cx);
    });
    wait_for(&mut h, loaded);
    let (dir, error) = h.view.read_with(&h.cx, |app, _| {
        let p = app.remote.picker.as_ref().unwrap();
        (p.dir.clone(), p.error.clone())
    });
    assert_eq!(dir, home);
    assert_eq!(error, Some(format!("{gone_path} is no longer available")));

    // A folder typed in the path field opens instead of being refused.
    h.view.update(&mut h.cx, |app, cx| app.picker_add_text(logs_path.clone(), cx));
    wait_for(&mut h, |app| loaded(app) && app.remote.picker.as_ref().unwrap().dir.ends_with("logs"));
    h.view.read_with(&h.cx, |app, _| assert!(app.remote.picker.as_ref().unwrap().staged.is_empty()));

    // A glob on two Servers: two Live Globs waiting.
    let (a, b) = (web1.clone(), web2.clone());
    h.view.update(&mut h.cx, |app, cx| {
        app.picker_toggle_server(&b, cx);
        app.picker_add_text(format!("{logs_path}/app-*.log"), cx);
    });
    wait_for(&mut h, |app| loaded(app) && app.remote.picker.as_ref().unwrap().staged.len() == 2);
    let staged = h.view.read_with(&h.cx, |app, _| app.remote.picker.as_ref().unwrap().staged.clone());
    assert!(staged.contains(&Staged::Glob { server_id: a.clone(), dir: logs_path.clone(), pattern: "app-*.log".into() }));
    assert!(staged.contains(&Staged::Glob { server_id: b.clone(), dir: logs_path.clone(), pattern: "app-*.log".into() }));

    // A folder missing for one of them is reported with its name.
    h.view.update(&mut h.cx, |app, cx| app.picker_add_text(format!("{}/nowhere/x.log", root.path().display()), cx));
    wait_for(&mut h, |app| loaded(app) && app.remote.picker.as_ref().unwrap().error.is_some());
    let error = h.view.read_with(&h.cx, |app, _| app.remote.picker.as_ref().unwrap().error.clone().unwrap());
    assert!(error.contains("web1: The folder") && error.contains("web2: The folder"), "{error}");

    // Done: both globs are remembered as Recent Paths, newest first.
    h.view.update(&mut h.cx, |app, cx| app.picker_confirm(cx));
    let (recent, globs) = h.view.read_with(&h.cx, |app, _| {
        (app.remote.picker_state.recent(&a).to_vec(), app.remote.new_space_globs.len())
    });
    assert_eq!(globs, 2);
    assert_eq!(recent.len(), 1);
    assert!(recent[0].is_glob && recent[0].path.ends_with("app-*.log"));
    // Recent Paths and the folder are on disk, beside servers.json.
    let on_disk = std::fs::read_to_string(h.dir.path().join("picker_state.json")).unwrap();
    assert!(on_disk.contains("app-*.log") && on_disk.contains("last_dir"));
    // An existing Remote Log Space keeps the glob in its model.
    let space_id = h.view.update(&mut h.cx, |app, cx| {
        let globs = std::mem::take(&mut app.remote.new_space_globs);
        app.create_remote_log_space("Prod", Vec::new(), globs, Vec::new(), cx)
    });
    let space_id = space_id.expect("space created");
    h.view.read_with(&h.cx, |app, _| {
        let space = app.workspace_state.active_project().unwrap().log_space(&space_id).unwrap();
        assert_eq!(space.kind, LogSpaceKind::Remote);
        assert_eq!(space.globs.len(), 2);
    });
}

fn space_paths(app: &log_gpui::app::AppView, space_id: &str) -> Vec<String> {
    let space = app.workspace_state.active_project().unwrap().log_space(space_id).unwrap();
    let mut paths: Vec<String> = space.files.iter().filter_map(|f| f.remote.as_ref().map(|r| r.path().unwrap_or_default().to_string())).collect();
    paths.sort();
    paths
}

#[gpui::test]
fn a_live_glob_in_the_app_adds_new_files_stops_resumes_and_is_removed(cx: &mut TestAppContext) {
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    let web1 = add_server(&mut h, &ssh, "web1");
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().to_string_lossy().to_string();
    std::fs::write(root.path().join("a.log"), "one\n").unwrap();
    std::fs::write(root.path().join("keep.txt"), "x\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.set_glob_interval(2, cx));
    let globs = vec![(web1.clone(), "web1".to_string(), dir.clone(), "*.log".to_string())];
    let manual = vec![(web1.clone(), "web1".to_string(), format!("{dir}/keep.txt"))];
    let space_id = h
        .view
        .update(&mut h.cx, |app, cx| app.create_remote_log_space("Prod", manual, globs, Vec::new(), cx))
        .expect("space");
    let path = |name: &str| format!("{dir}/{name}");

    // The files that match now are added, the others are not.
    let sid = space_id.clone();
    wait_for(&mut h, |app| space_paths(app, &sid).contains(&path("a.log")));
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(space_paths(app, &space_id), vec![path("a.log"), path("keep.txt")]);
    });

    // A file created later joins within two scans.
    std::fs::write(root.path().join("b.log"), "two\n").unwrap();
    let sid = space_id.clone();
    wait_for(&mut h, |app| space_paths(app, &sid).contains(&path("b.log")));

    // Stop watching: new files are not added, the others stay.
    let (sid, gid) = h.view.read_with(&h.cx, |app, _| {
        let space = app.workspace_state.active_project().unwrap().log_space(&space_id).unwrap();
        (space.id.clone(), space.globs[0].glob_id.clone())
    });
    h.view.update(&mut h.cx, |app, cx| app.stop_watching_glob(&sid, &gid, cx));
    std::fs::write(root.path().join("c.log"), "three\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        h.cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    h.view.read_with(&h.cx, |app, _| {
        assert!(!space_paths(app, &space_id).contains(&path("c.log")));
        assert!(app.workspace_state.active_project().unwrap().log_space(&space_id).unwrap().globs[0].stopped);
    });

    // Resume: the folder is listed at once and c.log comes in.
    h.view.update(&mut h.cx, |app, cx| app.resume_watching_glob(&sid, &gid, cx));
    let s = space_id.clone();
    wait_for(&mut h, |app| space_paths(app, &s).contains(&path("c.log")));

    // Removing a file from the glob keeps it out, even as the file is written.
    let file_id = format!("remote:{web1}:{}", path("b.log"));
    h.view.update(&mut h.cx, |app, cx| app.remove_file_from_space(&space_id, &file_id, cx));
    std::fs::write(root.path().join("b.log"), "two\nmore\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        h.cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    h.view.read_with(&h.cx, |app, _| assert!(!space_paths(app, &space_id).contains(&path("b.log"))));

    // Removing the glob removes its files, not the one added by path.
    h.view.update(&mut h.cx, |app, cx| app.remove_live_glob(&sid, &gid, cx));
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(space_paths(app, &space_id), vec![path("keep.txt")]);
    });
}

#[gpui::test]
fn reopening_a_space_matches_its_globs_again_and_a_stopped_one_only_keeps_what_remains(cx: &mut TestAppContext) {
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    let web1 = add_server(&mut h, &ssh, "web1");
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().to_string_lossy().to_string();
    let path = |name: &str| format!("{dir}/{name}");
    std::fs::write(root.path().join("a.log"), "a\n").unwrap();
    std::fs::write(root.path().join("b.log"), "b\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.set_glob_interval(2, cx));
    let globs = vec![
        (web1.clone(), "web1".to_string(), dir.clone(), "*.log".to_string()),
    ];
    let space_id = h
        .view
        .update(&mut h.cx, |app, cx| app.create_remote_log_space("Prod", Vec::new(), globs, Vec::new(), cx))
        .expect("space");
    let s = space_id.clone();
    wait_for(&mut h, |app| space_paths(app, &s).len() == 2);
    let gid = h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().log_space(&space_id).unwrap().globs[0].glob_id.clone()
    });

    // Close the tab, change the folder, reopen: b.log is gone, c.log is new.
    h.view.update(&mut h.cx, |app, cx| app.close_active_tab(cx));
    std::fs::remove_file(root.path().join("b.log")).unwrap();
    std::fs::write(root.path().join("c.log"), "c\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.open_log_space_tab(&space_id, cx));
    let s = space_id.clone();
    wait_for(&mut h, |app| space_paths(app, &s).contains(&path("c.log")));
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(space_paths(app, &space_id), vec![path("a.log"), path("c.log")]);
    });

    // Stopped, closed and reopened: only what it had and still exists, no c.log twice, no d.log.
    h.view.update(&mut h.cx, |app, cx| app.stop_watching_glob(&space_id, &gid, cx));
    h.view.update(&mut h.cx, |app, cx| app.close_active_tab(cx));
    std::fs::remove_file(root.path().join("a.log")).unwrap();
    std::fs::write(root.path().join("d.log"), "d\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.open_log_space_tab(&space_id, cx));
    let s = space_id.clone();
    wait_for(&mut h, |app| !space_paths(app, &s).contains(&path("a.log")));
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        h.cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(space_paths(app, &space_id), vec![path("c.log")]);
        assert!(app.workspace_state.active_project().unwrap().log_space(&space_id).unwrap().globs[0].stopped);
        // No watcher is kept for a stopped glob once it has been matched.
        assert!(app.live.globs.watchers.is_empty());
    });
}
