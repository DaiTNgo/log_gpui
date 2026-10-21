//! The Keymap: default Shortcuts, changing them, and `keymap.json`.

use log_gpui::keymap::{AssignError, Command, Keymap, Shortcut};

fn sc(text: &str) -> Shortcut {
    Shortcut::parse(text).unwrap()
}

#[test]
fn no_two_commands_share_a_default_shortcut() {
    let keymap = Keymap::default();
    let mut seen = std::collections::HashMap::new();
    for command in Command::ALL {
        for shortcut in keymap.shortcuts(command) {
            assert!(!shortcut.is_fixed(), "{} takes a Fixed Key {}", command.name(), shortcut.text());
            if let Some(other) = seen.insert(shortcut.text(), command) {
                panic!("{} is the default of both {} and {}", shortcut.text(), other.name(), command.name());
            }
        }
    }
}

#[test]
fn every_command_has_its_own_id() {
    for command in Command::ALL {
        assert_eq!(Command::from_id(command.id()), Some(command));
    }
}

#[test]
fn shortcuts_read_and_write_the_same() {
    for text in ["cmd-f", "cmd-shift-up", "shift-enter", "cmd-alt-b", "cmd-,", "cmd--", "f5"] {
        assert_eq!(sc(text).text(), text);
    }
    assert_eq!(sc("ctrl-shift-F"), sc("cmd-shift-f"), "Ctrl is read as Cmd");
    assert_eq!(sc("Enter"), sc("enter"));
    assert!(Shortcut::parse("hyper-f").is_none());
    assert!(Shortcut::parse("").is_none());
}

#[test]
fn a_key_press_reads_as_its_shortcut() {
    let stroke = gpui::Keystroke::parse("ctrl-shift-g").unwrap();
    assert_eq!(Shortcut::from_keystroke(&stroke), sc("cmd-shift-g"));
    let stroke = gpui::Keystroke::parse("cmd-shift-G").unwrap();
    assert_eq!(Shortcut::from_keystroke(&stroke), sc("cmd-shift-g"));
}

#[test]
fn adding_a_shortcut_moves_it_from_the_command_that_had_it() {
    let mut keymap = Keymap::default();
    assert_eq!(keymap.command_for(&sc("cmd-f")), Some(Command::ToggleSearch));
    let moved = keymap.add(Command::ToggleFilter, sc("cmd-f")).unwrap();
    assert_eq!(moved, Some(Command::ToggleSearch));
    assert_eq!(keymap.command_for(&sc("cmd-f")), Some(Command::ToggleFilter));
    assert!(keymap.shortcuts(Command::ToggleSearch).is_empty());
    assert_eq!(keymap.shortcuts(Command::ToggleFilter), vec![sc("cmd-shift-f"), sc("cmd-f")]);
}

#[test]
fn fixed_keys_cannot_be_taken() {
    let mut keymap = Keymap::default();
    for text in ["escape", "up", "cmd-c", "cmd-3", "cmd-tab", "cmd-shift-tab"] {
        assert_eq!(keymap.add(Command::ToggleSearch, sc(text)), Err(AssignError::Fixed), "{text}");
    }
    assert!(keymap.is_default(Command::ToggleSearch));
}

#[test]
fn removing_and_resetting() {
    let mut keymap = Keymap::default();
    keymap.remove(Command::NextMatch, &sc("enter"));
    assert_eq!(keymap.shortcuts(Command::NextMatch), vec![sc("cmd-g")]);
    assert_eq!(keymap.command_for(&sc("enter")), None);
    keymap.add(Command::ToggleFollow, sc("enter")).unwrap();
    keymap.reset(Command::NextMatch);
    assert_eq!(keymap.command_for(&sc("enter")), Some(Command::NextMatch), "reset takes its default back");
    assert!(keymap.shortcuts(Command::ToggleFollow).is_empty());
    keymap.reset_all();
    assert!(!keymap.has_changes());
}

#[test]
fn putting_the_defaults_back_by_hand_is_no_change() {
    let mut keymap = Keymap::default();
    keymap.remove(Command::ToggleSearch, &sc("cmd-f"));
    keymap.add(Command::ToggleSearch, sc("cmd-f")).unwrap();
    assert!(keymap.is_default(Command::ToggleSearch));
}

#[test]
fn the_file_keeps_only_the_changed_commands() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keymap.json");
    assert_eq!(Keymap::load(&path), Ok(Keymap::default()), "no file is the defaults");

    let mut keymap = Keymap::default();
    keymap.add(Command::ToggleFollow, sc("cmd-shift-l")).unwrap();
    keymap.remove(Command::CloseTab, &sc("cmd-w"));
    keymap.save_to(&path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        json,
        serde_json::json!({ "shortcuts": { "live.follow": ["cmd-shift-l"], "tabs.close": [] } })
    );
    assert_eq!(Keymap::load(&path), Ok(keymap));
}

#[test]
fn unknown_commands_and_bad_shortcuts_in_the_file_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keymap.json");
    std::fs::write(
        &path,
        r#"{ "shortcuts": { "no.such": ["cmd-x"], "live.pause": ["hyper-p", "escape", "cmd-shift-p"] } }"#,
    )
    .unwrap();
    let keymap = Keymap::load(&path).unwrap();
    assert_eq!(keymap.shortcuts(Command::TogglePause), vec![sc("cmd-shift-p")]);

    std::fs::write(&path, "not json").unwrap();
    assert!(Keymap::load(&path).is_err());
}
