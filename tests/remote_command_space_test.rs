use log_gpui::remote::PresetFields;
use log_gpui::workspace::log_space::{AddCommandError, LogFile, LogSpace, LogSpaceKind, RemoteRef, RemoteTarget};
use serde_json::json;

fn remote_space() -> LogSpace {
    LogSpace::new("s1".into(), "Checkout".into(), LogSpaceKind::Remote)
}

fn command(server: &str, text: &str) -> (String, String, String, Option<PresetFields>) {
    (server.to_string(), "web1".to_string(), text.to_string(), None)
}

#[test]
fn a_remote_command_is_named_after_its_server_and_command() {
    let file = LogFile::remote_command("srv-1", "web1", "journalctl -f -u nginx", None);
    assert_eq!(file.display_name, "web1$ journalctl -f -u nginx");
    assert!(file.file_id.starts_with("remote-cmd:srv-1:"));
    assert!(file.is_remote_command());
    // The same command on another Server is another member.
    let other = LogFile::remote_command("srv-2", "web2", "journalctl -f -u nginx", None);
    assert_ne!(file.file_id, other.file_id);
}

#[test]
fn files_and_commands_live_together_in_a_remote_log_space() {
    let mut space = remote_space();
    space.add_remote_files(&[("srv-1".into(), "web1".into(), "/app/app.log".into())]).unwrap();
    let outcome = space.add_remote_commands(&[command("srv-1", "journalctl -f -u nginx")]).unwrap();
    assert_eq!(outcome.added.len(), 1);
    assert_eq!(space.files.len(), 2);
}

#[test]
fn the_same_command_on_the_same_server_is_added_once() {
    let mut space = remote_space();
    space.add_remote_commands(&[command("srv-1", "journalctl -f -u nginx")]).unwrap();
    let outcome = space.add_remote_commands(&[command("srv-1", "journalctl -f -u nginx")]).unwrap();
    assert!(outcome.added.is_empty());
    assert_eq!(outcome.skipped, vec!["web1$ journalctl -f -u nginx".to_string()]);
    assert_eq!(space.files.len(), 1);
}

#[test]
fn only_a_remote_log_space_takes_commands() {
    for kind in [LogSpaceKind::Imported, LogSpaceKind::Scratch] {
        let mut space = LogSpace::new("s".into(), "x".into(), kind);
        assert_eq!(space.add_remote_commands(&[command("srv-1", "date")]), Err(AddCommandError::WrongKind));
        assert!(space.files.is_empty());
    }
}

#[test]
fn a_command_that_elevates_is_refused() {
    let mut space = remote_space();
    let err = space.add_remote_commands(&[command("srv-1", "sudo -n journalctl -f")]).unwrap_err();
    assert!(matches!(err, AddCommandError::Elevation(e) if e.word == "sudo"));
    assert!(space.files.is_empty());
}

#[test]
fn the_preset_is_kept_only_for_an_unedited_command() {
    let fields = PresetFields::journal("nginx");
    let mut space = remote_space();
    space
        .add_remote_commands(&[
            ("srv-1".into(), "web1".into(), fields.command(), Some(fields.clone())),
            ("srv-1".into(), "web1".into(), fields.command().replace("-n 1000", "-n 50"), Some(fields.clone())),
        ])
        .unwrap();
    let presets: Vec<Option<PresetFields>> = space
        .files
        .iter()
        .map(|f| match &f.remote.as_ref().unwrap().target {
            RemoteTarget::Command { preset, .. } => preset.clone(),
            RemoteTarget::File { .. } => panic!("a command"),
        })
        .collect();
    assert_eq!(presets, vec![Some(fields), None]);
}

#[test]
fn a_remote_file_stored_with_only_a_path_still_reads() {
    let old = json!({"server_id": "srv-1", "path": "/app/app.log"});
    let read: RemoteRef = serde_json::from_value(old).unwrap();
    assert_eq!(read, RemoteRef::file("srv-1", "/app/app.log"));
}

#[test]
fn a_space_with_files_and_commands_round_trips() {
    let mut space = remote_space();
    space.add_remote_files(&[("srv-1".into(), "web1".into(), "/app/app.log".into())]).unwrap();
    space.add_remote_commands(&[command("srv-1", "tail -F /tmp/x.log")]).unwrap();
    if let Some(RemoteTarget::Command { restart_on_exit, .. }) =
        space.files[1].remote.as_mut().map(|r| &mut r.target)
    {
        *restart_on_exit = true;
    }
    let json = serde_json::to_string(&space).unwrap();
    let back: LogSpace = serde_json::from_str(&json).unwrap();
    assert_eq!(back.files, space.files);
}

#[test]
fn a_member_that_cannot_be_read_is_skipped_not_fatal() {
    let mut space = remote_space();
    space.add_remote_files(&[("srv-1".into(), "web1".into(), "/app/app.log".into())]).unwrap();
    let mut value = serde_json::to_value(&space).unwrap();
    value["files"].as_array_mut().unwrap().push(json!({
        "file_id": "remote-x:1",
        "display_name": "future member",
        "remote": {"server_id": "srv-1", "target": {"Socket": {"addr": "x"}}}
    }));
    let back: LogSpace = serde_json::from_value(value).unwrap();
    assert_eq!(back.files.len(), 1);
    assert_eq!(back.files[0].display_name, "web1:/app/app.log");
}

#[test]
fn markers_on_a_remote_command_do_not_come_back() {
    let mut space = remote_space();
    space.add_remote_commands(&[command("srv-1", "tail -F /tmp/x.log")]).unwrap();
    space.add_remote_files(&[("srv-1".into(), "web1".into(), "/app/app.log".into())]).unwrap();
    for file in &mut space.files {
        file.paint_marker(1, "hello", 0..5, log_gpui::palette::HighlightColor::Custom("#ffff00".into()));
    }
    assert!(space.files.iter().all(|f| f.markers.len() == 1));
    let back: LogSpace = serde_json::from_str(&serde_json::to_string(&space).unwrap()).unwrap();
    let kept: Vec<(bool, usize)> = back.files.iter().map(|f| (f.is_remote_command(), f.markers.len())).collect();
    assert_eq!(kept, vec![(true, 0), (false, 1)]);
}
