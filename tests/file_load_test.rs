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
        app.create_log_space("A", LogSpaceKind::Imported, &[source], cx);
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
        assert_eq!(app.live.sources.len(), 1, "and then followed");
    });
}

#[gpui::test]
fn leaving_a_tab_while_it_reads_drops_the_read(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let (a, b) = (log_file(&h, "a.log", 50), log_file(&h, "b.log", 50));
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("A", LogSpaceKind::Imported, &[a], cx));
    h.cx.run_until_parked();
    let a_ids: Vec<String> = h.view.read_with(&h.cx, |app, _| app.parsed_files.keys().cloned().collect());
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("B", LogSpaceKind::Imported, &[b], cx);
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
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("Gone", LogSpaceKind::Imported, &[source], cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(matches!(app.empty_canvas(), Some(EmptyCanvas::LoadFailed { .. })), "{:?}", app.empty_canvas());
        assert!(app.notice.as_deref().is_some_and(|n| n.contains("was not found")), "{:?}", app.notice);
    });
}
