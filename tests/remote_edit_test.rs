//! Editing a Remote File, a Live Glob and a Remote Command in the real
//! `AppView`, against a local sshd (see `remote_hub_test.rs`):
//! `SSH_TEST_DIR`, `SSH_TEST_PORT`. Skipped when unset.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::AppView;
use log_gpui::app::remote_picker::{EditMember, PickerTarget, Staged};
use log_gpui::remote::{AuthMethod, PresetFields, Server};
use log_gpui::workspace::log_space::{LogSpace, RemoteTarget};
use std::time::{Duration, Instant};

struct Ssh {
    dir: String,
    key: String,
    port: u16,
    user: String,
}

fn ssh() -> Option<Ssh> {
    let dir = std::env::var("SSH_TEST_DIR").ok()?;
    Some(Ssh {
        key: format!("{dir}/userkey"),
        dir,
        port: std::env::var("SSH_TEST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(2222),
        user: std::env::var("USER").ok()?,
    })
}

fn add_server(h: &mut Harness, ssh: &Ssh, name: &str) -> String {
    let mut server = Server::new(name, "127.0.0.1", &ssh.user, AuthMethod::Key { path: ssh.key.clone() });
    server.port = ssh.port;
    h.view.update(&mut h.cx, |app, _| {
        let id = app.remote.servers.save(server).unwrap();
        *app.remote.shared_servers.lock().unwrap() = app.remote.servers.clone();
        id
    })
}

fn wait_for(h: &mut Harness, mut done: impl FnMut(&AppView) -> bool) {
    h.cx.executor().allow_parking();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        h.cx.run_until_parked();
        let finished = h.view.update(&mut h.cx, |app, cx| {
            if let Some(request) = app.remote.host_prompts.pop_front() {
                let _ = request.reply.send(true);
            }
            // The 100 ms pump runs on a timer the test clock does not advance.
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

/// Let background work run for a while.
fn settle(h: &mut Harness, secs: u64) {
    h.cx.executor().allow_parking();
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        h.cx.run_until_parked();
        h.view.update(&mut h.cx, |app, cx| {
            if let Some(request) = app.remote.host_prompts.pop_front() {
                let _ = request.reply.send(true);
            }
            app.pump_live(cx);
        });
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn space<'a>(app: &'a AppView, id: &str) -> &'a LogSpace {
    app.workspace_state.active_project().unwrap().log_space(id).unwrap()
}

fn lines(app: &AppView, file_id: &str) -> Vec<String> {
    app.parsed_files.get(file_id).map(|p| p.lines.iter().map(|l| l.to_string()).collect()).unwrap_or_default()
}

fn names(app: &AppView, id: &str) -> Vec<String> {
    space(app, id).ordered_files().iter().map(|f| f.display_name.clone()).collect()
}

fn picker_idle(app: &AppView) -> bool {
    app.remote.picker.as_ref().is_some_and(|p| p.busy.is_none())
}

fn red() -> log_gpui::palette::HighlightColor {
    log_gpui::palette::HighlightColor::palette(log_gpui::palette::Hue::Red, log_gpui::palette::Shade::Medium)
}

fn new_space(h: &mut Harness, files: Vec<(String, String, String)>, globs: Vec<(String, String, String, String)>, commands: Vec<(String, String, String, Option<PresetFields>)>) -> String {
    h.view
        .update(&mut h.cx, |app, cx| app.create_remote_log_space("Checkout", files, globs, commands, cx))
        .expect("space")
}

/// Type `text` in the open Edit dialog and wait until it was checked.
fn type_path(h: &mut Harness, text: &str) {
    let text = text.to_string();
    h.view.update(&mut h.cx, |app, cx| app.picker_add_text(text, cx));
    wait_for(h, |app| picker_idle(app));
}

#[gpui::test]
fn editing_a_mistyped_path_keeps_the_place_and_the_file_view_and_reads_the_new_file(cx: &mut TestAppContext) {
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    let web1 = add_server(&mut h, &ssh, "web1");
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().to_string_lossy().to_string();
    for name in ["first.log", "third.log"] {
        std::fs::write(root.path().join(name), format!("{name} line\n")).unwrap();
    }
    std::fs::write(root.path().join("app.log"), "hello from app\n").unwrap();
    let p = |name: &str| format!("{dir}/{name}");
    let files = ["first.log", "ap.log", "third.log"].map(|n| (web1.clone(), "web1".to_string(), p(n))).to_vec();
    let space_id = new_space(&mut h, files, Vec::new(), Vec::new());
    let file_id = format!("remote:{web1}:{}", p("ap.log"));

    // ap.log does not exist: the file waits. Its File View is open.
    h.view.update(&mut h.cx, |app, cx| app.open_file_view_tab(&space_id, &format!("remote:{web1}:{}", p("ap.log")), cx));
    let id = file_id.clone();
    wait_for(&mut h, |app| app.live.sources.get(&id).is_some_and(|s| s.status == log_gpui::live::SourceStatus::WaitingForFile));
    // A Marker on it must not survive: it is another file now.
    h.view.update(&mut h.cx, |app, _| {
        let project = app.workspace_state.active_project_mut().unwrap();
        project.log_space_mut(&space_id).unwrap().file_mut(&file_id).unwrap().paint_marker(1, "x", 0..1, red());
    });

    // Edit… opens the dialog with the file, alone, Save off.
    let (sid, fid) = (space_id.clone(), file_id.clone());
    h.view.update(&mut h.cx, |app, cx| app.open_remote_edit(&sid, EditMember::File(fid), cx));
    wait_for(&mut h, picker_idle);
    h.view.read_with(&h.cx, |app, _| {
        let picker = app.remote.picker.as_ref().unwrap();
        assert_eq!(picker.staged, vec![Staged::File { server_id: web1.clone(), path: p("ap.log") }]);
        assert!(app.picker_is_edit());
        assert!(!app.picker_edit_can_save(), "nothing changed yet");
        assert_eq!(app.picker_edit_warning(), None, "the path is the same");
    });
    type_path(&mut h, &p("app.log"));
    h.view.read_with(&h.cx, |app, _| {
        let picker = app.remote.picker.as_ref().unwrap();
        assert_eq!(picker.staged.len(), 1, "an Edit holds one file");
        assert!(app.picker_edit_can_save());
        assert_eq!(app.picker_edit_warning(), Some("Markers on this file will be removed"));
    });
    h.view.update(&mut h.cx, |app, cx| app.picker_save_edit(cx));
    h.view.read_with(&h.cx, |app, _| assert!(app.remote.picker.is_none(), "Save closes the dialog"));

    // The new file took the second place; the File View went with it.
    let id = file_id.clone();
    wait_for(&mut h, |app| lines(app, &id).iter().any(|l| l == "hello from app"));
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(names(app, &space_id), [format!("web1:{}", p("first.log")), format!("web1:{}", p("app.log")), format!("web1:{}", p("third.log"))]);
        let s = space(app, &space_id);
        assert!(s.file(&file_id).unwrap().markers.is_empty());
        assert!(s.file(&file_id).is_some(), "same member, same id");
        let tabs = app.workspace_state.active_project().unwrap().working_set.tabs();
        assert!(tabs.iter().any(|t| matches!(&t.target, log_gpui::workspace::working_set::TabTarget::FileView { file_id: f, .. } if *f == file_id)));
        // Recent Paths of the Server have the new path.
        assert!(app.remote.picker_state.recent(&web1).iter().any(|r| r.path == p("app.log")));
    });
    // And it is saved: another session reads the new path.
    h.view.read_with(&h.cx, |app, _| {
        let json = serde_json::to_value(space(app, &space_id)).unwrap();
        assert_eq!(json["files"][1]["remote"]["target"]["File"]["path"], p("app.log"));
    });
}

#[gpui::test]
fn the_starting_time_reads_the_same_file_again_and_keeps_its_markers(cx: &mut TestAppContext) {
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    let web1 = add_server(&mut h, &ssh, "web1");
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().to_string_lossy().to_string();
    let log = format!("{dir}/app.log");
    let text: String = (0..60).map(|m| format!("2026-10-19 10:{m:02}:00 INFO line {m}\n")).collect();
    std::fs::write(&log, text).unwrap();
    let space_id = new_space(&mut h, vec![(web1.clone(), "web1".to_string(), log.clone())], Vec::new(), Vec::new());
    let file_id = format!("remote:{web1}:{log}");
    // Starts from 10:40.
    h.view.update(&mut h.cx, |app, cx| {
        let (s, f, l) = (space_id.clone(), file_id.clone(), log.clone());
        app.apply_remote_file_edit(&s, &f, &web1, &l, Some("2026-10-19 10:40".into()), cx).unwrap();
    });
    let id = file_id.clone();
    wait_for(&mut h, |app| lines(app, &id).len() >= 20);
    let before = h.view.read_with(&h.cx, |app, _| lines(app, &file_id).len());
    assert!(before < 40, "{before}");
    h.view.update(&mut h.cx, |app, _| {
        let project = app.workspace_state.active_project_mut().unwrap();
        let file = project.log_space_mut(&space_id).unwrap().file_mut(&file_id).unwrap();
        assert!(file.paint_marker(55, "2026-10-19 10:54:00 INFO line 54", 0..4, red()));
    });

    // Back to 10:20: the same file, read again with more lines; the Marker stays.
    let (sid, fid) = (space_id.clone(), file_id.clone());
    h.view.update(&mut h.cx, |app, cx| app.open_remote_edit(&sid, EditMember::File(fid), cx));
    wait_for(&mut h, picker_idle);
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.remote.picker.as_ref().unwrap().start_time, "2026-10-19 10:40");
    });
    h.view.update(&mut h.cx, |app, _| app.remote.picker.as_mut().unwrap().start_time = "2026-10-19 10:20".into());
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.picker_edit_can_save());
        assert_eq!(app.picker_edit_warning(), None, "the same file keeps its Markers");
    });
    h.view.update(&mut h.cx, |app, cx| app.picker_save_edit(cx));
    let id = file_id.clone();
    wait_for(&mut h, |app| lines(app, &id).len() > before);
    h.view.read_with(&h.cx, |app, _| {
        let file = space(app, &space_id).file(&file_id).unwrap();
        assert_eq!(file.remote.as_ref().unwrap().start_time.as_deref(), Some("2026-10-19 10:20"));
        assert_eq!(file.markers.len(), 1, "the Marker is kept");
        assert_eq!(file.remote.as_ref().unwrap().path(), Some(log.as_str()));
    });
}

