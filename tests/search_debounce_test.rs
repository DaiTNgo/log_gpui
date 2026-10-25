//! Typing in Search waits for a pause, looks for the Matches on a worker
//! thread for the last text only, and Live lines that arrive meanwhile are
//! not lost or counted twice. Time Range text re-filters only when it reads.
mod common;

use common::fake_live::{lines, open};
use gpui::TestAppContext;
use std::time::Duration;

fn matches(f: &common::fake_live::Files) -> Vec<usize> {
    f.h.view.read_with(&f.h.cx, |app, _| app.focus.search().matched_visible_indices.to_vec())
}

fn type_text(f: &mut common::fake_live::Files, text: &str) {
    f.h.view.update(&mut f.h.cx, |app, cx| app.type_search_query(text, cx));
}

/// Past the pause, and until the Scan it starts (on worker threads) is done.
fn wait_for_scan(f: &mut common::fake_live::Files) {
    f.h.cx.executor().advance_clock(Duration::from_millis(150));
    f.h.cx.run_until_parked();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while scanning(f) {
        assert!(std::time::Instant::now() < deadline, "the Scan ends");
        std::thread::sleep(Duration::from_millis(1));
        f.h.cx.run_until_parked();
    }
}

fn scanning(f: &common::fake_live::Files) -> bool {
    f.h.view.read_with(&f.h.cx, |app, _| app.search_scanning())
}

fn scans(f: &common::fake_live::Files) -> usize {
    f.h.view.read_with(&f.h.cx, |app, _| app.refreshes.scans)
}

/// The Matches a search found at once gives, for comparing.
fn matches_found_at_once(f: &mut common::fake_live::Files, text: &str) -> Vec<usize> {
    f.h.view.update(&mut f.h.cx, |app, cx| app.trigger_search_query(text, cx));
    matches(f)
}

#[gpui::test]
fn typing_quickly_scans_once_for_the_last_text(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 200, 1_000_000);
    for text in ["u", "us", "use", "user=", "user=4"] {
        type_text(&mut f, text);
        f.h.cx.executor().advance_clock(Duration::from_millis(30));
        f.h.cx.run_until_parked();
    }
    assert_eq!(scans(&f), 0, "typing without a pause scans nothing");
    assert!(matches(&f).is_empty(), "the Matches wait for the pause");
    assert!(f.h.view.read_with(&f.h.cx, |app, _| app.focus.search().pending));
    wait_for_scan(&mut f);
    assert_eq!(scans(&f), 1, "one scan, for the last text");
    let found = matches(&f);
    assert!(!found.is_empty());
    assert!(!f.h.view.read_with(&f.h.cx, |app, _| app.focus.search().pending));
    assert_eq!(found, matches_found_at_once(&mut f, "user=4"), "the same Matches as a search at once");
    let label = f.h.view.read_with(&f.h.cx, |app, _| app.match_counter_label());
    assert!(label.ends_with(&format!("/{}", found.len())), "the counter shows the final result: {label}");
}

#[gpui::test]
fn lines_arriving_during_a_scan_are_neither_lost_nor_counted_twice(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 100, 1_000_000);
    f.send("alpha", lines("alpha", 100..130, 0));
    type_text(&mut f, "user=3");
    // A batch before the pause is over, one while the scan runs, one after.
    f.pump();
    f.h.cx.executor().advance_clock(Duration::from_millis(150));
    while scans(&f) == 0 {
        f.h.cx.executor().tick();
    }
    f.send("alpha", lines("alpha", 130..170, 0));
    f.pump();
    f.h.cx.run_until_parked();
    wait_for_scan(&mut f);
    f.send("alpha", lines("alpha", 170..200, 0));
    f.pump();
    f.h.cx.run_until_parked();
    let found = matches(&f);
    let mut sorted = found.clone();
    sorted.dedup();
    assert_eq!(found, sorted, "no Match twice");
    assert_eq!(found, matches_found_at_once(&mut f, "user=3"));
}

#[gpui::test]
fn half_typed_time_range_text_does_not_refilter(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 60, 1_000_000);
    let from = f.h.view.read_with(&f.h.cx, |app, _| app.filter_ui.from_input.clone());
    let filters = |f: &common::fake_live::Files| f.h.view.read_with(&f.h.cx, |app, _| app.refreshes.filter);
    let type_from = |f: &mut common::fake_live::Files, text: &'static str| {
        let from = from.clone();
        f.h.cx.update(|window, cx| from.update(cx, |state, cx| state.set_value(text, window, cx)));
        f.h.view.update(&mut f.h.cx, |app, cx| app.on_time_inputs_typed(cx));
    };
    let before = filters(&f);
    for text in ["2026-10", "2026-10-19 10:", "2026-10-19T"] {
        type_from(&mut f, text);
        wait_for_scan(&mut f);
    }
    assert_eq!(filters(&f), before, "text that does not read leaves the Filter alone");
    type_from(&mut f, "2026-10-19 10:00:20");
    assert_eq!(filters(&f), before, "it waits for the pause");
    wait_for_scan(&mut f);
    assert_eq!(filters(&f), before + 1);
    assert!(f.h.view.read_with(&f.h.cx, |app, _| app.filter_pass.len()) < 62);
    // The same bounds written another way do not filter again.
    type_from(&mut f, "2026-10-19 10:00:20.000");
    wait_for_scan(&mut f);
    assert_eq!(filters(&f), before + 1);
}

#[gpui::test]
fn a_newer_query_replaces_the_scan_of_the_one_before(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 200, 1_000_000);
    type_text(&mut f, "user=4");
    wait_for_scan(&mut f);
    type_text(&mut f, "user=5");
    assert!(!scanning(&f), "nothing scans until the pause");
    assert!(f.h.view.read_with(&f.h.cx, |app, _| app.focus.search().pending));
    wait_for_scan(&mut f);
    assert_eq!(scans(&f), 2);
    assert_eq!(matches(&f), matches_found_at_once(&mut f, "user=5"), "only the newer query's Matches");
}

#[gpui::test]
fn a_full_refresh_finds_the_matches_at_once(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 200, 1_000_000);
    type_text(&mut f, "user=4");
    f.h.view.update(&mut f.h.cx, |app, cx| app.apply_filters_and_refresh(cx));
    assert!(!scanning(&f), "no Scan is left running");
    let found = matches(&f);
    assert_eq!(found, matches_found_at_once(&mut f, "user=4"));
}

#[gpui::test]
fn the_counter_tells_a_scan_under_way(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 200, 1_000_000);
    type_text(&mut f, "user=4");
    wait_for_scan(&mut f);
    let label = f.h.view.update(&mut f.h.cx, |app, _| {
        // As the Scan reports while it runs: the Matches so far, and how far.
        app.focus.search_mut().scanning = Some((30, 100));
        app.match_counter_label()
    });
    assert!(label.ends_with(" · 30%"), "{label}");
}
