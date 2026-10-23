//! Keys that are easy to hit by mistake do nothing.
mod common;

use gpui::{Focusable as _, TestAppContext};
use log_gpui::keymap::{Command, Keymap, Shortcut};

#[test]
fn clear_file_has_no_default_shortcut() {
    let keymap = Keymap::default();
    assert!(keymap.shortcuts(Command::ClearFile).is_empty());
    assert_eq!(keymap.command_for(&Shortcut::parse("cmd-shift-k").unwrap()), None);
    assert_eq!(keymap.command_for(&Shortcut::parse("space").unwrap()), None);
}

#[gpui::test]
fn cmd_shift_k_and_space_change_nothing(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    let state = |h: &common::Harness| {
        h.view.read_with(&h.cx, |app, _| (app.all_entries.len(), app.context_sidebar.visible, app.confirm.clone()))
    };
    let before = state(&h);
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let handle = app.focus_handle(cx);
        window.focus(&handle, cx);
    });
    h.cx.simulate_keystrokes("cmd-shift-k space");
    h.cx.run_until_parked();
    assert_eq!(state(&h), before);
}
