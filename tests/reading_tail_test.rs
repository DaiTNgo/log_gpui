//! A Live Log Space opens at its newest line (L3, openspec change
//! `line-offset-index`, task 3.3): while its files are indexed from the
//! start, their last lines show first, with no line numbers yet.

mod common;

use common::{Harness, open_app};
use gpui::TestAppContext;
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

fn text(name: &str, lines: usize) -> String {
    (0..lines).map(|i| format!("2026-09-05 10:{:02}:{:02} [INFO] [API] {name} line {i}\n", i / 60 % 60, i % 60)).collect()
}

fn write_log(h: &Harness, name: &str, text: &str) -> ImportSource {
    let path = h.dir.path().join(name);
    std::fs::write(&path, text).unwrap();
    ImportSource::Disk(path.to_string_lossy().into_owned())
}

#[gpui::test]
fn a_live_space_shows_its_last_lines_while_it_is_indexed(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let source = write_log(&h, "big.log", &text("a", 3000));
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Live", LogSpaceKind::Live, &[source], cx);
        assert!(app.is_reading());
        let tail = app.reading_tail();
        assert!(!tail.is_empty() && tail.len() <= 500);
        assert_eq!(tail.last().unwrap().raw_line.as_ref(), "2026-09-05 10:49:59 [INFO] [API] a line 2999");
        assert!(tail.windows(2).all(|w| w[0].timestamp <= w[1].timestamp));
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.is_reading());
        assert!(app.reading_tail().is_empty(), "the rows show once the file is read");
        assert_eq!(app.view.len(), 3000, "with their line numbers");
        assert!(app.live.follow, "following the newest line");
    });
}

#[gpui::test]
fn the_last_lines_of_several_files_show_by_time(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let a = write_log(&h, "a.log", &text("a", 100));
    let b = write_log(&h, "b.log", &text("b", 50));
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Live", LogSpaceKind::Live, &[a, b], cx);
        let tail = app.reading_tail();
        assert_eq!(tail.len(), 150);
        assert!(tail.windows(2).all(|w| w[0].timestamp <= w[1].timestamp));
        assert_eq!(tail.last().unwrap().source.as_deref(), Some("a.log"));
    });
}

#[gpui::test]
fn a_static_space_shows_no_last_lines(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let source = write_log(&h, "big.log", &text("a", 3000));
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Static", LogSpaceKind::Static, &[source], cx);
        assert!(app.is_reading());
        assert!(app.reading_tail().is_empty());
    });
}
