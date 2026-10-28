//! Rows whose line is not read yet (openspec change `line-offset-index`,
//! task 4.2): a frame draws a placeholder instead of reading the file, the
//! lines are read on a worker thread, and the canvas is drawn again.

mod common;

use gpui::TestAppContext;

fn lines(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("2026-09-05 10:{:02}:{:02} [INFO] [api] line {i}", i / 60 % 60, i % 60)).collect()
}

#[gpui::test]
fn rows_not_read_yet_are_read_after_the_frame_and_drawn(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines(2000));
    let file = h.view.read_with(&h.cx, |app, _| app.parsed_files.values().next().unwrap().indexed.clone().unwrap());
    file.forget_cached();
    let before = h.view.read_with(&h.cx, |app, _| app.blocks_read_soon);
    h.cx.update(|window, _| window.refresh());
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert!(app.blocks_read_soon > before, "the rows on screen were asked for"));
    assert!(file.cached_entry(0).is_some(), "the first rows were read");
    assert!(file.cached_entry(1500).is_none(), "rows off screen were not");
    assert!(h.cx.debug_bounds("row-placeholder").is_none(), "drawn again with their lines");
}

#[test]
fn a_cached_row_is_had_without_reading_and_a_missing_one_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.log");
    std::fs::write(&path, lines(600).join("\n") + "\n").unwrap();
    let source = std::sync::Arc::new(log_gpui::index::DiskSource::stay_open(&path, log_gpui::index::HandleBudget::new(2)).unwrap());
    let file = log_gpui::index::file::IndexedFile::index_now(source, Default::default()).unwrap();
    assert!(file.cached_entry(10).is_none());
    assert!(file.entry(10).is_some());
    assert!(file.cached_entry(10).is_some(), "read with its block");
    assert!(file.cached_entry(300).is_none(), "another block");
}
