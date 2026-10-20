//! "Make column…" and Extraction Rules in the real app.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::column::PinnableColumn;
use log_gpui::focus::FocusPaneNotice;
use log_gpui::model::FieldKey;
use log_gpui::palette::{HighlightColor, Hue, Shade};
use log_gpui::text_selection::{TextPos, TextSelection};
use std::time::Duration;

fn lines() -> Vec<String> {
    let mut lines: Vec<String> = (0..30)
        .map(|i| {
            let user = ["alice", "bob", "carol"][i % 3];
            format!("2026-10-19 10:00:{:02} INFO [api] request {i} user={user} took=3ms", i)
        })
        .collect();
    lines.push("2026-10-19 10:00:40 INFO [api] health check ok".into());
    lines.push("2026-10-19 10:00:41 WARN [api] request 31 user= anonymous".into());
    lines
}

fn select(h: &mut Harness, row: usize, value: &str) {
    h.view.update(&mut h.cx, |app, _| {
        let line = app.all_entries[app.visible_indices[row]].raw_line.to_string();
        let s = line.find(value).unwrap();
        app.text_selection = Some(TextSelection::new(TextPos::new(row, s), TextPos::new(row, s + value.len())));
    });
}

fn make_column(h: &mut Harness) {
    let (row, sel) = h.view.read_with(&h.cx, |app, _| app.make_column_target()).expect("one-line selection");
    h.view.update_in(&mut h.cx, |app, window, cx| app.open_new_column(row, sel, window, cx));
    h.cx.run_until_parked();
}

fn visible(h: &mut Harness) -> usize {
    h.view.read_with(&h.cx, |app, _| app.filter_pass.len())
}

fn add_user_column(h: &mut Harness) {
    select(h, 0, "alice");
    make_column(h);
    h.view.update(&mut h.cx, |app, cx| app.confirm_new_column(cx));
    h.cx.run_until_parked();
}

#[gpui::test]
fn make_column_from_a_selection(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    select(&mut h, 3, "alice");
    make_column(&mut h);
    h.view.read_with(&h.cx, |app, cx| {
        let d = app.new_column.as_ref().expect("dialog open");
        assert_eq!(d.name_input.read(cx).value().to_string(), "user");
        assert_eq!(d.candidates[0].description, "after `user=`");
        let p = d.preview.as_ref().unwrap();
        assert_eq!(p.matched[0].row, 3, "the selected line first");
        assert_eq!(&p.matched[0].text[p.matched[0].highlight.clone()], "alice");
        assert_eq!(p.unmatched.as_ref().unwrap().row, 31, "a line with the key but no value");
        assert_eq!(p.summary(), "30 of 32 lines (93%)");
        assert!(d.can_save());
    });
    h.view.update(&mut h.cx, |app, cx| app.confirm_new_column(cx));
    h.cx.run_until_parked();

    let user = PinnableColumn::Field(FieldKey::new("user"));
    assert!(h.view.read_with(&h.cx, |app, _| app.new_column.is_none()));
    assert!(h.view.read_with(&h.cx, |app, _| app.column_layout.columns().contains(&user)));
    h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("user", "alice", cx));
    h.cx.run_until_parked();
    assert_eq!(visible(&mut h), 10);
    h.view.update(&mut h.cx, |app, cx| {
        app.add_highlight_rule("user", &["bob".into()], &[], HighlightColor::palette(Hue::Red, Shade::Medium), cx)
            .unwrap();
    });
    let painted = h.view.read_with(&h.cx, |app, _| app.highlight_paints.iter().filter(|p| p.is_some()).count());
    assert_eq!(painted, 10);
}