#[gpui::test]
fn narrowing_a_live_glob_previews_removes_the_rest_and_does_not_read_the_kept_files_again(cx: &mut TestAppContext) {
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    let web1 = add_server(&mut h, &ssh, "web1");
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().to_string_lossy().to_string();
    for name in ["app-1.log", "app-2.log", "worker.log"] {
        std::fs::write(root.path().join(name), format!("{name}\n")).unwrap();
    }
    let p = |name: &str| format!("{dir}/{name}");
    h.view.update(&mut h.cx, |app, cx| app.set_glob_interval(2, cx));
    let globs = vec![(web1.clone(), "web1".to_string(), dir.clone(), "*.log".to_string())];
    let space_id = new_space(&mut h, Vec::new(), globs, Vec::new());
    let sid = space_id.clone();
    wait_for(&mut h, |app| space(app, &sid).files.len() == 3);
    let glob_id = h.view.read_with(&h.cx, |app, _| space(app, &space_id).globs[0].glob_id.clone());
    let kept_id = format!("remote:{web1}:{}", p("app-1.log"));
    let worker_id = format!("remote:{web1}:{}", p("worker.log"));
    let kept = kept_id.clone();
    wait_for(&mut h, |app| app.live.sources.contains_key(&kept) && lines(app, &kept).len() == 1);
    let channel = h.view.read_with(&h.cx, |app, _| app.live.sources[&kept_id].handle.commands.clone());
    h.view.update(&mut h.cx, |app, _| {
        let project = app.workspace_state.active_project_mut().unwrap();
        project.log_space_mut(&space_id).unwrap().file_mut(&kept_id).unwrap().paint_marker(1, "app-1.log", 0..3, red());
    });
    // A File View of the file that will go.
    h.view.update(&mut h.cx, |app, cx| app.open_file_view_tab(&space_id, &worker_id, cx));

    let (s, g) = (space_id.clone(), glob_id.clone());
    h.view.update(&mut h.cx, |app, cx| app.open_remote_edit(&s, EditMember::Glob(g), cx));
    wait_for(&mut h, picker_idle);
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.picker_edit_can_save());
        assert_eq!(app.remote.picker.as_ref().unwrap().staged, vec![Staged::Glob { server_id: web1.clone(), dir: dir.clone(), pattern: "*.log".into() }]);
    });
    type_path(&mut h, &p("app-*.log"));
    wait_for(&mut h, |app| app.remote.picker.as_ref().is_some_and(|p| p.glob_preview.as_ref().is_some_and(|v| v.plan.is_some())));
    h.view.read_with(&h.cx, |app, _| {
        let preview = app.remote.picker.as_ref().unwrap().glob_preview.clone().unwrap();
        assert_eq!(preview.summary().unwrap(), "+0 files \u{b7} \u{2212}1 file \u{b7} 2 kept");
        assert!(app.picker_edit_can_save());
    });
    h.view.update(&mut h.cx, |app, cx| app.picker_save_edit(cx));
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.remote.picker.is_none());
        let s = space(app, &space_id);
        assert_eq!(s.globs[0].pattern, "app-*.log");
        assert_eq!(s.globs[0].glob_id, glob_id);
        assert!(s.file(&worker_id).is_none(), "worker.log is removed");
        assert!(s.globs[0].excluded.is_empty(), "removed by the edit, not by the user");
        assert_eq!(s.glob_files(&glob_id).len(), 2);
        assert_eq!(s.file(&kept_id).unwrap().markers.len(), 1);
        let tabs = app.workspace_state.active_project().unwrap().working_set.tabs();
        assert!(!tabs.iter().any(|t| matches!(&t.target, log_gpui::workspace::working_set::TabTarget::FileView { file_id: f, .. } if *f == worker_id)));
        // The kept file goes on reading with the same source.
        assert!(app.live.sources[&kept_id].handle.commands.same_channel(&channel));
    });
    // The watcher now matches the new pattern: worker.log does not come back.
    settle(&mut h, 5);
    h.view.read_with(&h.cx, |app, _| assert!(space(app, &space_id).file(&worker_id).is_none()));
}

