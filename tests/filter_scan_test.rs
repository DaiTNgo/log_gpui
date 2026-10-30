//! A Filter condition that needs the text of the rows' values (openspec
//! change `line-offset-index`, task 5.3): a Custom Value on a column with
//! no value ids (too many values, or `message`) is checked by a Scan of
//! the lines read from disk, and the Focus Pane takes the rows as it finds
//! them. The rows that pass are those the Filter gives reading every row.

mod common;

use common::open_app_with_lines;
use gpui::TestAppContext;
use log_gpui::index::BytesSource;
use log_gpui::index::file::IndexedFile;
use std::sync::Arc;

/// Lines, some about a timeout, some followed by a stack line of their own.
fn lines() -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..3000 {
        let what = if i % 7 == 0 { "request timeout" } else { "request done" };
        out.push(format!("2026-09-05 10:00:{:02} [INFO] [API] {what} {i}", i % 60));
        if i % 5 == 0 {
            out.push(format!("    at handler {i}"));
        }
    }
    out
}

/// JSON lines whose `user` has more values than Filter keeps.
fn json_lines() -> Vec<String> {
    (0..2000).map(|i| format!(r#"{{"time":"2026-09-05T10:00:00Z","level":"info","msg":"hi","user":"u{i}"}}"#)).collect()
}

#[gpui::test]
fn a_custom_value_on_the_message_is_found_by_a_scan(cx: &mut TestAppContext) {
    let mut h = open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.filter_state.add_custom("message", "TIMEOUT");
        app.apply_filter_change(cx);
        // Nothing was read on the UI thread: the Scan reads the lines.
        assert!(app.filter_is_scanning());
        assert!(app.filter_pass.is_empty());
        assert!(app.filter_scanning.is_some());
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.filter_is_scanning());
        assert_eq!(app.filter_scanning, None);
        let every_row = app.filter_state.evaluate(&app.filter_index, &app.rows());
        assert_eq!(*app.filter_pass, every_row);
        // Each timeout line, and the stack line under one.
        assert_eq!(every_row.len(), 429 + 86);
    });
}

#[gpui::test]
fn a_custom_value_on_a_column_with_too_many_values_is_found_by_a_scan(cx: &mut TestAppContext) {
    let mut h = open_app_with_lines(cx, &json_lines());
    h.view.update(&mut h.cx, |app, cx| {
        assert!(app.filter_index.column("user").is_some_and(|c| !c.is_filterable()));
        // A Filter saved before the column had too many values.
        app.filter_state.add_custom("user", "u12");
        app.apply_filter_change(cx);
        assert!(app.filter_is_scanning());
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let expected: Vec<usize> = (0..2000).filter(|i| format!("u{i}").contains("u12")).collect();
        assert_eq!(*app.filter_pass, expected);
    });
}

#[gpui::test]
fn a_new_filter_stops_the_scan_of_the_one_before(cx: &mut TestAppContext) {
    let mut h = open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.filter_state.add_custom("message", "timeout");
        app.apply_filter_change(cx);
        app.filter_state.remove_custom("message", "timeout");
        app.filter_state.add_custom("message", "done 29");
        app.apply_filter_change(cx);
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let every_row = app.filter_state.evaluate(&app.filter_index, &app.rows());
        assert_eq!(*app.filter_pass, every_row, "only the rows of the new Filter");
        assert!(!every_row.is_empty());
    });
}

#[gpui::test]
fn a_value_filter_applies_at_once_without_a_scan(cx: &mut TestAppContext) {
    let mut h = open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.filter_toggle_value("module", "api", cx);
        assert!(!app.filter_is_scanning());
        assert_eq!(app.filter_pass.len(), app.view.len());
    });
}

#[test]
fn each_entry_at_gives_a_continuation_line_its_parent() {
    let text: String = lines().iter().map(|l| format!("{l}\n")).collect();
    let file = IndexedFile::index_now(Arc::new(BytesSource::new(text.into_bytes())), Default::default()).unwrap();
    // A stack line, a line far from it, and the last line.
    let at = [1, 2, 900, file.len() - 1];
    let mut seen = Vec::new();
    file.each_entry_at(&at, |line, entry, parent| {
        seen.push((line, entry.raw_line.to_string(), parent.raw_line.to_string()));
    })
    .unwrap();
    let text_of = |n: usize| file.entry(n).unwrap().raw_line.to_string();
    assert_eq!(seen.len(), 4);
    assert_eq!(seen[0], (1, text_of(1), text_of(0)), "the stack line has its Parent Line");
    assert_eq!(seen[1], (2, text_of(2), text_of(2)));
    assert_eq!(seen[2].1, text_of(900));
    assert_eq!(seen[3].1, text_of(file.len() - 1));
}

/// The Focus Pane lists what passes the Filter without reading those lines:
/// with nothing to find, its widest row comes from the Line Index.
#[gpui::test]
fn the_focus_pane_reads_no_line_to_list_what_passes(cx: &mut TestAppContext) {
    let mut h = open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        let file = app.parsed_files.values().next().and_then(|p| p.indexed.clone()).expect("read through its Line Index");
        file.forget_cached();
        app.filter_toggle_value("module", "api", cx);
        assert!(app.focus_rows.len() > 3000);
        assert!(file.cached_entry(2000).is_none(), "no line far down was read");
        let longest = app.rows().get(app.focus_longest).map(|e| e.raw_line.len()).unwrap();
        let widest = (0..app.view.len()).filter_map(|r| app.rows().get(r).map(|e| e.raw_line.len())).max().unwrap();
        assert_eq!(longest, widest);
    });
}
