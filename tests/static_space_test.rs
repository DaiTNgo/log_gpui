//! A Static Log Space shows its local files as they are on disk (openspec
//! change `static-live-log-spaces`, task 2.2): it has no live source, reads
//! on what is written to a file without moving the view, and reads a file
//! again when another one, or a rewritten one, is at its path.

mod common;

use common::{Harness, open_app};
use gpui::TestAppContext;
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

fn lines(name: &str, from: usize, to: usize, level: &str) -> String {
    (from..to).map(|i| format!("2026-09-05 10:{:02}:{:02} [{level}] [{name}] line {i}\n", i / 60, i % 60)).collect()
}

/// A Static Log Space of `app.log` holding `text`, read.
fn open_static(cx: &mut TestAppContext, text: &str) -> Harness {
    let mut h = open_app(cx);
    let path = h.dir.path().join("app.log");
    std::fs::write(&path, text).unwrap();
    let source = ImportSource::Disk(path.to_string_lossy().into_owned());
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("A", LogSpaceKind::Static, &[source], cx));
    h.cx.run_until_parked();
    h
}

/// Let the app look at the file until `done` holds.
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
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}

fn last_line(app: &log_gpui::app::AppView) -> String {
    app.rows_snapshot().last().map(|e| e.raw_line.to_string()).unwrap_or_default()
}

#[gpui::test]
fn a_static_space_is_not_followed(cx: &mut TestAppContext) {
    let h = open_static(cx, &lines("app", 0, 20, "INFO"));
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.view.len(), 20);
        assert!(app.live.sources.is_empty(), "no live source");
        assert!(!app.active_space_is_live(), "no Live/Pause or Follow");
        assert!(!app.live.follow);
    });
}

#[gpui::test]
fn lines_written_to_a_static_file_are_read_on(cx: &mut TestAppContext) {
    use std::io::Write;
    let mut h = open_static(cx, &lines("app", 0, 20, "INFO"));
    let path = h.dir.path().join("app.log");
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(lines("app", 20, 25, "INFO").as_bytes()).unwrap();
    pump_until(&mut h, "the new lines show", |app| app.view.len() == 25);
    h.view.read_with(&h.cx, |app, _| {
        assert!(last_line(app).ends_with("line 24"));
        assert!(!app.live.follow, "the view does not follow them");
    });
}

#[gpui::test]
fn a_static_file_replaced_with_mv_is_read_again_with_its_filter(cx: &mut TestAppContext) {
    let mut h = open_static(cx, &(lines("app", 0, 60, "INFO") + &lines("app", 60, 100, "ERROR")));
    h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("level", "error", cx));
    h.cx.run_until_parked();
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.filter_pass.len()), 40);

    let other = h.dir.path().join("new.log");
    std::fs::write(&other, lines("new", 0, 30, "INFO") + &lines("new", 30, 40, "ERROR")).unwrap();
    std::fs::rename(&other, h.dir.path().join("app.log")).unwrap();
    pump_until(&mut h, "the other file shows", |app| app.view.len() == 40 && last_line(app).contains("[new]"));
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.rows_snapshot().iter().all(|e| e.raw_line.contains("[new]")), "no line of the old file");
        assert_eq!(app.filter_pass.len(), 10, "the Filter still shows ERROR only");
    });
}

#[gpui::test]
fn a_static_file_rewritten_in_place_and_longer_is_read_again(cx: &mut TestAppContext) {
    let mut h = open_static(cx, &lines("app", 0, 100, "INFO"));
    // `cp bigger.log app.log`: the same inode, other bytes, longer.
    std::fs::write(h.dir.path().join("app.log"), lines("big", 0, 300, "WARN")).unwrap();
    pump_until(&mut h, "the rewritten file shows", |app| app.view.len() == 300);
    h.view.read_with(&h.cx, |app, _| {
        let rows = app.rows_snapshot();
        assert!(rows.iter().all(|e| e.raw_line.contains("[big]")), "no line read at an old offset");
        assert!(rows[0].raw_line.ends_with("line 0") && rows[299].raw_line.ends_with("line 299"));
    });
}

#[gpui::test]
fn a_deleted_static_file_keeps_its_lines_until_it_is_back(cx: &mut TestAppContext) {
    let mut h = open_static(cx, &lines("app", 0, 20, "INFO"));
    let path = h.dir.path().join("app.log");
    std::fs::remove_file(&path).unwrap();
    pump_until(&mut h, "the status bar says so", |app| app.status_bar_labels().iter().any(|l| l == "File deleted"));
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.view.len(), 20, "the lines stay"));

    std::fs::write(&path, lines("back", 0, 5, "INFO")).unwrap();
    pump_until(&mut h, "the file back at the path shows", |app| app.view.len() == 5 && last_line(app).contains("[back]"));
    h.view.read_with(&h.cx, |app, _| assert!(!app.status_bar_labels().iter().any(|l| l.contains("deleted"))));
}

#[gpui::test]
fn a_dormant_static_tab_shows_its_file_as_it_is_now_when_chosen(cx: &mut TestAppContext) {
    let mut h = open_static(cx, &lines("app", 0, 20, "INFO"));
    let tab = h.view.read_with(&h.cx, |app, _| app.workspace_state.active_project().unwrap().working_set.active_tab_index().unwrap());
    let other = h.dir.path().join("other.log");
    std::fs::write(&other, lines("other", 0, 3, "INFO")).unwrap();
    let source = ImportSource::Disk(other.to_string_lossy().into_owned());
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("B", LogSpaceKind::Static, &[source], cx));
    h.cx.run_until_parked();

    // Replaced while its tab is Dormant: nothing looks at it then.
    std::fs::write(h.dir.path().join("app.log"), lines("new", 0, 7, "WARN")).unwrap();
    h.view.update(&mut h.cx, |app, cx| app.switch_to_tab(tab, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.view.len(), 7);
        assert!(last_line(app).contains("[new]"));
    });
}
