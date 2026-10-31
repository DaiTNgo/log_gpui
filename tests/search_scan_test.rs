//! Search on files read through their Line Index (openspec change
//! `line-offset-index`, task 6.2): Enter, rows built again or another Focus
//! Tab finds the Matches by a Scan of the lines read from disk, not on the UI
//! thread; a file held in memory is searched at once.

mod common;

use common::{open_app_with_lines};
use gpui::TestAppContext;

fn lines() -> Vec<String> {
    (0..3000)
        .map(|i| {
            let what = if i % 7 == 0 { "request timeout" } else { "request done" };
            format!("2026-09-05 10:00:{:02} [INFO] [API] {what} {i}", i % 60)
        })
        .collect()
}

#[gpui::test]
fn enter_finds_the_matches_by_a_scan_and_goes_to_one(cx: &mut TestAppContext) {
    let mut h = open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.trigger_search_query("timeout", cx);
        assert!(app.search_scanning(), "the lines are read off the UI thread");
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.search_scanning());
        assert_eq!(app.focus.search().matched_visible_indices.len(), 429);
        assert!(app.selected_row_pos.is_some(), "Enter goes to a Match");
    });
}

#[gpui::test]
fn rows_built_again_find_the_matches_again_by_a_scan(cx: &mut TestAppContext) {
    let mut h = open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| app.trigger_search_query("timeout", cx));
    h.cx.run_until_parked();
    h.view.update(&mut h.cx, |app, cx| {
        app.apply_filters_and_refresh(cx);
        assert!(app.search_scanning(), "the Matches are found again off the UI thread");
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.search_scanning());
        assert_eq!(app.focus.search().matched_visible_indices.len(), 429);
    });
}
