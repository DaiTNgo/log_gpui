//! The context sidebar (openspec change `sidebar-and-selection-actions`):
//! the Inspector pinned under a splitter, the size kept with the session,
//! compact Filter and Highlights sections, and the Inspector's Fields.

mod common;

use common::Harness;
use gpui::{Entity, TestAppContext};
use gpui_component::input::InputState;
use log_gpui::app::context_layout::preset_from;
use log_gpui::workspace::state::{
    ContextSection, ContextSidebarLayout, ContextSidebarState, InspectorMode, MAX_CONTEXT_SIDEBAR_WIDTH,
    MIN_CONTEXT_SIDEBAR_WIDTH,
};

fn lines() -> Vec<String> {
    (0..60)
        .map(|i| {
            let (level, module) = match i % 4 {
                0 => ("INFO", "API"),
                1 => ("ERROR", "DB"),
                2 => ("WARN", "PAY"),
                _ => ("DEBUG", "AUTH"),
            };
            format!("2026-10-18 08:{:02}:00 [{level}] [{module}] request {i}", i % 60)
        })
        .collect()
}

fn draw(h: &mut Harness) {
    h.cx.update(|window, _| window.refresh());
    h.cx.run_until_parked();
}

fn focus_input(h: &mut Harness, pick: fn(&log_gpui::app::AppView) -> Entity<InputState>) {
    h.view.update_in(&mut h.cx, |app, window, cx| {
        pick(app).update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    });
    h.cx.run_until_parked();
}

#[test]
fn the_width_and_the_splitter_keep_their_limits() {
    let mut layout = ContextSidebarLayout::default();
    layout.set_width(100.0);
    assert_eq!(layout.width, MIN_CONTEXT_SIDEBAR_WIDTH);
    layout.set_width(2000.0);
    assert_eq!(layout.width, MAX_CONTEXT_SIDEBAR_WIDTH);
    layout.set_inspector_height(10.0, 600.0);
    assert_eq!(layout.inspector_height(600.0), 120.0);
    layout.set_inspector_height(590.0, 600.0);
    assert_eq!(layout.inspector_height(600.0), 480.0);
}

#[test]
fn columns_start_collapsed() {
    assert!(ContextSidebarState::default().is_collapsed(ContextSection::Columns));
}

#[test]
fn a_session_without_the_layout_reads_with_the_defaults_and_keeps_a_new_one() {
    let old: log_gpui::workspace::session::SessionManifest =
        serde_json::from_str(r#"{"version":5,"active_project_id":null,"projects":[]}"#).unwrap();
    assert_eq!(old.context_layout, ContextSidebarLayout::default());
    let mut ws = old.into_workspace();
    ws.context_layout.set_width(400.0);
    ws.context_layout.inspector_mode = InspectorMode::Raw;
    let saved = serde_json::to_string(&log_gpui::workspace::session::SessionManifest::from_workspace(&ws)).unwrap();
    let back: log_gpui::workspace::session::SessionManifest = serde_json::from_str(&saved).unwrap();
    assert_eq!(back.context_layout.width, 400.0);
    assert_eq!(back.context_layout.inspector_mode, InspectorMode::Raw);
}

#[test]
fn a_preset_counts_back_from_the_newest_line() {
    let newest = chrono::NaiveDate::from_ymd_opt(2026, 10, 18).unwrap().and_hms_opt(8, 40, 0).unwrap().and_utc().timestamp();
    assert_eq!(preset_from(newest as f64, 3600).as_deref(), Some("2026-10-18 07:40:00"));
}

#[gpui::test]
fn the_1h_preset_follows_the_log_not_the_clock(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| app.apply_time_preset(Some(5 * 60), cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.filter_state.time_range.from, "2026-10-18 08:54:00");
        assert_eq!(app.filter_state.time_range.to, "");
        assert_eq!(app.filter_pass.len(), 6, "08:54 to 08:59");
    });
    h.view.update(&mut h.cx, |app, cx| app.apply_time_preset(None, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert!(!app.filter_state.time_range.is_active()));
}

#[gpui::test]
fn the_inspector_stays_in_sight_when_a_long_form_opens(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.jump_to_row(3, cx);
        app.context_sidebar.reveal(ContextSection::Filter);
        app.context_sidebar.reveal(ContextSection::Highlights);
        app.open_rule_form(cx);
        app.rule_form_pick_column("module", cx);
        app.toggle_filter_column("level", cx);
    });
    draw(&mut h);
    let splitter = h.cx.debug_bounds("ctx-inspector-splitter").expect("the splitter above the Inspector");
    let window = h.cx.update(|window, _| window.viewport_size());
    assert!(splitter.bottom() < window.height, "{splitter:?} in {window:?}");
}

#[gpui::test]
fn plus_again_keeps_the_new_rule_form(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.open_rule_form(cx);
        app.rule_form_pick_column("module", cx);
        app.rule_form_toggle_value("DB", cx);
        app.open_rule_form(cx);
    });
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.rule_form.as_ref().unwrap().values, ["DB"]));
}

#[gpui::test]
fn enter_in_the_value_input_adds_a_custom_value(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.context_sidebar.reveal(ContextSection::Filter);
        app.toggle_filter_column("module", cx);
    });
    draw(&mut h);
    focus_input(&mut h, |app| app.filter_ui.search_input.clone());
    h.cx.simulate_input("pa");
    h.cx.simulate_keystrokes("enter");
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, cx| {
        assert_eq!(app.filter_state.column("module").unwrap().custom, ["pa"]);
        assert_eq!(app.filter_ui.search_input.read(cx).value().as_ref(), "", "the input empties");
        assert_eq!(app.filter_pass.len(), 15);
    });
}

#[gpui::test]
fn the_inspector_lists_the_fields_and_filters_from_them(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| app.jump_to_row(1, cx));
    let fields = h.view.read_with(&h.cx, |app, _| app.inspector_fields());
    let pairs: Vec<(&str, &str)> = fields.iter().map(|f| (f.key.as_str(), f.value.as_str())).collect();
    assert!(pairs.contains(&("level", "ERROR")), "{pairs:?}");
    assert!(pairs.contains(&("module", "DB")), "{pairs:?}");
    assert_eq!(pairs.first().map(|p| p.0), Some("timestamp"));
    assert_eq!(pairs.last(), Some(&("message", "request 1")));
    // Hide from the field.
    h.view.update(&mut h.cx, |app, cx| app.filter_from_selection("module", "DB", true, true, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let c = app.filter_state.column("module").unwrap();
        assert!(c.hide && c.values == ["db"]);
    });
}