#[gpui::test]
fn a_half_typed_regex_keeps_the_last_preview_and_errs_only_after_a_pause(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    select(&mut h, 0, "alice");
    make_column(&mut h);
    h.view.update(&mut h.cx, |app, cx| app.toggle_new_column_regex(cx));
    h.cx.run_until_parked();
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let input = app.new_column.as_ref().unwrap().regex_input.clone();
        input.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
    });
    h.cx.simulate_input(r"user=(\w+");
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let d = app.new_column.as_ref().unwrap();
        assert!(d.stale, "keeps the last valid Preview");
        assert!(d.preview.is_some());
        assert!(!d.can_save());
        assert!(d.pattern.visible_error().is_none(), "no error while typing");
    });
    h.cx.executor().advance_clock(Duration::from_millis(600));
    h.cx.run_until_parked();
    assert!(h.view.read_with(&h.cx, |app, _| app.new_column.as_ref().unwrap().pattern.visible_error().is_some()));
    h.cx.simulate_input(")");
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let d = app.new_column.as_ref().unwrap();
        assert!(!d.stale && d.can_save() && d.pattern.visible_error().is_none());
    });
    h.cx.simulate_keystrokes("escape");
    h.cx.run_until_parked();
    assert!(h.view.read_with(&h.cx, |app, _| app.new_column.is_none()), "Esc closes the dialog");
}

#[gpui::test]
fn turning_the_rule_off_while_filtering_explains_the_empty_pane(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    add_user_column(&mut h);
    h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("user", "alice", cx));
    h.cx.run_until_parked();
    let id = h.view.read_with(&h.cx, |app, _| app.extraction_rules()[0].id);
    h.view.update(&mut h.cx, |app, cx| app.set_extraction_rule_enabled(id, false, cx));
    h.cx.run_until_parked();

    assert_eq!(visible(&mut h), 0);
    let notice = h.view.read_with(&h.cx, |app, _| app.focus_pane_notice()).unwrap();
    assert_eq!(
        notice.title(),
        "No lines: Filter uses user, which is no longer read (rule turned off)"
    );
    assert!(matches!(notice, FocusPaneNotice::UnreadColumn { can_turn_on: true, .. }));
    assert!(h.view.read_with(&h.cx, |app, _| app.filter_state.column("user").is_some_and(|c| c.is_active())));

    h.view.update(&mut h.cx, |app, cx| app.turn_column_back_on("user", cx));
    h.cx.run_until_parked();
    assert_eq!(visible(&mut h), 10);
}

#[gpui::test]
fn a_deleted_rule_can_come_back(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    add_user_column(&mut h);
    h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("user", "bob", cx));
    let id = h.view.read_with(&h.cx, |app, _| app.extraction_rules()[0].id);
    h.view.update(&mut h.cx, |app, cx| app.remove_extraction_rule(id, cx));
    h.cx.run_until_parked();
    let notice = h.view.read_with(&h.cx, |app, _| app.focus_pane_notice()).unwrap();
    assert!(notice.title().contains("rule deleted"), "{}", notice.title());
    h.view.update(&mut h.cx, |app, cx| app.turn_column_back_on("user", cx));
    h.cx.run_until_parked();
    assert_eq!(visible(&mut h), 10);
}

#[gpui::test]
fn rules_survive_reopening_the_project(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    add_user_column(&mut h);
    let manifest = h.view.read_with(&h.cx, |app, _| {
        log_gpui::workspace::session::SessionManifest::from_workspace(&app.workspace_state)
    });
    let back = log_gpui::workspace::session::SessionManifest::from_json(&manifest.to_json().unwrap()).unwrap();
    let ws = back.into_workspace();
    let rules = &ws.projects[0].extraction_rules;
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].name, "user");
    assert_eq!(rules[0].description, "after `user=`");
}

#[gpui::test]
fn the_rules_group_lists_rules_and_live_lines_use_them(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    add_user_column(&mut h);
    let groups = h.view.read_with(&h.cx, |app, _| app.column_groups(""));
    assert_eq!(groups.rules.len(), 1);
    assert_eq!(groups.rules[0].lines, 30);
    assert!(groups.detected.iter().all(|r| r.key != "user"), "a rule's field is not a Detected Field");
}
