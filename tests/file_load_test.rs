//! Local files are read in the background (openspec change
//! `line-offset-index`): the window does not wait for a large file, the
//! canvas and the tab tell how far the read is, and a read the tab no
//! longer needs is dropped.

mod common;

use common::{Harness, open_app};
use gpui::TestAppContext;
use log_gpui::space_view::EmptyCanvas;
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

fn log_file(h: &Harness, name: &str, lines: usize) -> ImportSource {
    let path = h.dir.path().join(name);
    let text: String = (0..lines).map(|i| format!("2026-09-05 10:00:{:02} [INFO] [{name}] line {i}\n", i % 60)).collect();
    std::fs::write(&path, text).unwrap();
    ImportSource::Disk(path.to_string_lossy().into_owned())
}

#[gpui::test]
fn a_file_is_read_in_the_background(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let source = log_file(&h, "a.log", 500);
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("A", LogSpaceKind::Static, &[source], cx);
        // Nothing waited for the file: it is being read.
        assert!(app.is_reading());
        assert!(app.view.is_empty());
        match app.empty_canvas() {
            Some(EmptyCanvas::Reading { files }) => assert_eq!(files[0].0, "a.log"),
            other => panic!("the canvas tells the read: {other:?}"),
        }
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.is_reading());
        assert_eq!(app.view.len(), 500, "the view is built once the file is read");
        assert!(app.empty_canvas().is_none());
        assert!(app.live.sources.is_empty(), "a Static file is not followed");
    });
}

#[gpui::test]
fn leaving_a_tab_while_it_reads_drops_the_read(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let (a, b) = (log_file(&h, "a.log", 50), log_file(&h, "b.log", 50));
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("A", LogSpaceKind::Static, &[a], cx));
    h.cx.run_until_parked();
    let a_ids: Vec<String> = h.view.read_with(&h.cx, |app, _| app.parsed_files.keys().cloned().collect());
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("B", LogSpaceKind::Static, &[b], cx);
        assert!(app.is_reading());
        // Back to the first tab before b.log is read.
        let first = app
            .workspace_state
            .active_project()
            .unwrap()
            .working_set
            .tabs()
            .iter()
            .position(|t| t.target.space_id().is_some_and(|id| id.ends_with("--a")))
            .unwrap();
        app.switch_to_tab(first, cx);
        // a.log is read again (it went when its tab was left), b.log no more.
        let reading: Vec<String> = app.reading().into_iter().map(|(name, _)| name).collect();
        assert_eq!(reading, ["a.log"]);
        assert!(app.loads.keys().all(|id| a_ids.contains(id)), "b.log is no longer read");
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let read: Vec<String> = app.parsed_files.keys().cloned().collect();
        assert_eq!(read, a_ids, "only the chosen tab's file is held");
        assert_eq!(app.view.len(), 50);
    });
}

#[gpui::test]
fn a_file_that_cannot_be_read_says_why(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let source = log_file(&h, "gone.log", 3);
    std::fs::remove_file(h.dir.path().join("gone.log")).unwrap();
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("Gone", LogSpaceKind::Static, &[source], cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(matches!(app.empty_canvas(), Some(EmptyCanvas::LoadFailed { .. })), "{:?}", app.empty_canvas());
        assert!(app.notice.as_deref().is_some_and(|n| n.contains("was not found")), "{:?}", app.notice);
    });
}