#[gpui::test]
fn editing_a_remote_command_keeps_lines_and_markers_breaks_and_starts_a_new_run(cx: &mut TestAppContext) {
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    let web1 = add_server(&mut h, &ssh, "web1");
    let fake = |name: &str| format!("{}/fake/docker-{name}.log", ssh.dir);
    std::fs::write(fake("api"), "2026-10-19T03:02:03.000000001Z api one\n2026-10-19T03:02:04.000000001Z api two\n").unwrap();
    std::fs::write(fake("worker"), "2026-10-19T03:02:05.000000001Z worker one\n").unwrap();
    let docker = |name: &str| PresetFields::docker(name);
    let command = |name: &str| docker(name).command();
    let space_id = new_space(&mut h, Vec::new(), Vec::new(), vec![(web1.clone(), "web1".into(), command("api"), Some(docker("api")))]);
    let file_id = h.view.read_with(&h.cx, |app, _| space(app, &space_id).files[0].file_id.clone());
    let id = file_id.clone();
    wait_for(&mut h, |app| lines(app, &id).len() == 2);
    h.view.update(&mut h.cx, |app, _| {
        let project = app.workspace_state.active_project_mut().unwrap();
        let file = project.log_space_mut(&space_id).unwrap().file_mut(&file_id).unwrap();
        // The Marker is on a line of the first Run.
        assert!(file.paint_marker(1, "2026-10-19T03:02:03.000000001Z api one", 0..4, red()));
    });
    h.view.update(&mut h.cx, |app, cx| app.set_restart_on_exit(&space_id.clone(), &file_id.clone(), true, cx));

    let (s, f) = (space_id.clone(), file_id.clone());
    h.view.update(&mut h.cx, |app, cx| app.open_remote_edit(&s, EditMember::Command(f), cx));
    h.cx.simulate_resize(gpui::size(gpui::px(1200.0), gpui::px(900.0)));
    h.cx.run_until_parked();
    // The dialog opens with the command and its preset filled in.
    h.view.read_with(&h.cx, |app, cx| {
        assert!(app.picker_is_edit());
        assert_eq!(app.command_text(cx), command("api"));
        assert_eq!(app.command_preset_fields(cx), Some(docker("api")));
        assert!(!app.command_edit_changed(cx), "Save is off until something changes");
    });
    // Change the container field: the command follows, Save turns on.
    h.cx.update(|window, cx| {
        let view = h.view.clone();
        let subject = view.read(cx).remote.picker.as_ref().unwrap().command.inputs.as_ref().unwrap().subject.clone();
        subject.update(cx, |state, cx| state.set_value("worker", window, cx));
    });
    // (A value set from code raises no change event, so say the fields changed.)
    h.view.update(&mut h.cx, |app, cx| {
        app.command_select_preset(Some(log_gpui::app::remote_command::PresetKind::Docker), cx)
    });
    for i in 0..4 {
        h.cx.run_until_parked();
        h.cx.simulate_resize(gpui::size(gpui::px(1200.0 + i as f32), gpui::px(900.0)));
    }
    h.view.read_with(&h.cx, |app, cx| {
        assert_eq!(app.command_text(cx), command("worker"));
        assert!(app.command_edit_changed(cx));
    });
    h.view.update(&mut h.cx, |app, cx| app.command_save_edit(cx));
    h.view.read_with(&h.cx, |app, _| assert!(app.remote.picker.is_none()));

    // The old lines stay, a Break names both commands, the new Run starts at 1.
    let id = file_id.clone();
    wait_for(&mut h, |app| lines(app, &id).len() == 3);
    h.view.read_with(&h.cx, |app, _| {
        let parsed = &app.parsed_files[&file_id];
        let texts: Vec<&str> = parsed.lines.iter().map(|l| l.as_ref()).collect();
        assert!(texts[0].contains("api one") && texts[1].contains("api two") && texts[2].contains("worker one"), "{texts:?}");
        let label = &parsed.raw_breaks.values().next().expect("a Break").label;
        assert!(label.contains("command changed"), "{label}");
        assert!(label.contains("'api'") && label.contains("'worker'"), "{label}");
        assert_eq!(parsed.entries[2].index, 1, "a new Run counts from line 1");
        assert!(parsed.entries[2].generation > parsed.entries[1].generation);
        let file = space(app, &space_id).file(&file_id).unwrap();
        assert_eq!(file.display_name, format!("web1$ {}", command("worker")));
        assert_eq!(file.markers.len(), 1, "the Marker on the old line stays");
        match &file.remote.as_ref().unwrap().target {
            RemoteTarget::Command { command: c, preset, restart_on_exit } => {
                assert_eq!(*c, command("worker"));
                assert_eq!(preset.as_ref(), Some(&docker("worker")));
                assert!(*restart_on_exit, "Restart on exit keeps its value");
            }
            other => panic!("{other:?}"),
        }
    });
}

