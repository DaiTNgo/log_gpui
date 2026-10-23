//! Switching tabs by keyboard: ⌃Tab / ⌃⇧Tab cycle, ⌘9 is the last tab.
mod common;

use gpui::{Focusable as _, TestAppContext};
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

fn three_tabs(cx: &mut TestAppContext) -> common::Harness {
    let mut h = common::open_app(cx);
    for name in ["one", "two", "three"] {
        let path = h.dir.path().join(format!("{name}.log"));
        std::fs::write(&path, common::sample_lines(3).join("\n")).unwrap();
        let source = ImportSource::Disk(path.to_string_lossy().into_owned());
        h.view.update(&mut h.cx, |app, cx| app.create_log_space(name, LogSpaceKind::Imported, &[source], cx));
        h.cx.run_until_parked();
    }
    h
}

fn press(h: &mut common::Harness, keys: &str) {
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let handle = app.focus_handle(cx);
        window.focus(&handle, cx);
    });
    h.cx.simulate_keystrokes(keys);
    h.cx.run_until_parked();
}

fn active(h: &common::Harness) -> Option<usize> {
    h.view.read_with(&h.cx, |app, _| app.workspace_state.active_project().unwrap().working_set.active_tab_index())
}

#[gpui::test]
fn ctrl_tab_cycles_with_wrap_around(cx: &mut TestAppContext) {
    let mut h = three_tabs(cx);
    press(&mut h, "cmd-1");
    assert_eq!(active(&h), Some(0));
    press(&mut h, "ctrl-tab");
    assert_eq!(active(&h), Some(1));
    press(&mut h, "ctrl-shift-tab");
    press(&mut h, "ctrl-shift-tab");
    assert_eq!(active(&h), Some(2), "wraps to the last tab");
    press(&mut h, "ctrl-tab");
    assert_eq!(active(&h), Some(0), "wraps to the first tab");
}

#[gpui::test]
fn cmd_9_is_the_last_tab(cx: &mut TestAppContext) {
    let mut h = three_tabs(cx);
    press(&mut h, "cmd-1");
    press(&mut h, "cmd-9");
    assert_eq!(active(&h), Some(2));
}
