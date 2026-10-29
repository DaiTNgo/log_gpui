//! A Highlight Rule that needs the text of the rows' values (openspec
//! change `line-offset-index`, task 7.2): the rows around the top row are
//! painted at once, the rest of a file read from disk by a Scan.

mod common;

use common::{open_app_with_held_lines, open_app_with_lines};
use gpui::TestAppContext;
use log_gpui::highlight::{HighlightRule, paint_rows};
use log_gpui::palette::next_rule_color;

fn lines() -> Vec<String> {
    (0..30_000)
        .map(|i| {
            let what = if i % 7 == 0 { "request timeout" } else { "request done" };
            let level = if i % 10 == 0 { "ERROR" } else { "INFO" };
            format!("2026-09-05 10:00:{:02} [{level}] [API] {what} {i}", i % 60)
        })
        .collect()
}

fn rules() -> Vec<HighlightRule> {
    vec![
        HighlightRule::new(1, "message", &[], &["timeout".to_string()], next_rule_color(0)).unwrap(),
        HighlightRule::new(2, "level", &["error".to_string()], &[], next_rule_color(1)).unwrap(),
    ]
}

fn add_rules(app: &mut log_gpui::app::AppView, cx: &mut gpui::Context<log_gpui::app::AppView>) {
    for rule in rules() {
        app.add_highlight_rule(&rule.column, &rule.values, &rule.custom, rule.color.clone(), cx).unwrap();
    }
}

#[gpui::test]
fn a_rule_on_the_message_paints_the_top_rows_at_once_and_the_rest_by_a_scan(cx: &mut TestAppContext) {
    let mut h = open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        add_rules(app, cx);
        assert!(app.paint_is_scanning());
        let paints = app.highlight_paints.clone();
        assert_eq!(paints.get(7).map(|p| p.rule), Some(0), "a row on screen is painted at once");
        assert!(paints.get(20_010).is_none(), "an ERROR row far down waits: the rule on the message comes first");
        assert!(paints.get(21_000).is_none(), "a timeout row far down waits for the Scan");
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.paint_is_scanning());
        let expected = paint_rows(&rules(), &app.rows_snapshot());
        let have: Vec<Option<usize>> = app.highlight_paints.iter().map(|p| p.map(|p| p.rule)).collect();
        let want: Vec<Option<usize>> = expected.iter().map(|p| p.map(|p| p.rule)).collect();
        assert_eq!(have, want, "every row painted as reading every row would");
        let ids: Vec<u64> = app.highlight_rules().iter().map(|r| r.id).collect();
        assert_eq!(app.rule_counts[&ids[0]], 30_000_usize.div_ceil(7));
        assert_eq!(app.rule_counts[&ids[1]], 3000);
    });
}

#[gpui::test]
fn a_held_file_is_painted_at_once(cx: &mut TestAppContext) {
    let mut h = open_app_with_held_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        add_rules(app, cx);
        assert!(!app.paint_is_scanning());
        assert_eq!(app.highlight_paints.get(21_000).map(|p| p.rule), Some(0));
    });
}

#[gpui::test]
fn a_rule_told_by_value_ids_before_it_paints_at_once(cx: &mut TestAppContext) {
    let mut h = open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        add_rules(app, cx);
        // The level rule first: its rows need no text.
        app.move_highlight_rule(1, 0, cx);
        assert_eq!(app.highlight_paints.get(20_010).map(|p| p.rule), Some(0));
        assert!(app.highlight_paints.get(21_007).is_none());
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.highlight_paints.get(21_007).map(|p| p.rule), Some(1)));
}
