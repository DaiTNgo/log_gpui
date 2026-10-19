mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::focus::FocusPaneNotice;
use log_gpui::search::SearchState;

fn lines() -> Vec<String> {
    vec![
        "2026-10-19 10:00:01 INFO request ok".into(),
        "2026-10-19 10:00:02 INFO db timeout".into(),
        "2026-10-19 10:00:03 INFO cache miss".into(),
    ]
}

/// Type `text` into the toolbar Search field, as a person would.
fn type_search(h: &mut Harness, text: &str) {
    h.view.update_in(&mut h.cx, |app, window, cx| app.activate_search(window, cx));
    h.cx.simulate_input(text);
    h.cx.run_until_parked();
}

fn notice(h: &mut Harness) -> Option<FocusPaneNotice> {
    h.view.read_with(&h.cx, |app, _| app.focus_pane_notice())
}

fn counter(h: &mut Harness) -> String {
    h.view.read_with(&h.cx, |app, _| app.match_counter_label())
}

fn filter_level(h: &mut Harness, value: &str) {
    h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("level", value, cx));
    h.cx.run_until_parked();
}

#[test]
fn notice_for_empty_pane() {
    assert_eq!(FocusPaneNotice::for_empty_pane(None, false, false), FocusPaneNotice::Hint);
    assert_eq!(
        FocusPaneNotice::for_empty_pane(Some("tmeout"), false, false),
        FocusPaneNotice::NoMatches { expression: "tmeout".into(), filtering: false }
    );
    assert_eq!(FocusPaneNotice::for_empty_pane(None, true, true), FocusPaneNotice::FilterExcludesAll);
    // Filter leaves no line: that is the cause even with a Search Expression.
    assert_eq!(FocusPaneNotice::for_empty_pane(Some("db"), true, true), FocusPaneNotice::FilterExcludesAll);
    assert_eq!(
        FocusPaneNotice::for_empty_pane(Some("db"), true, false),
        FocusPaneNotice::NoMatches { expression: "db".into(), filtering: true }
    );
}

#[test]
fn counter_tells_no_search_from_no_matches() {
    assert_eq!(SearchState::counter_label(false, (0, 0)), "–");
    assert_eq!(SearchState::counter_label(true, (0, 0)), "0/0");
    assert_eq!(SearchState::counter_label(true, (2, 5)), "2/5");
}

#[gpui::test]
fn untouched_pane_shows_hint_and_dash(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    assert_eq!(notice(&mut h), Some(FocusPaneNotice::Hint));
    assert_eq!(counter(&mut h), "–");
}

#[gpui::test]
fn search_with_no_matches_says_so(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    type_search(&mut h, "tmeout");
    let n = notice(&mut h).expect("pane is empty");
    assert_eq!(n, FocusPaneNotice::NoMatches { expression: "tmeout".into(), filtering: false });
    assert!(n.title().contains("tmeout"), "{}", n.title());
    assert!(!n.offers_clear_filter());
    assert_eq!(counter(&mut h), "0/0");
}

#[gpui::test]
fn search_with_matches_lists_lines(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    type_search(&mut h, "timeout");
    assert_eq!(notice(&mut h), None);
    assert_eq!(counter(&mut h), "1/1");
}

#[gpui::test]
fn filter_excluding_every_line_offers_clear_filter(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    filter_level(&mut h, "error");
    let n = notice(&mut h).expect("pane is empty");
    assert_eq!(n, FocusPaneNotice::FilterExcludesAll);
    assert!(n.offers_clear_filter());

    h.view.update(&mut h.cx, |app, cx| app.filter_clear_all(cx));
    h.cx.run_until_parked();
    assert_eq!(notice(&mut h), Some(FocusPaneNotice::Hint));
}

#[gpui::test]
fn clear_filter_brings_search_rows_back(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    type_search(&mut h, "timeout");
    filter_level(&mut h, "error");
    assert_eq!(notice(&mut h), Some(FocusPaneNotice::FilterExcludesAll));

    h.view.update(&mut h.cx, |app, cx| app.filter_clear_all(cx));
    h.cx.run_until_parked();
    assert_eq!(notice(&mut h), None);
    let rows = h.view.read_with(&h.cx, |app, _| app.focus_rows.len());
    assert_eq!(rows, 1);
}

#[gpui::test]
fn focus_find_with_no_hits_says_no_matches(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    type_search(&mut h, "INFO");
    let label = |h: &mut Harness| h.view.read_with(&h.cx, |app, _| app.focus.find().label());
    assert_eq!(label(&mut h), "–");

    h.view.update(&mut h.cx, |app, cx| app.on_focus_find_changed("nothing-here", cx));
    h.cx.run_until_parked();
    assert_eq!(label(&mut h), "No matches");

    h.view.update(&mut h.cx, |app, cx| app.on_focus_find_changed("cache", cx));
    h.cx.run_until_parked();
    assert_eq!(label(&mut h), "1/1");
}