#[gpui::test]
fn a_value_that_cannot_be_saved_or_a_cancel_changes_nothing(cx: &mut TestAppContext) {
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    let web1 = add_server(&mut h, &ssh, "web1");
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().to_string_lossy().to_string();
    for name in ["a.log", "b.log"] {
        std::fs::write(root.path().join(name), "x\n").unwrap();
    }
    let p = |name: &str| format!("{dir}/{name}");
    let files = ["a.log", "b.log"].map(|n| (web1.clone(), "web1".to_string(), p(n))).to_vec();
    let space_id = new_space(&mut h, files, Vec::new(), Vec::new());
    let a_id = format!("remote:{web1}:{}", p("a.log"));
    let before = h.view.read_with(&h.cx, |app, _| space(app, &space_id).files.clone());

    // A folder that does not exist: the dialog says so, stays open, nothing changes.
    let (s, f) = (space_id.clone(), a_id.clone());
    h.view.update(&mut h.cx, |app, cx| app.open_remote_edit(&s, EditMember::File(f), cx));
    wait_for(&mut h, picker_idle);
    type_path(&mut h, "/no/such/dir/a.log");
    h.view.read_with(&h.cx, |app, _| {
        let picker = app.remote.picker.as_ref().unwrap();
        assert!(picker.error.as_deref().is_some_and(|e| e.contains("does not exist")), "{:?}", picker.error);
        assert_eq!(picker.staged, vec![Staged::File { server_id: web1.clone(), path: p("a.log") }], "still the old value");
        assert!(!app.picker_edit_can_save());
        assert_eq!(space(app, &space_id).files, before);
    });

    // Another member's path: Save is on, but the dialog refuses and stays.
    type_path(&mut h, &p("b.log"));
    h.view.read_with(&h.cx, |app, _| assert!(app.picker_edit_can_save()));
    h.view.update(&mut h.cx, |app, cx| app.picker_save_edit(cx));
    h.view.read_with(&h.cx, |app, _| {
        let picker = app.remote.picker.as_ref().expect("the dialog stays open");
        assert!(picker.error.as_deref().is_some_and(|e| e.contains("already in this Log Space")), "{:?}", picker.error);
        assert_eq!(space(app, &space_id).files, before);
    });

    // Cancel: nothing changes; a late answer to a check is dropped.
    h.view.update(&mut h.cx, |app, cx| {
        app.picker_add_text(p("a2.log"), cx);
        app.close_remote_picker(cx);
    });
    settle(&mut h, 1);
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.remote.picker.is_none());
        assert_eq!(space(app, &space_id).files, before);
    });
    // The same dialog opened again does not receive the answer of the first.
    let (s, f) = (space_id.clone(), a_id.clone());
    h.view.update(&mut h.cx, |app, cx| {
        app.picker_add_text("/stale/answer.log".into(), cx);
        app.close_remote_picker(cx);
        app.open_remote_edit(&s, EditMember::File(f), cx);
    });
    settle(&mut h, 1);
    h.view.read_with(&h.cx, |app, _| {
        let picker = app.remote.picker.as_ref().unwrap();
        assert_eq!(picker.staged, vec![Staged::File { server_id: web1.clone(), path: p("a.log") }]);
    });
    let _ = PickerTarget::NewSpace;
}

