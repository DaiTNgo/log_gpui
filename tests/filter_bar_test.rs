//! The Filter Bar and pausing the Filter (openspec change
//! `sidebar-and-selection-actions`): the Filter shows above the table as
//! chips, a chip's × removes its condition, and a paused Filter shows every
//! line while keeping its conditions.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::filter::{ColumnFilter, FilterState};
use log_gpui::workspace::state::ContextSection;

fn lines() -> Vec<String> {
    vec![
        "2026-10-19 10:00:01 [INFO] [API] open".to_string(),
        "2026-10-19 10:00:02 [ERROR] [DB] request timeout".to_string(),
        "2026-10-19 10:00:03 [WARN] [API] slow".to_string(),
        "2026-10-19 10:00:04 [DEBUG] [DB] pool ok".to_string(),
    ]
}

fn draw(h: &mut Harness) {
    h.cx.update(|window, _| window.refresh());
    h.cx.run_until_parked();
}

fn shown(h: &Harness) -> usize {
    h.view.read_with(&h.cx, |app, _| app.filter_pass.len())
}

#[test]
fn a_chip_names_its_values_and_counts_the_rest() {
    let f = ColumnFilter { column: "level".into(), values: vec!["error".into(), "warn".into()], ..Default::default() };
    assert_eq!(f.chip_text("Level", |v| v.to_uppercase()), "Level: ERROR, WARN");
    let f = ColumnFilter {
        column: "module".into(),
        values: vec!["db".into(), "api".into(), "auth".into()],
        custom: vec!["pay".into()],
        hide: true,
    };
    assert_eq!(f.chip_text("Module", |v| v.to_string()), "not Module: db, api +2");
    let f = ColumnFilter { column: "message".into(), custom: vec!["timeout".into()], ..Default::default() };
    assert_eq!(f.chip_text("Message", |v| v.to_string()), "Message: ∋ timeout");
}

#[test]
fn pausing_is_not_saved_and_a_change_applies_the_filter_again() {
    let mut f = FilterState::new();
    f.toggle_value("level", "ERROR");
    f.paused = true;
    assert!(!f.is_applied());
    let saved: FilterState = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
    assert!(!saved.paused, "a reopened view has its Filter on");
    f.toggle_value("level", "WARN");
    assert!(!f.paused);
}

#[gpui::test]
fn the_bar_shows_the_filter_and_a_chip_removes_its_condition(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    draw(&mut h);
    assert!(h.cx.debug_bounds("filter-chip-level").is_none(), "no bar while the Filter is empty");
    h.view.update(&mut h.cx, |app, cx| {
        app.context_sidebar.visible = false;
        app.filter_toggle_value("level", "ERROR", cx);
        app.filter_toggle_value("level", "WARN", cx);
    });
    draw(&mut h);
    assert_eq!(shown(&h), 2);
    let chip = h.cx.debug_bounds("filter-chip-level").expect("the chip shows with the sidebar closed");
    // The chip's body opens the Filter section on its card.
    h.cx.simulate_click(chip.center(), gpui::Modifiers::none());
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.context_sidebar.visible);
        assert!(!app.context_sidebar.is_collapsed(ContextSection::Filter));
        assert_eq!(app.filter_ui.expanded.as_deref(), Some("level"));
    });
    draw(&mut h);
    let remove = h.cx.debug_bounds("filter-chip-level-remove").unwrap();
    h.cx.simulate_click(remove.center(), gpui::Modifiers::none());
    draw(&mut h);
    assert_eq!(shown(&h), 4);
    h.view.read_with(&h.cx, |app, _| assert!(app.filter_state.is_empty()));
}

#[gpui::test]
fn a_paused_filter_shows_every_line_and_keeps_its_conditions(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.filter_set_hide("level", true, cx);
        app.filter_toggle_value("level", "DEBUG", cx);
    });
    h.cx.run_until_parked();
    assert_eq!(shown(&h), 3);
    h.view.update(&mut h.cx, |app, cx| app.toggle_filter_paused(cx));
    h.cx.run_until_parked();
    assert_eq!(shown(&h), 4, "DEBUG shows while paused");
    h.view.read_with(&h.cx, |app, _| assert!(app.filter_state.is_value_ticked("level", "debug")));
    h.view.update(&mut h.cx, |app, cx| app.toggle_filter_paused(cx));
    h.cx.run_until_parked();
    assert_eq!(shown(&h), 3);
    // Changing a condition while paused applies the Filter again.
    h.view.update(&mut h.cx, |app, cx| {
        app.toggle_filter_paused(cx);
        app.filter_toggle_value("level", "WARN", cx);
    });
    h.cx.run_until_parked();
    assert_eq!(shown(&h), 2);
}
