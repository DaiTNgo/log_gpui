//! A Filter change re-evaluates the Filter and the Focus Pane only: Matches,
//! highlight paints and rule counts read every row of the view and stay.
mod common;

use gpui::TestAppContext;
use log_gpui::app::AppView;
use log_gpui::focus::FocusRow;
use std::sync::Arc;

fn lines() -> Vec<String> {
    let levels = ["INFO", "ERROR", "WARN", "INFO"];
    (0..40)
        .map(|i| format!("2026-10-19 10:{:02}:{:02} {} request {i} user={}", i / 60, i % 60, levels[i % 4], i % 7))
        .collect()
}

fn pane(app: &AppView) -> Vec<usize> {
    app.focus_rows.iter().filter_map(|r| if let FocusRow::Entry(ix) = r { Some(*ix) } else { None }).collect()
}

#[gpui::test]
fn matches_and_highlights_survive_a_filter_change(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        let color = log_gpui::palette::next_rule_color(0);
        app.add_highlight_rule("level", &["warn".to_string()], &[], color, cx).unwrap();
        app.trigger_search_query("user=3", cx);
    });
    // The Matches are found by a Scan of the lines on disk.
    h.cx.run_until_parked();
    let (matches, paints, counts, total) = h.view.read_with(&h.cx, |app, _| {
        (
            app.focus.search().matched_rows.clone(),
            app.highlight_paints.clone(),
            app.rule_counts.clone(),
            app.focus.search().total_matches(),
        )
    });
    assert!(total > 0);

    h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("level", "error", cx));
    h.view.read_with(&h.cx, |app, _| {
        assert!(Arc::ptr_eq(&matches, &app.focus.search().matched_rows), "Matches were not recomputed");
        assert!(Arc::ptr_eq(&paints, &app.highlight_paints), "paints were not recomputed");
        assert_eq!(counts, app.rule_counts);
        assert_eq!(app.focus.search().total_matches(), total);
        // The pane lists the Matches that also pass the Filter.
        let expected: Vec<usize> =
            app.focus.search().matched_rows.iter().copied().filter(|ix| app.filter_pass.contains(*ix)).collect();
        assert_eq!(pane(app), expected);
        assert_eq!(app.filter_pass.len(), 10);
    });

    h.view.update(&mut h.cx, |app, cx| app.filter_clear_all(cx));
    h.view.read_with(&h.cx, |app, _| {
        assert!(Arc::ptr_eq(&matches, &app.focus.search().matched_rows));
        assert_eq!(app.filter_pass.len(), 40);
    });
}

#[gpui::test]
fn the_focus_pane_follows_the_filter_without_a_search(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("level", "warn", cx));
    h.view.read_with(&h.cx, |app, _| {
        let expected: Vec<usize> = (0..40).filter(|i| i % 4 == 2).collect();
        assert_eq!(pane(app), expected);
    });
}

#[gpui::test]
fn no_filter_keeps_no_list_of_rows(cx: &mut TestAppContext) {
    use log_gpui::filter::FilterPass;
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.filter_pass, FilterPass::All { rows: 40 }));
    h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("level", "error", cx));
    h.view.read_with(&h.cx, |app, _| assert!(matches!(app.filter_pass, FilterPass::Rows(_))));
    // Paused, every row passes again: no list either.
    h.view.update(&mut h.cx, |app, cx| app.toggle_filter_paused(cx));
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.filter_pass, FilterPass::All { rows: 40 }));
    h.view.update(&mut h.cx, |app, cx| app.toggle_filter_paused(cx));
    h.view.update(&mut h.cx, |app, cx| app.filter_clear_all(cx));
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.filter_pass, FilterPass::All { rows: 40 }));
}

#[gpui::test]
fn no_painting_rule_keeps_no_byte_per_row(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.highlight_paints.len(), 40);
        assert_eq!(app.highlight_paints.heap_bytes(), 0);
    });
    let id = h.view.update(&mut h.cx, |app, cx| {
        let color = log_gpui::palette::next_rule_color(0);
        app.add_highlight_rule("level", &["warn".to_string()], &[], color, cx).unwrap();
        app.highlight_rules()[0].id
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.highlight_paints.heap_bytes() >= 40);
        assert_eq!(app.highlight_paints.iter().filter(|p| p.is_some()).count(), 10);
    });
    h.view.update(&mut h.cx, |app, cx| app.toggle_highlight_rule(id, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.highlight_paints.heap_bytes(), 0, "the last rule turned off");
        assert!(app.highlight_paints.iter().all(|p| p.is_none()));
    });
}