#[gpui::test]
fn moving_a_file_to_another_server_closes_the_connection_nothing_uses_any_more(cx: &mut TestAppContext) {
    use log_gpui::remote::ServerState;
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    let web1 = add_server(&mut h, &ssh, "web1");
    let web2 = add_server(&mut h, &ssh, "web2");
    let root = tempfile::tempdir().unwrap();
    let log = format!("{}/app.log", root.path().display());
    std::fs::write(&log, "one line\n").unwrap();
    let space_id = new_space(&mut h, vec![(web1.clone(), "web1".to_string(), log.clone())], Vec::new(), Vec::new());
    let file_id = format!("remote:{web1}:{log}");
    let id = file_id.clone();
    wait_for(&mut h, |app| lines(app, &id).len() == 1);
    let hub = h.view.read_with(&h.cx, |app, _| app.remote.hub.clone().expect("a hub"));
    assert_eq!(hub.health().get(&web1).state, ServerState::Connected);

    let (s, f, l, w2) = (space_id.clone(), file_id.clone(), log.clone(), web2.clone());
    h.view.update(&mut h.cx, |app, cx| {
        app.apply_remote_file_edit(&s, &f, &w2, &l, None, cx).unwrap();
    });
    let id = file_id.clone();
    wait_for(&mut h, |app| lines(app, &id).len() == 1);
    // web1 has nothing in this Log Space any more: its connection was closed.
    let deadline = Instant::now() + Duration::from_secs(10);
    while hub.health().get(&web1).state == ServerState::Connected {
        assert!(Instant::now() < deadline, "the connection to web1 stayed open");
        settle(&mut h, 1);
    }
    h.view.read_with(&h.cx, |app, _| {
        let file = space(app, &space_id).file(&file_id).unwrap();
        assert_eq!(file.display_name, format!("web2:{log}"));
        assert_eq!(file.remote.as_ref().unwrap().server_id, web2);
    });
    assert_eq!(hub.health().get(&web2).state, ServerState::Connected);
}

