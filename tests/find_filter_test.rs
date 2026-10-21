//! Find Filter: Focus Find narrows the Focus Pane to the rows it finds.
mod common;

use common::Harness;
use gpui::TestAppContext;

fn lines() -> Vec<String> {
    vec![
        "2026-10-19 10:00:01 INFO request ok".into(),
        "2026-10-19 10:00:02 INFO db timeout".into(),
        "2026-10-19 10:00:03 INFO cache miss".into(),
        "2026-10-19 10:00:04 INFO db retry".into(),
        "2026-10-19 10:00:05 WARN disk low".into(),
    ]
}

fn search(h: &mut Harness, text: &str) {
    h.view.update(&mut h.cx, |app, cx| app.trigger_search_query(text, cx));
    h.cx.run_until_parked();
}

fn find(h: &mut Harness, text: &str) {
    h.view.update(&mut h.cx, |app, cx| app.on_focus_find_changed(text, cx));
    h.cx.run_until_parked();
}

fn toggle(h: &mut Harness) {
    h.view.update(&mut h.cx, |app, cx| app.toggle_find_filter(cx));
    h.cx.run_until_parked();
}

fn row_count(h: &mut Harness) -> usize {
    h.view.read_with(&h.cx, |app, _| app.focus_rows.len())
}

fn label(h: &mut Harness) -> String {
    h.view.read_with(&h.cx, |app, _| app.focus.find().label())
}

#[gpui::test]
fn find_filter_keeps_only_the_rows_focus_find_finds(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    search(&mut h, "INFO");
    find(&mut h, "db");
    assert_eq!(row_count(&mut h), 4);
    assert_eq!(label(&mut h), "1/2");

    toggle(&mut h);
    assert_eq!(row_count(&mut h), 2);
    assert_eq!(label(&mut h), "2 of 4 lines");

    toggle(&mut h);
    assert_eq!(row_count(&mut h), 4);
}

#[gpui::test]
fn with_empty_text_the_find_filter_narrows_nothing(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    search(&mut h, "INFO");
    toggle(&mut h);
    assert_eq!(row_count(&mut h), 4);
    let filtering = h.view.read_with(&h.cx, |app, _| app.focus.find().is_filtering());
    assert!(!filtering, "the funnel is not shown as active");

    // Typing while it is on narrows at once.
    find(&mut h, "db");
    assert_eq!(row_count(&mut h), 2);
    assert_eq!(label(&mut h), "2 of 4 lines");
    find(&mut h, "");
    assert_eq!(row_count(&mut h), 4);
}

#[gpui::test]
fn next_and_previous_step_through_the_kept_rows(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    search(&mut h, "INFO");
    find(&mut h, "db");
    toggle(&mut h);
    let current = |h: &mut Harness| h.view.read_with(&h.cx, |app, _| app.current_line().map(|l| l.line));
    h.view.update(&mut h.cx, |app, cx| app.focus_find_next(cx));
    assert_eq!(current(&mut h), Some(4));
    h.view.update(&mut h.cx, |app, cx| app.focus_find_next(cx));
    assert_eq!(current(&mut h), Some(2));
    h.view.update(&mut h.cx, |app, cx| app.focus_find_prev(cx));
    assert_eq!(current(&mut h), Some(4));
}

#[gpui::test]
fn the_find_filter_narrows_the_raw_view_too(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| app.set_view_mode(log_gpui::workspace::working_set::ViewMode::Raw, cx));
    h.cx.run_until_parked();
    search(&mut h, "INFO");
    find(&mut h, "db");
    assert_eq!(row_count(&mut h), 4);
    toggle(&mut h);
    assert_eq!(row_count(&mut h), 2);
    assert_eq!(label(&mut h), "2 of 4 lines");
}

#[gpui::test]
fn each_focus_tab_keeps_its_own_find_filter(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    search(&mut h, "INFO");
    find(&mut h, "db");
    toggle(&mut h);
    h.view.update_in(&mut h.cx, |app, window, cx| app.add_focus_tab(window, cx));
    h.cx.run_until_parked();
    find(&mut h, "db");
    assert_eq!(row_count(&mut h), 4, "the new tab's Find Filter is off");

    h.view.update(&mut h.cx, |app, cx| app.switch_focus_tab(0, cx));
    h.cx.run_until_parked();
    assert_eq!(row_count(&mut h), 2);
    h.view.update(&mut h.cx, |app, cx| app.switch_focus_tab(1, cx));
    h.cx.run_until_parked();
    assert_eq!(row_count(&mut h), 4);
}

#[gpui::test]
fn reopening_the_log_space_starts_with_the_find_filter_off(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    search(&mut h, "INFO");
    find(&mut h, "db");
    toggle(&mut h);
    assert_eq!(row_count(&mut h), 2);
    let space_id = h.view.read_with(&h.cx, |app, _| {
        let project = app.workspace_state.active_project().unwrap();
        project.working_set.active_tab().unwrap().target.space_id().unwrap().to_string()
    });

    h.view.update(&mut h.cx, |app, cx| app.close_active_tab(cx));
    h.cx.run_until_parked();
    h.view.update(&mut h.cx, |app, cx| app.open_log_space_tab(&space_id, cx));
    h.cx.run_until_parked();

    let (text, filter) = h.view.read_with(&h.cx, |app, _| (app.focus.find().query.clone(), app.focus.find().filter));
    assert_eq!(text, "db", "the find text is restored");
    assert!(!filter);
    assert_eq!(row_count(&mut h), 4);
}

#[gpui::test]
fn the_keymap_command_toggles_the_find_filter(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    search(&mut h, "INFO");
    find(&mut h, "db");
    let command = log_gpui::keymap::Command::from_id("focus_pane.toggle_find_filter").expect("command");
    assert!(command.default_shortcuts().is_empty());
    h.view.update_in(&mut h.cx, |app, window, cx| app.execute_command(command, window, cx));
    h.cx.run_until_parked();
    assert_eq!(row_count(&mut h), 2);
}

#[test]
fn the_find_filter_is_not_stored() {
    let mut tabs = log_gpui::focus::FocusTabs::default();
    tabs.find_mut().query = "db".into();
    tabs.find_mut().filter = true;
    let json = serde_json::to_string(&tabs).unwrap();
    let back: log_gpui::focus::FocusTabs = serde_json::from_str(&json).unwrap();
    assert_eq!(back.find().query, "db");
    assert!(!back.find().filter);
}
