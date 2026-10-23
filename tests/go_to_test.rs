//! Go to Line: the typed line becomes the current line, in both views, and
//! nothing the user was looking for changes.
mod common;

use common::Harness;
use gpui::{Focusable as _, TestAppContext};
use log_gpui::focus::LineRef;

fn focus_app(h: &mut Harness) {
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let handle = app.focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    });
    h.cx.run_until_parked();
}

fn press(h: &mut Harness, keys: &str) {
    h.cx.simulate_keystrokes(keys);
    h.cx.run_until_parked();
}

/// `cmd-l`, type `text`, Enter.
fn go_to_line(h: &mut Harness, text: &str) {
    focus_app(h);
    press(h, "cmd-l");
    assert!(bar_open(h), "cmd-l opens the Go to bar");
    h.cx.simulate_input(text);
    h.cx.run_until_parked();
    press(h, "enter");
}

fn bar_open(h: &Harness) -> bool {
    h.view.read_with(&h.cx, |app, _| app.go_to.is_some())
}

fn current(h: &Harness) -> Option<LineRef> {
    h.view.read_with(&h.cx, |app, _| app.current_line())
}

#[gpui::test]
fn enter_lands_on_the_typed_line(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    go_to_line(&mut h, "12");
    assert!(!bar_open(&h), "the bar closes after the jump");
    assert_eq!(current(&h).map(|l| l.line), Some(12));
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.selected_entry().map(|e| e.index), Some(12));
        assert!(app.notice.is_none());
    });
}

#[gpui::test]
fn the_raw_view_lands_on_the_typed_line(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    h.view.update(&mut h.cx, |app, cx| app.toggle_view_mode(cx));
    h.cx.run_until_parked();
    go_to_line(&mut h, "7");
    assert_eq!(current(&h).map(|l| l.line), Some(7));
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.raw.selected.as_ref().map(|(_, line)| *line), Some(6));
    });
}

#[gpui::test]
fn past_the_end_lands_on_the_last_line_with_a_note(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    go_to_line(&mut h, "500");
    assert_eq!(current(&h).map(|l| l.line), Some(20));
    h.view.read_with(&h.cx, |app, _| {
        let notice = app.notice.clone().expect("a note says the file is shorter");
        assert!(notice.contains("20 lines"), "{notice}");
    });
}

#[gpui::test]
fn invalid_text_keeps_the_bar_open_and_does_not_move(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    go_to_line(&mut h, "4");
    go_to_line(&mut h, "abc");
    assert!(bar_open(&h), "the bar stays open to fix the text");
    h.view.read_with(&h.cx, |app, _| assert!(app.go_to.as_ref().unwrap().error.is_some()));
    assert_eq!(current(&h).map(|l| l.line), Some(4));
}

#[gpui::test]
fn escape_closes_without_moving(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    go_to_line(&mut h, "4");
    focus_app(&mut h);
    press(&mut h, "cmd-l");
    h.cx.simulate_input("15");
    h.cx.run_until_parked();
    press(&mut h, "escape");
    assert!(!bar_open(&h));
    assert_eq!(current(&h).map(|l| l.line), Some(4));
    press(&mut h, "cmd-l");
    assert!(bar_open(&h), "the keyboard is back on the view after Escape");
}

#[gpui::test]
fn a_jump_leaves_search_and_filter_alone(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    h.view.update_in(&mut h.cx, |app, window, cx| app.activate_search(window, cx));
    h.cx.simulate_input("line 1");
    h.cx.run_until_parked();
    let before = h.view.read_with(&h.cx, |app, _| (app.focus.search().query.clone(), app.filter_state.clone()));

    go_to_line(&mut h, "9");
    assert_eq!(current(&h).map(|l| l.line), Some(9));
    let after = h.view.read_with(&h.cx, |app, _| (app.focus.search().query.clone(), app.filter_state.clone()));
    assert_eq!(before, after);
}

#[gpui::test]
fn the_past_end_note_goes_with_the_next_jump(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(1500));
    go_to_line(&mut h, "99999");
    h.view.read_with(&h.cx, |app, _| {
        let notice = app.notice.clone().expect("a note says the file is shorter");
        assert!(notice.contains("1,500 lines"), "the count is grouped like the status bar: {notice}");
    });
    go_to_line(&mut h, "3");
    h.view.read_with(&h.cx, |app, _| assert!(app.notice.is_none(), "the old note is dropped"));
}

#[gpui::test]
fn a_jump_keeps_a_notice_it_did_not_leave(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    h.view.update(&mut h.cx, |app, _| app.notice = Some("Open in full: stopped".into()));
    go_to_line(&mut h, "5");
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.notice.as_deref(), Some("Open in full: stopped")));
}