#[gpui::test]
fn a_late_event_of_a_closed_source_does_not_reach_the_view(cx: &mut TestAppContext) {
    let Some(ssh) = ssh() else { return };
    let mut h = common::open_app(cx);
    let web1 = add_server(&mut h, &ssh, "web1");
    let root = tempfile::tempdir().unwrap();
    let (old, new) = (format!("{}/old.log", root.path().display()), format!("{}/new.log", root.path().display()));
    std::fs::write(&old, "old line\n").unwrap();
    std::fs::write(&new, "new line\n").unwrap();
    let space_id = new_space(&mut h, vec![(web1.clone(), "web1".to_string(), old.clone())], Vec::new(), Vec::new());
    let file_id = format!("remote:{web1}:{old}");
    let id = file_id.clone();
    wait_for(&mut h, |app| lines(app, &id).len() == 1);
    // The old source is still sending when the file is edited.
    let stale = h.view.read_with(&h.cx, |app, _| app.live.sources[&file_id].handle.clone());
    h.view.update(&mut h.cx, |app, cx| {
        app.apply_remote_file_edit(&space_id, &file_id, &web1, &new, None, cx).unwrap();
    });
    let seq_after = h.view.read_with(&h.cx, |app, _| app.live.sources[&file_id].seq);
    stale.send(log_gpui::live::SourceCommand::Reconnect);
    let id = file_id.clone();
    wait_for(&mut h, |app| lines(app, &id) == ["new line"]);
    settle(&mut h, 2);
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(lines(app, &file_id), ["new line"], "nothing of the old file");
        assert_eq!(app.live.sources[&file_id].seq, seq_after);
    });
}
