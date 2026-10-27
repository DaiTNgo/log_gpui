//! Moving between rows by keyboard, with or without a selected row.
mod common;

use common::Harness;
use gpui::{Focusable as _, TestAppContext};
use log_gpui::go_to::{RowKey, row_for_key};

#[test]
fn keys_land_on_the_rows_in_sight_or_next_to_the_selection() {
    // 100 rows, rows 40..60 in sight.
    let land = |key, selected| row_for_key(key, selected, 40, 20, 100);
    assert_eq!(land(RowKey::Down, None), Some(40), "first row in sight");
    assert_eq!(land(RowKey::Up, None), Some(59), "last row in sight");
    assert_eq!(land(RowKey::Down, Some(10)), Some(11));
    assert_eq!(land(RowKey::Up, Some(0)), Some(0));
    assert_eq!(land(RowKey::Down, Some(99)), Some(99));
    assert_eq!(land(RowKey::PageDown, Some(50)), Some(70));
    assert_eq!(land(RowKey::PageDown, Some(95)), Some(99));
    assert_eq!(land(RowKey::PageUp, None), Some(20));
    assert_eq!(land(RowKey::PageUp, Some(5)), Some(0));
    assert_eq!(land(RowKey::Home, Some(50)), Some(0));
    assert_eq!(land(RowKey::End, None), Some(99));
    assert_eq!(row_for_key(RowKey::Down, None, 0, 20, 0), None, "no rows");
}

fn press(h: &mut Harness, keys: &str) {
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let handle = app.focus_handle(cx);
        window.focus(&handle, cx);
    });
    h.cx.simulate_keystrokes(keys);
    h.cx.run_until_parked();
}

fn selected(h: &Harness) -> Option<usize> {
    h.view.read_with(&h.cx, |app, _| app.selected_row_pos)
}

#[gpui::test]
fn arrow_keys_work_without_a_selected_row(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(50));
    assert_eq!(selected(&h), None);
    press(&mut h, "down");
    let first = selected(&h).expect("down selects a row");
    press(&mut h, "down");
    assert_eq!(selected(&h), Some(first + 1));
    press(&mut h, "up");
    assert_eq!(selected(&h), Some(first));
}

#[gpui::test]
fn home_and_end_go_to_the_first_and_last_row(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(50));
    press(&mut h, "end");
    assert_eq!(selected(&h), Some(49));
    press(&mut h, "home");
    assert_eq!(selected(&h), Some(0));
}

#[gpui::test]
fn end_turns_follow_back_on(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_live_lines(cx, &common::sample_lines(50));
    h.view.update(&mut h.cx, |app, cx| app.set_follow(false, cx));
    press(&mut h, "end");
    assert!(h.view.read_with(&h.cx, |app, _| app.live.follow));
}

#[gpui::test]
fn row_keys_do_nothing_while_typing_in_search(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(50));
    h.view.update_in(&mut h.cx, |app, window, cx| app.activate_search(window, cx));
    h.cx.simulate_keystrokes("end");
    h.cx.run_until_parked();
    assert_eq!(selected(&h), None);
}
