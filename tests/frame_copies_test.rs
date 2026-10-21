//! What a frame must not copy or walk: the Raw View's File Sections share the
//! file's Breaks, the caret's line comes from the editor's own lookup, and the
//! row of the current line is found by scanning at most once.
mod common;

use common::open_app_with_lines;
use gpui::TestAppContext;
use gpui_component::input::Rope;
use log_gpui::raw_view::{line_and_byte, line_and_byte_in_rope};
use std::sync::Arc;

fn log_lines(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("2026-09-05 10:00:{:02} [INFO] [api] line {i}", i % 60)).collect()
}

#[gpui::test]
fn raw_sections_share_the_files_breaks(cx: &mut TestAppContext) {
    let mut h = open_app_with_lines(cx, &log_lines(20));
    h.view.update(&mut h.cx, |app, _| {
        let sections = app.raw_sections();
        assert_eq!(sections.len(), 1);
        let parsed = &app.parsed_files[&sections[0].file_id];
        assert!(Arc::ptr_eq(&sections[0].breaks, &parsed.raw_breaks), "no copy of the breaks");
        assert!(Arc::ptr_eq(&sections[0].lines, &parsed.lines), "no copy of the lines");
    });
}

#[test]
fn the_caret_line_from_the_rope_equals_the_walk_over_every_line() {
    let lines = ["alpha", "", "bé\u{1F600}ta", "gamma", ""];
    let text = lines.join("\n");
    let rope = Rope::from(text.as_str());
    for cursor in (0..=text.len()).filter(|c| text.is_char_boundary(*c)) {
        assert_eq!(
            line_and_byte_in_rope(&rope, cursor, lines.len()),
            line_and_byte(&lines, cursor),
            "cursor {cursor}"
        );
    }
    // Past the end of the lines: nothing, like the walk.
    assert_eq!(line_and_byte_in_rope(&rope, text.len() + 5, lines.len()), line_and_byte(&lines, text.len() + 5));
}

#[gpui::test]
fn a_stale_selected_position_scans_the_rows_once(cx: &mut TestAppContext) {
    let mut h = open_app_with_lines(cx, &log_lines(60));
    h.view.update(&mut h.cx, |app, _| {
        // Rows moved: neither the selected position nor `line - 1` is the row.
        app.all_entries.reverse();
        let line = app.line_ref_of_entry(10).unwrap();
        app.selected_row_pos = Some(3);
        let before = app.entry_scans.get();
        for _ in 0..5 {
            assert_eq!(app.entry_of_line_ref(&line), Some(10));
        }
        assert_eq!(app.entry_scans.get() - before, 1, "later frames reuse the row found");
        // The remembered row is checked: after the rows moved again it is found anew.
        app.all_entries.reverse();
        let moved = app.entry_of_line_ref(&line).unwrap();
        assert_ne!(moved, 10);
        assert_eq!(app.line_ref_of_entry(moved), Some(line));
    });
}
