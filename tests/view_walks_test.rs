//! What must not read every line of a file read through its Line Index
//! (openspec change `line-offset-index`, task 4.3): a refresh of the view
//! finds the longest row from the index, and the Focus Pane lists the rows
//! that pass the Filter without reading them.

mod common;

use gpui::TestAppContext;

fn lines(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let tail = if i == 1234 { " and the longest message of them all" } else { "" };
            format!("2026-09-05 10:{:02}:{:02} [INFO] [api] line {i}{tail}", i / 60 % 60, i % 60)
        })
        .collect()
}

fn indexed(h: &common::Harness) -> std::sync::Arc<log_gpui::index::file::IndexedFile> {
    h.view.read_with(&h.cx, |app, _| app.parsed_files.values().next().unwrap().indexed.clone().unwrap())
}

#[gpui::test]
fn a_refresh_finds_the_longest_row_without_reading_lines(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines(20_000));
    let file = indexed(&h);
    file.forget_cached();
    h.view.update(&mut h.cx, |app, cx| app.apply_filters_and_refresh(cx));
    assert_eq!(file.cached(), 0, "no line was read");
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.longest_row, 1234));
}

#[gpui::test]
fn the_focus_pane_lists_the_passing_rows_without_reading_them(cx: &mut TestAppContext) {
    let h = common::open_app_with_lines(cx, &lines(20_000));
    let file = indexed(&h);
    file.forget_cached();
    h.view.read_with(&h.cx, |app, _| {
        let pass: Vec<usize> = (0..app.view.len()).step_by(3).collect();
        let rows = log_gpui::focus::compute_focus_rows(&app.rows(), &pass, true, None);
        assert_eq!(rows.len(), pass.len());
    });
    assert_eq!(file.cached(), 0);
}

#[test]
fn the_longest_line_comes_from_the_lengths() {
    let mut index = log_gpui::index::LineIndex::new();
    assert_eq!(index.longest(), None);
    for len in [5, 9, 3, 9, 1] {
        index.push(len);
    }
    assert_eq!(index.longest(), Some((1, 9)), "the first of the longest");
}
