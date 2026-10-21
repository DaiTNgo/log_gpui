//! Enter and Shift+Enter step through the Search's Matches wherever the
//! focus is, the same as in the search field; Enter typed into another
//! input stays with that input.
mod common;

use common::Harness;
use gpui::{Entity, Focusable as _, TestAppContext};
use gpui_component::input::InputState;

fn focus_app(h: &mut Harness) {
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let handle = app.focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    });
    h.cx.run_until_parked();
}

fn focus_input(h: &mut Harness, pick: fn(&log_gpui::app::AppView) -> Entity<InputState>) {
    h.view.update_in(&mut h.cx, |app, window, cx| {
        pick(app).update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    });
    h.cx.run_until_parked();
}

fn press(h: &mut Harness, keys: &str) {
    h.cx.simulate_keystrokes(keys);
    h.cx.run_until_parked();
}

fn current(h: &Harness) -> usize {
    h.view.read_with(&h.cx, |app, _| app.focus.search().current_match_idx)
}

/// Lines 1..=20, searched for "line 1" typed into the field: Matches are
/// lines 1 and 10..=19.
fn searched(cx: &mut TestAppContext) -> Harness {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    h.view.update_in(&mut h.cx, |app, window, cx| app.activate_search(window, cx));
    h.cx.simulate_input("line 1");
    h.cx.executor().advance_clock(std::time::Duration::from_millis(150));
    h.cx.run_until_parked();
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.focus.search().matched_visible_indices.len()), 11);
    assert_eq!(current(&h), 0);
    h
}

#[gpui::test]
fn enter_outside_the_field_steps_through_the_matches(cx: &mut TestAppContext) {
    let mut h = searched(cx);
    focus_app(&mut h);
    press(&mut h, "enter");
    assert_eq!(current(&h), 1, "Enter goes to the next Match");
    press(&mut h, "enter");
    assert_eq!(current(&h), 2);
    press(&mut h, "shift-enter");
    assert_eq!(current(&h), 1, "Shift+Enter goes back");
}

#[gpui::test]
fn enter_in_the_search_field_steps_once(cx: &mut TestAppContext) {
    let mut h = searched(cx);
    focus_input(&mut h, |app| app.search_input.clone());
    press(&mut h, "enter");
    assert_eq!(current(&h), 1, "one step, not one from the field and one from the window");
    press(&mut h, "shift-enter");
    assert_eq!(current(&h), 0);
}

#[gpui::test]
fn enter_in_another_input_leaves_the_search_alone(cx: &mut TestAppContext) {
    let mut h = searched(cx);
    focus_input(&mut h, |app| app.rule_input.clone());
    press(&mut h, "enter");
    assert_eq!(current(&h), 0);
}

#[gpui::test]
fn enter_and_space_without_a_search_do_nothing(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(5));
    focus_app(&mut h);
    press(&mut h, "enter space shift-enter");
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.focus.search().is_open);
        assert!(!app.is_palette_open);
    });
}
