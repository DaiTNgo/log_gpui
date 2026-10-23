//! Jumping to the next and previous line that carries a Marker.
mod common;

use gpui::TestAppContext;
use log_gpui::keymap::{Command, Keymap, Shortcut};
use log_gpui::marker::TextMarker;
use log_gpui::palette::{HighlightColor, Hue, Shade};

fn marked(cx: &mut TestAppContext, rows: &[usize]) -> common::Harness {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(30));
    h.view.update(&mut h.cx, |app, _| {
        for &row in rows {
            app.all_entries[row]
                .markers
                .push(TextMarker { range: 0..4, color: HighlightColor::palette(Hue::Red, Shade::Medium) });
        }
    });
    h
}

fn selected(h: &common::Harness) -> Option<usize> {
    h.view.read_with(&h.cx, |app, _| app.selected_row_pos)
}

#[gpui::test]
fn next_and_previous_marker_wrap_around(cx: &mut TestAppContext) {
    let mut h = marked(cx, &[3, 20]);
    let mut next = |h: &mut common::Harness| h.view.update(&mut h.cx, |app, cx| app.jump_to_next_marker(cx));
    next(&mut h);
    assert_eq!(selected(&h), Some(3), "with nothing selected, the first Marker");
    next(&mut h);
    assert_eq!(selected(&h), Some(20));
    next(&mut h);
    assert_eq!(selected(&h), Some(3), "wraps");
    h.view.update(&mut h.cx, |app, cx| app.jump_to_prev_marker(cx));
    assert_eq!(selected(&h), Some(20), "wraps back");
}

#[gpui::test]
fn with_no_markers_a_notice_says_so(cx: &mut TestAppContext) {
    let mut h = marked(cx, &[]);
    h.view.update(&mut h.cx, |app, cx| app.jump_to_next_marker(cx));
    assert_eq!(selected(&h), None);
    let notice = h.view.read_with(&h.cx, |app, _| app.notice.clone());
    assert_eq!(notice.as_deref(), Some("No Markers in this view"));
}

#[test]
fn marker_jumps_have_shortcuts() {
    let keymap = Keymap::default();
    assert_eq!(keymap.command_for(&Shortcut::parse("cmd-alt-down").unwrap()), Some(Command::GoNextMarker));
    assert_eq!(keymap.command_for(&Shortcut::parse("cmd-alt-up").unwrap()), Some(Command::GoPrevMarker));
}
