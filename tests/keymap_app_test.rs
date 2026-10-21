//! Changing a Shortcut from the Keymap settings: the new keys run the
//! Command, the old ones stop, and the change is saved.
mod common;

use common::Harness;
use gpui::{Focusable as _, TestAppContext};
use log_gpui::keymap::{Command, Shortcut};

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

fn record(h: &mut Harness, command: Command, keys: &str) {
    h.view.update_in(&mut h.cx, |app, window, cx| app.start_recording_shortcut(command, window, cx));
    let stroke = gpui::Keystroke::parse(keys).unwrap();
    h.view.update(&mut h.cx, |app, cx| app.record_shortcut(&stroke, cx));
}

fn search_open(h: &Harness) -> bool {
    h.view.read_with(&h.cx, |app, _| app.focus.search().is_open)
}

#[gpui::test]
fn a_recorded_shortcut_runs_its_command_and_is_saved(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(5));
    record(&mut h, Command::ToggleSearch, "cmd-shift-l");
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.keymap_recording, None);
        assert_eq!(app.keymap.command_for(&Shortcut::parse("cmd-shift-l").unwrap()), Some(Command::ToggleSearch));
    });

    focus_app(&mut h);
    press(&mut h, "cmd-shift-l");
    assert!(search_open(&h), "the new shortcut opens the Search");

    let saved = std::fs::read_to_string(h.dir.path().join("keymap.json")).unwrap();
    assert!(saved.contains("cmd-shift-l"), "{saved}");
}

#[gpui::test]
fn a_removed_shortcut_does_nothing(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(5));
    h.view.update(&mut h.cx, |app, cx| {
        app.remove_shortcut(Command::ToggleSearch, &Shortcut::parse("cmd-f").unwrap(), cx)
    });
    focus_app(&mut h);
    press(&mut h, "cmd-f");
    assert!(!search_open(&h));
}

#[gpui::test]
fn taking_a_shortcut_says_which_command_lost_it(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    record(&mut h, Command::ToggleFollow, "cmd-f");
    h.view.read_with(&h.cx, |app, _| {
        let message = app.keymap_message.clone().unwrap_or_default();
        assert!(message.contains("Search: Find in Log"), "{message}");
        assert!(app.keymap.shortcuts(Command::ToggleSearch).is_empty());
    });
}

#[gpui::test]
fn escape_cancels_recording_and_fixed_keys_are_refused(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    record(&mut h, Command::ToggleFollow, "escape");
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.keymap_recording, None);
        assert!(app.keymap.is_default(Command::ToggleFollow));
    });
    record(&mut h, Command::ToggleFollow, "cmd-c");
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.keymap.is_default(Command::ToggleFollow));
        assert!(app.keymap_message.clone().unwrap_or_default().contains("cannot be changed"));
    });
}

#[gpui::test]
fn the_palette_shows_the_current_shortcut(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    record(&mut h, Command::ToggleFollow, "cmd-shift-l");
    let label = h.view.read_with(&h.cx, |app, _| {
        app.keymap.shortcuts(Command::ToggleFollow).first().map(log_gpui::app::shortcut_label)
    });
    let expected = if cfg!(target_os = "macos") { "⌘⇧L" } else { "Ctrl+Shift+L" };
    assert_eq!(label.as_deref(), Some(expected));
}
