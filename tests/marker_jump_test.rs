//! Jumping to the next and previous line that carries a Marker.
mod common;

use gpui::TestAppContext;
use log_gpui::keymap::{Command, Keymap, Shortcut};
use log_gpui::palette::{HighlightColor, Hue, Shade};

fn marked(cx: &mut TestAppContext, rows: &[usize]) -> common::Harness {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(30));
    h.view.update(&mut h.cx, |app, _| {
        // Painted on the file, as marking a line does; the view looks them up.
        let lines = app.rows_snapshot();
        let project = app.workspace_state.active_project_mut().unwrap();
        let file = project.log_spaces.last_mut().unwrap().files.first_mut().unwrap();
        for &row in rows {
            let color = HighlightColor::palette(Hue::Red, Shade::Medium);
            assert!(file.paint_marker(lines[row].index, &lines[row].raw_line, 0..4, color));
        }
        app.reapply_markers();
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

/// The Markers are looked up by line number, from their table (openspec
/// change `line-offset-index`, task 7.3): no line is read to find them.
#[gpui::test]
fn jumping_to_a_marker_reads_no_line(cx: &mut TestAppContext) {
    let mut h = marked(cx, &[3, 20]);
    h.view.update(&mut h.cx, |app, cx| {
        let file_id = app.view.files[0].file_id.to_string();
        let indexed = app.parsed_files[&file_id].indexed.clone().expect("read through its Line Index");
        indexed.forget_cached();
        assert_eq!(app.rows().marked_rows(), [3, 20]);
        app.jump_to_next_marker(cx);
        assert_eq!(app.selected_row_pos, Some(3));
        assert!(app.rows().markers(20).len() == 1);
        assert!(indexed.cached_entry(20).is_none(), "line 21 was not read to find its Marker");
    });
}