/// Pump the live sources until `done` holds (the source reads on its own
/// thread, every few tens of milliseconds).
fn pump_until(h: &mut Harness, what: &str, done: impl Fn(&log_gpui::app::AppView) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        h.view.update(&mut h.cx, |app, cx| {
            app.pump_live(cx);
        });
        h.cx.run_until_parked();
        if h.view.read_with(&h.cx, |app, _| done(app)) {
            return;
        }
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn append(h: &Harness, name: &str, text: &str) {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().append(true).open(h.dir.path().join(name)).unwrap();
    f.write_all(text.as_bytes()).unwrap();
}

#[gpui::test]
fn a_local_file_of_a_live_space_is_followed_through_its_line_index(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let source = log_file(&h, "a.log", 100);
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("A", LogSpaceKind::Live, &[source], cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let parsed = app.parsed_files.values().next().unwrap();
        assert!(parsed.indexed.is_some() && parsed.entries.is_empty(), "read through its Line Index, not held");
    });
    pump_until(&mut h, "the source is live", |app| {
        app.live_file_statuses().iter().all(|(_, s)| *s == log_gpui::live::SourceStatus::Live)
    });

    append(&h, "a.log", "2026-09-05 11:00:00 [ERROR] [a.log] new one\n2026-09-05 11:00:01 [INFO] [a.log] new two\n");
    pump_until(&mut h, "the new lines show", |app| app.view.len() == 102);
    h.view.read_with(&h.cx, |app, _| {
        let rows = app.rows_snapshot();
        assert!(rows[101].raw_line.ends_with("new two"));
        assert_eq!(rows[101].index, 102);
    });

    // Paused, the view stays as it is; Live again, what was written comes.
    h.view.update(&mut h.cx, |app, cx| app.toggle_pause(cx));
    append(&h, "a.log", "2026-09-05 11:00:02 [INFO] [a.log] while paused\n");
    for _ in 0..10 {
        h.view.update(&mut h.cx, |app, cx| {
            app.pump_live(cx);
        });
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.view.len()), 102, "nothing while paused");
    h.view.update(&mut h.cx, |app, cx| app.toggle_pause(cx));
    pump_until(&mut h, "the paused line comes", |app| app.view.len() == 103);

    // A line written in two goes: shown once whole.
    append(&h, "a.log", "2026-09-05 11:00:03 [WARN] [a.log] half");
    append(&h, "a.log", " and the rest\n");
    pump_until(&mut h, "the whole line shows", |app| {
        app.view.len() == 104 && app.rows_snapshot()[103].raw_line.ends_with("half and the rest")
    });
}

#[gpui::test]
fn a_rotated_local_file_of_a_live_space_is_read_again(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let source = log_file(&h, "a.log", 20);
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("A", LogSpaceKind::Live, &[source], cx));
    h.cx.run_until_parked();
    pump_until(&mut h, "the source is live", |app| {
        app.live_file_statuses().iter().all(|(_, s)| *s == log_gpui::live::SourceStatus::Live)
    });
    let path = h.dir.path().join("a.log");
    std::fs::rename(&path, h.dir.path().join("a.log.1")).unwrap();
    std::fs::write(&path, "2026-09-05 12:00:00 [INFO] [a.log] after the rotation\n").unwrap();
    pump_until(&mut h, "the new file shows", |app| {
        app.rows_snapshot().last().is_some_and(|e| e.raw_line.ends_with("after the rotation"))
    });
}

#[gpui::test]
fn the_local_files_of_a_live_space_share_one_timeline_and_pause_together(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let (a, b) = (log_file(&h, "a.log", 3), log_file(&h, "b.log", 3));
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("Both", LogSpaceKind::Live, &[a, b], cx));
    h.cx.run_until_parked();
    pump_until(&mut h, "both sources are live", |app| {
        let statuses = app.live_file_statuses();
        statuses.len() == 2 && statuses.iter().all(|(_, s)| *s == log_gpui::live::SourceStatus::Live)
    });

    append(&h, "b.log", "2026-09-05 11:00:02 [INFO] [b.log] b later\n");
    append(&h, "a.log", "2026-09-05 11:00:01 [INFO] [a.log] a sooner\n");
    pump_until(&mut h, "both new lines show", |app| app.view.len() == 8);
    h.view.read_with(&h.cx, |app, _| {
        let rows = app.rows_snapshot();
        let last: Vec<&str> = rows[6..].iter().map(|e| e.raw_line.rsplit("] ").next().unwrap()).collect();
        assert_eq!(last, ["a sooner", "b later"], "in time order, across the files");
    });

    // Pause holds both files; Live brings what both wrote.
    h.view.update(&mut h.cx, |app, cx| app.toggle_pause(cx));
    append(&h, "a.log", "2026-09-05 11:00:03 [INFO] [a.log] a paused\n");
    append(&h, "b.log", "2026-09-05 11:00:04 [INFO] [b.log] b paused\n");
    for _ in 0..10 {
        h.view.update(&mut h.cx, |app, cx| {
            app.pump_live(cx);
        });
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.view.len()), 8, "nothing while paused");
    h.view.update(&mut h.cx, |app, cx| app.toggle_pause(cx));
    pump_until(&mut h, "the paused lines of both come", |app| app.view.len() == 10);
}
