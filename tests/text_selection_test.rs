use log_gpui::text_selection::{TextPos, TextSelection, byte_at_x, clip_to_span};

fn sel(a: (usize, usize), b: (usize, usize)) -> TextSelection {
    TextSelection::new(TextPos::new(a.0, a.1), TextPos::new(b.0, b.1))
}

#[test]
fn first_middle_and_last_rows_of_a_drag() {
    // From byte 5 of row 3 to byte 4 of row 5.
    let s = sel((3, 5), (5, 4));
    assert_eq!(s.range_in_row(2, 20), None);
    assert_eq!(s.range_in_row(3, 20), Some(5..20), "first row: anchor to line end");
    assert_eq!(s.range_in_row(4, 30), Some(0..30), "middle row: whole line");
    assert_eq!(s.range_in_row(5, 40), Some(0..4), "last row: line start to head");
    assert_eq!(s.range_in_row(6, 20), None);
}

#[test]
fn a_backward_drag_selects_the_same_text() {
    let forward = sel((3, 5), (5, 4));
    let backward = sel((5, 4), (3, 5));
    for row in 2..7 {
        assert_eq!(forward.range_in_row(row, 25), backward.range_in_row(row, 25));
    }
}

#[test]
fn a_selection_inside_one_row() {
    let s = sel((7, 10), (7, 3));
    assert_eq!(s.range_in_row(7, 50), Some(3..10));
}

#[test]
fn a_click_without_drag_selects_no_text() {
    let s = sel((7, 3), (7, 3));
    assert!(s.is_empty());
    assert_eq!(s.range_in_row(7, 50), None);
    assert!(!s.covers_row(7));
}

#[test]
fn copying_gives_the_stored_text_of_each_row() {
    let lines = ["alpha one", "beta two", "gamma three", "delta four"];
    let rows: Vec<(usize, &str)> = lines.iter().copied().enumerate().collect();
    let s = sel((0, 6), (2, 5));
    assert_eq!(s.copy_text(rows.clone()), "one\nbeta two\ngamma");
    let whole = sel((1, 0), (2, 11));
    assert_eq!(whole.copy_text(rows), "beta two\ngamma three");
}

#[test]
fn copying_skips_rows_a_filter_hides() {
    // Row 2 is hidden: it is not among the rows shown.
    let rows: Vec<(usize, &str)> = vec![(0, "one"), (1, "two"), (3, "four"), (4, "five")];
    let s = sel((0, 0), (4, 4));
    assert_eq!(s.copy_text(rows), "one\ntwo\nfour\nfive");
}

#[test]
fn copying_keeps_an_empty_row_between_the_ends() {
    let rows: Vec<(usize, &str)> = vec![(0, "head"), (1, ""), (2, "tail")];
    let s = sel((0, 0), (2, 4));
    assert_eq!(s.copy_text(rows), "head\n\ntail");
}

#[test]
fn a_selection_meets_a_cell_span() {
    assert_eq!(clip_to_span(&(5..20), &(10..15)), Some(10..15));
    assert_eq!(clip_to_span(&(5..12), &(10..15)), Some(10..12));
    assert_eq!(clip_to_span(&(0..10), &(10..15)), None, "touching is not meeting");
    assert_eq!(clip_to_span(&(20..30), &(10..15)), None);
}

#[test]
fn hit_testing_follows_wide_glyphs() {
    // 8px per cell. "ab" then a wide glyph (two cells) then "c".
    let text = "abあc";
    assert_eq!(byte_at_x(text, 0.0, 8.0), 0);
    assert_eq!(byte_at_x(text, 3.0, 8.0), 0);
    assert_eq!(byte_at_x(text, 5.0, 8.0), 1);
    assert_eq!(byte_at_x(text, 17.0, 8.0), 2, "left half of the wide glyph");
    assert_eq!(byte_at_x(text, 27.0, 8.0), 5, "right half: after the glyph");
    assert_eq!(byte_at_x(text, 36.0, 8.0), 6, "the `c` is cell 4, not 3");
    assert_eq!(byte_at_x(text, 500.0, 8.0), text.len());
}

#[test]
fn copying_in_the_view_takes_every_selected_row_even_if_filter_would_hide_it() {
    // The view lists every line now: 10 consecutive rows, 3 of them INFO.
    let lines: Vec<String> = (0..10)
        .map(|i| format!("{} line {i}", if i % 4 == 1 { "INFO" } else { "ERROR" }))
        .collect();
    let rows = lines.iter().enumerate().map(|(i, l)| (i, l.as_str()));
    let s = sel((0, 0), (9, lines[9].len()));
    assert_eq!(s.copy_text(rows).lines().count(), 10);
}

#[test]
fn copying_in_the_pane_takes_only_the_rows_it_lists() {
    // Pane rows 0, 1, 2 are lines 10, 50 and 90 of the file.
    let listed = ["line 10", "line 50", "line 90"];
    let rows = listed.iter().enumerate().map(|(i, l)| (i, *l));
    let s = sel((0, 0), (2, listed[2].len()));
    assert_eq!(s.copy_text(rows), "line 10\nline 50\nline 90");
}
