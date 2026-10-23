//! The Raw View follows like the Parsed View: with Follow on, each File
//! Section of a live Log Space shows its last line.
mod common;

use gpui::TestAppContext;
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

fn paint(h: &mut common::Harness) {
    // A resize lays the window out; the editors apply their scroll on the
    // layout after the one that made them.
    for i in 0..3 {
        h.cx.simulate_resize(gpui::size(gpui::px(1200.0 + i as f32), gpui::px(800.0)));
        h.cx.run_until_parked();
    }
}

fn open_raw(cx: &mut TestAppContext, files: &[&str]) -> common::Harness {
    let mut h = common::open_app(cx);
    let sources: Vec<_> = files
        .iter()
        .map(|name| {
            let path = h.dir.path().join(name);
            std::fs::write(&path, common::sample_lines(300).join("\n")).unwrap();
            ImportSource::Disk(path.to_string_lossy().into_owned())
        })
        .collect();
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("Raw", LogSpaceKind::Imported, &sources, cx));
    h.cx.run_until_parked();
    h.view.update(&mut h.cx, |app, cx| app.toggle_view_mode(cx));
    h.cx.run_until_parked();
    paint(&mut h);
    h
}

/// How far each File Section's editor is scrolled down, in pixels.
fn scrolled(h: &common::Harness) -> Vec<f32> {
    h.view.read_with(&h.cx, |app, cx| {
        app.raw_sections()
            .iter()
            .filter_map(|s| app.raw.editors.get(&s.file_id))
            .map(|e| -f32::from(e.state.read(cx).scroll_offset().y))
            .collect()
    })
}

#[gpui::test]
fn a_followed_raw_view_opens_at_the_last_lines(cx: &mut TestAppContext) {
    let h = open_raw(cx, &["one.log"]);
    let offsets = scrolled(&h);
    assert_eq!(offsets.len(), 1);
    assert!(offsets[0] > 1000.0, "scrolled to the end: {offsets:?}");
}

#[gpui::test]
fn every_file_section_follows(cx: &mut TestAppContext) {
    let h = open_raw(cx, &["one.log", "two.log"]);
    let offsets = scrolled(&h);
    assert_eq!(offsets.len(), 2);
    assert!(offsets.iter().all(|y| *y > 1000.0), "{offsets:?}");
}

#[gpui::test]
fn with_follow_off_the_raw_view_stays_at_the_top(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let path = h.dir.path().join("one.log");
    std::fs::write(&path, common::sample_lines(300).join("\n")).unwrap();
    let source = ImportSource::Disk(path.to_string_lossy().into_owned());
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("Raw", LogSpaceKind::Imported, &[source], cx));
    h.cx.run_until_parked();
    h.view.update(&mut h.cx, |app, cx| {
        app.toggle_view_mode(cx);
        // As Open in full does: the view opens with Follow off.
        app.set_follow(false, cx);
    });
    paint(&mut h);
    assert_eq!(scrolled(&h), vec![0.0]);
}

#[gpui::test]
fn scrolling_up_in_the_raw_view_turns_follow_off(cx: &mut TestAppContext) {
    let mut h = open_raw(cx, &["one.log"]);
    assert!(h.view.read_with(&h.cx, |app, _| app.live.follow));
    h.cx.simulate_event(gpui::ScrollWheelEvent {
        position: gpui::point(gpui::px(700.0), gpui::px(400.0)),
        delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(120.0))),
        ..Default::default()
    });
    h.cx.run_until_parked();
    assert!(!h.view.read_with(&h.cx, |app, _| app.live.follow));
}
