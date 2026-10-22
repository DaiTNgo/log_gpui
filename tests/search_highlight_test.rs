//! Search Highlight: whether the Focus Pane paints Search Matches.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::keymap::Command;

fn lines() -> Vec<String> {
    vec![
        "2026-10-19 10:00:01 INFO request ok".into(),
        "2026-10-19 10:00:02 INFO db timeout".into(),
        "2026-10-19 10:00:03 INFO cache timeout".into(),
    ]
}

/// Type `text` into the toolbar Search field, as a person would.
fn type_search(h: &mut Harness, text: &str) {
    h.view.update_in(&mut h.cx, |app, window, cx| app.activate_search(window, cx));
    h.cx.simulate_input(text);
    h.cx.executor().advance_clock(std::time::Duration::from_millis(150));
    h.cx.run_until_parked();
}

fn run(h: &mut Harness, command: Command) {
    h.view.update_in(&mut h.cx, |app, window, cx| app.execute_command(command, window, cx));
    h.cx.run_until_parked();
}

fn is_on(h: &mut Harness) -> bool {
    h.view.read_with(&h.cx, |app, _| app.workspace_state.show_search_highlight)
}

const LINE: &str = "2026-10-19 10:00:02 INFO db timeout";

#[test]
fn the_command_has_no_default_shortcut() {
    let command = Command::from_id("focus_pane.toggle_search_highlight").unwrap();
    assert!(command.default_shortcuts().is_empty());
}

#[gpui::test]
fn the_focus_pane_paints_search_matches_by_default(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    type_search(&mut h, "timeout");
    assert!(is_on(&mut h));
    let ranges = h.view.read_with(&h.cx, |app, _| app.focus_pane_search_ranges(LINE));
    assert_eq!(ranges, vec![(28, 35)]);
}

#[gpui::test]
fn the_command_turns_search_painting_off_and_on(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    type_search(&mut h, "timeout");

    run(&mut h, Command::ToggleSearchHighlight);
    assert!(!is_on(&mut h));
    let ranges = h.view.read_with(&h.cx, |app, _| app.focus_pane_search_ranges(LINE));
    assert!(ranges.is_empty());
    // The Focus Tab still lists the same lines.
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.focus_rows.len()), 2);

    run(&mut h, Command::ToggleSearchHighlight);
    assert!(is_on(&mut h));
    let ranges = h.view.read_with(&h.cx, |app, _| app.focus_pane_search_ranges(LINE));
    assert_eq!(ranges, vec![(28, 35)]);
}

#[gpui::test]
fn focus_find_still_paints_with_search_highlight_off(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    type_search(&mut h, "timeout");
    h.view.update(&mut h.cx, |app, cx| app.set_show_search_highlight(false, cx));
    h.view.update(&mut h.cx, |app, cx| app.on_focus_find_changed("db", cx));
    h.cx.run_until_parked();

    let (label, find) = h.view.read_with(&h.cx, |app, _| (app.focus.find().label(), app.focus.find().find_ranges(LINE)));
    assert_eq!(label, "1/1");
    assert_eq!(find, vec![(25, 27)]);
}

#[gpui::test]
fn the_setting_is_app_wide(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    type_search(&mut h, "timeout");
    h.view.update(&mut h.cx, |app, cx| app.set_show_search_highlight(false, cx));
    // A new Focus Tab, with a Search of its own, follows the same setting.
    h.view.update_in(&mut h.cx, |app, window, cx| app.add_focus_tab(window, cx));
    h.cx.run_until_parked();
    type_search(&mut h, "cache");
    let ranges = h.view.read_with(&h.cx, |app, _| app.focus_pane_search_ranges("INFO cache timeout"));
    assert!(ranges.is_empty());
}
