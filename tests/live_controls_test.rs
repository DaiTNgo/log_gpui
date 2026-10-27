//! Follow, Pause and the connection label say what state they are in.
mod common;

use gpui::TestAppContext;
use log_gpui::keymap::{Command, Keymap, Shortcut};
use log_gpui::live::{SourceStatus, StatusTone};

fn controls(h: &common::Harness) -> (String, String) {
    h.view.read_with(&h.cx, |app, _| app.live_control_labels())
}

#[gpui::test]
fn follow_and_pause_read_their_state(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_live_lines(cx, &common::sample_lines(20));
    assert_eq!(controls(&h), ("Following".into(), "Pause".into()));

    h.view.update(&mut h.cx, |app, cx| app.toggle_follow(cx));
    assert_eq!(controls(&h).0, "Follow");

    h.view.update(&mut h.cx, |app, cx| app.toggle_pause(cx));
    assert_eq!(controls(&h).1, "Resume");
    h.view.update(&mut h.cx, |app, cx| app.toggle_pause(cx));
    assert_eq!(controls(&h).1, "Pause");
}

#[test]
fn each_connection_state_has_its_tone() {
    assert_eq!(SourceStatus::Live.tone(), StatusTone::Good);
    assert_eq!(SourceStatus::Connecting.tone(), StatusTone::Busy);
    assert_eq!(SourceStatus::WaitingForFile.tone(), StatusTone::Busy);
    assert_eq!(SourceStatus::Reconnecting.tone(), StatusTone::Warning);
    assert_eq!(SourceStatus::NeedsSecret.tone(), StatusTone::Warning);
    assert_eq!(SourceStatus::Disconnected.tone(), StatusTone::Bad);
    assert_eq!(SourceStatus::AuthFailed.tone(), StatusTone::Bad);
    assert_eq!(SourceStatus::Error("x".into()).tone(), StatusTone::Bad);
}

#[test]
fn follow_and_pause_have_default_shortcuts() {
    let keymap = Keymap::default();
    assert_eq!(keymap.command_for(&Shortcut::parse("cmd-alt-f").unwrap()), Some(Command::ToggleFollow));
    assert_eq!(keymap.command_for(&Shortcut::parse("cmd-alt-p").unwrap()), Some(Command::TogglePause));
}
