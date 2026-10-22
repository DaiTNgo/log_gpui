use log_gpui::workspace::log_space::{
    FileOrder, LogFile, LogSpace, LogSpaceKind, RemoveFileError, SpaceNameError, WrongKind,
    validate_space_name,
};
use log_gpui::workspace::project::{Project, ProjectColorAccent};
use log_gpui::workspace::working_set::TabSessionState;
use std::path::Path;

fn project() -> Project {
    Project::new("prod".into(), "Prod".into(), ProjectColorAccent::Blue)
}

fn imported(paths: &[&str]) -> LogSpace {
    let mut space = LogSpace::new("s".into(), "Payment".into(), LogSpaceKind::Imported);
    let paths: Vec<String> = paths.iter().map(|p| p.to_string()).collect();
    space.add_imported_files(&paths).unwrap();
    space
}

// ─── Names ───────────────────────────────────────────────────────────────────

#[test]
fn blank_names_are_refused() {
    let mut p = project();
    assert_eq!(p.create_log_space("   ", LogSpaceKind::Scratch).err(), Some(SpaceNameError::Blank));
    assert_eq!(p.create_log_space("", LogSpaceKind::Scratch).err(), Some(SpaceNameError::Blank));
    assert!(p.log_spaces.is_empty());
}

#[test]
fn duplicate_names_are_refused_after_trimming() {
    let mut p = project();
    p.create_log_space("Payment", LogSpaceKind::Scratch).unwrap();
    assert!(p.has_space_named(" Payment "));
    assert_eq!(
        p.create_log_space(" Payment ", LogSpaceKind::Imported).err(),
        Some(SpaceNameError::Duplicate)
    );
    assert_eq!(p.log_spaces.len(), 1);
}

#[test]
fn the_same_name_is_allowed_in_another_project() {
    let mut prod = project();
    let mut staging = Project::new("staging".into(), "Staging".into(), ProjectColorAccent::Green);
    prod.create_log_space("Payment", LogSpaceKind::Scratch).unwrap();
    assert!(staging.create_log_space("Payment", LogSpaceKind::Scratch).is_ok());
    // Their ids differ, so their scratch folders never collide.
    assert_ne!(prod.log_spaces[0].id, staging.log_spaces[0].id);
}

#[test]
fn renaming_to_an_existing_name_is_refused_but_keeping_the_name_is_fine() {
    let mut p = project();
    p.create_log_space("Auth", LogSpaceKind::Scratch).unwrap();
    let payment = p.create_log_space("Payment", LogSpaceKind::Scratch).unwrap().id.clone();
    let auth = p.log_spaces[0].id.clone();

    assert_eq!(p.rename_log_space(&auth, "Payment"), Err(SpaceNameError::Duplicate));
    assert_eq!(p.log_space(&auth).unwrap().name, "Auth");
    assert_eq!(p.rename_log_space(&payment, "Payment"), Ok(()));
    assert_eq!(p.rename_log_space(&auth, " Identity "), Ok(()));
    assert_eq!(p.log_space(&auth).unwrap().name, "Identity");
}

#[test]
fn validate_space_name_trims() {
    assert_eq!(validate_space_name("  x ", ["y"]), Ok("x".to_string()));
}

// ─── Kinds never mix ─────────────────────────────────────────────────────────

#[test]
fn imported_spaces_only_gain_imported_files() {
    let mut space = imported(&["/a.log"]);
    assert_eq!(space.new_scratch_file(Path::new("/tmp/x")).err(), Some(WrongKind));
    assert_eq!(space.files.len(), 1);
}

#[test]
fn scratch_spaces_only_gain_scratch_files() {
    let mut space = LogSpace::new("s".into(), "S".into(), LogSpaceKind::Scratch);
    assert_eq!(space.add_imported_files(&["/a.log".to_string()]).err(), Some(WrongKind));
    assert!(space.files.is_empty());
}

#[test]
fn importing_a_known_path_is_skipped_and_reported() {
    let mut space = imported(&["/var/log/api.log"]);
    let outcome = space
        .add_imported_files(&["/var/log/api.log".to_string(), "/var/log/db.log".to_string()])
        .unwrap();
    assert_eq!(outcome.skipped, vec!["/var/log/api.log".to_string()]);
    assert_eq!(outcome.added, vec!["/var/log/db.log".to_string()]);
    assert_eq!(space.files.len(), 2);
}

// ─── Scratch files ───────────────────────────────────────────────────────────

#[test]
fn scratch_files_are_named_with_the_smallest_unused_number() {
    let mut space = LogSpace::new("s".into(), "S".into(), LogSpaceKind::Scratch);
    let dir = Path::new("/tmp/spaces/s");
    let names: Vec<String> = (0..3)
        .map(|_| space.new_scratch_file(dir).unwrap().display_name.clone())
        .collect();
    assert_eq!(names, ["untitled.log", "untitled-2.log", "untitled-3.log"]);

    // Freeing a number reuses it.
    let second = space.files[1].file_id.clone();
    space.remove_file(&second).unwrap();
    assert_eq!(space.new_scratch_file(dir).unwrap().display_name, "untitled-2.log");
    let expected = dir.join("untitled-2.log");
    assert_eq!(space.files.last().unwrap().path.as_deref(), expected.to_str());
}

#[test]
fn the_last_scratch_file_cannot_be_removed() {
    let mut space = LogSpace::new("s".into(), "S".into(), LogSpaceKind::Scratch);
    let id = space.new_scratch_file(Path::new("/tmp/x")).unwrap().file_id.clone();
    assert_eq!(space.remove_file(&id), Err(RemoveFileError::LastScratchFile));
    assert_eq!(space.files.len(), 1);
}

#[test]
fn the_last_imported_file_can_be_removed() {
    let mut space = imported(&["/a.log"]);
    assert!(space.remove_file("/a.log").is_ok());
    assert!(space.files.is_empty());
    assert_eq!(space.remove_file("/a.log"), Err(RemoveFileError::NotFound));
}

#[test]
fn removing_a_file_drops_its_markers_and_file_view_filter() {
    let mut space = imported(&["/a.log", "/b.log"]);
    let mut filter = space.file_filter("/a.log");
    filter.toggle_value("level", "ERROR");
    space.set_file_filter("/a.log", filter);

    let removed = space.remove_file("/a.log").unwrap();
    assert_eq!(removed.file_id, "/a.log");
    assert!(space.file_filter("/a.log").is_empty());
}

#[test]
fn renaming_a_scratch_file_refuses_blank_and_taken_names() {
    let mut space = LogSpace::new("s".into(), "S".into(), LogSpaceKind::Scratch);
    let dir = Path::new("/tmp/x");
    let first = space.new_scratch_file(dir).unwrap().file_id.clone();
    space.new_scratch_file(dir).unwrap();

    assert!(!space.rename_file(&first, "  "));
    assert!(!space.rename_file(&first, "untitled-2.log"));
    assert!(space.rename_file(&first, "gateway.log"));
    assert_eq!(space.file(&first).unwrap().display_name, "gateway.log");
}

// ─── File order ──────────────────────────────────────────────────────────────

#[test]
fn files_are_ordered_by_when_they_were_added_or_by_name() {
    let mut space = imported(&["/logs/worker.log", "/logs/api.log"]);
    let names = |s: &LogSpace| -> Vec<String> {
        s.ordered_files().iter().map(|f| f.display_name.clone()).collect()
    };
    assert_eq!(names(&space), ["worker.log", "api.log"]);

    space.set_file_order(FileOrder::Name);
    assert_eq!(names(&space), ["api.log", "worker.log"]);

    // Files added later come last in the added order.
    space.add_imported_files(&["/logs/auth.log".to_string()]).unwrap();
    space.set_file_order(FileOrder::Added);
    assert_eq!(names(&space), ["worker.log", "api.log", "auth.log"]);
}

#[test]
fn the_source_column_defaults_to_two_or_more_files() {
    let mut space = imported(&["/a.log"]);
    assert!(!space.source_column_shown());
    space.add_imported_files(&["/b.log".to_string()]).unwrap();
    assert!(space.source_column_shown());
    space.show_source = Some(false);
    assert!(!space.source_column_shown());
}

// ─── Tabs ────────────────────────────────────────────────────────────────────

#[test]
fn opening_a_view_twice_focuses_the_existing_tab() {
    let mut p = project();
    let id = p.create_log_space("Payment", LogSpaceKind::Imported).unwrap().id.clone();
    p.log_space_mut(&id).unwrap().add_imported_files(&["/a.log".to_string()]).unwrap();

    assert!(p.open_log_space(&id, TabSessionState::new()).is_some());
    assert!(p.open_log_space(&id, TabSessionState::new()).is_none());
    assert_eq!(p.working_set.tab_count(), 1);

    // The Log Space and a File View of it are two tabs.
    assert!(p.open_file_view(&id, "/a.log", TabSessionState::new()).is_some());
    assert_eq!(p.working_set.tab_count(), 2);
    assert!(p.open_file_view(&id, "/a.log", TabSessionState::new()).is_none());
    assert_eq!(p.working_set.tab_count(), 2);
}

#[test]
fn removing_a_file_closes_its_file_view() {
    let mut p = project();
    let id = p.create_log_space("Payment", LogSpaceKind::Imported).unwrap().id.clone();
    p.log_space_mut(&id)
        .unwrap()
        .add_imported_files(&["/a.log".to_string(), "/b.log".to_string()])
        .unwrap();
    p.open_log_space(&id, TabSessionState::new());
    p.open_file_view(&id, "/a.log", TabSessionState::new());

    p.remove_file(&id, "/a.log").unwrap();

    assert_eq!(p.working_set.tab_count(), 1);
    assert!(!p.working_set.is_file_open("/a.log"));
}

// ─── Markers keep the hash of their line ─────────────────────────────────────

fn red() -> log_gpui::palette::HighlightColor {
    log_gpui::palette::HighlightColor::palette(log_gpui::palette::Hue::Red, log_gpui::palette::Shade::Medium)
}

#[test]
fn editing_a_line_drops_its_marker_but_keeps_the_others() {
    let mut file = LogFile::new("f".into(), "f.log".into());
    for (line, text) in [(1, "alpha"), (2, "beta"), (3, "gamma")] {
        assert!(file.paint_marker(line, text, 0..2, red()));
    }

    // Line 2 was edited and a line was appended.
    let dropped = file.prune_markers(&["alpha", "beta edited", "gamma", "delta"]);

    assert_eq!(dropped, 1);
    let lines: Vec<usize> = file.markers.iter().map(|m| m.line).collect();
    assert_eq!(lines, vec![1, 3]);
}

#[test]
fn inserting_a_line_above_drops_the_markers_below() {
    let mut file = LogFile::new("f".into(), "f.log".into());
    file.paint_marker(2, "beta", 0..2, red());
    let dropped = file.prune_markers(&["alpha", "new", "beta"]);
    assert_eq!(dropped, 1);
    assert!(file.markers.is_empty());
}

#[test]
fn a_marker_past_the_end_of_the_file_is_dropped() {
    let mut file = LogFile::new("f".into(), "f.log".into());
    file.paint_marker(5, "x", 0..1, red());
    assert_eq!(file.prune_markers(&["a"]), 1);
}

#[test]
fn a_line_with_several_markers_loses_them_all_when_edited() {
    let mut file = LogFile::new("f".into(), "f.log".into());
    file.paint_marker(1, "alpha beta", 0..5, red());
    file.paint_marker(1, "alpha beta", 6..10, red());
    assert_eq!(file.markers.len(), 2);
    assert_eq!(file.prune_markers(&["alpha beta!"]), 2);
}


// ─── Live Globs ──────────────────────────────────────────────────────────────

fn remote_space() -> LogSpace {
    LogSpace::new("r".into(), "Prod".into(), LogSpaceKind::Remote)
}

fn remote(sid: &str, path: &str) -> (String, String, String) {
    (sid.to_string(), "web1".to_string(), path.to_string())
}

#[test]
fn a_live_glob_is_added_once_per_server_and_pattern() {
    use log_gpui::workspace::log_space::AddGlobError;
    let mut space = remote_space();
    let id = space.add_live_glob("s1", "/var/log/app", "*.log").unwrap();
    assert_eq!(space.glob(&id).unwrap().full_pattern(), "/var/log/app/*.log");
    assert_eq!(space.add_live_glob("s1", "/var/log/app", "*.log"), Err(AddGlobError::Duplicate));
    assert!(space.add_live_glob("s2", "/var/log/app", "*.log").is_ok());
    assert!(space.add_live_glob("s1", "/var/log/app", "*.txt").is_ok());
    let mut imported = imported(&[]);
    assert_eq!(imported.add_live_glob("s1", "/x", "*"), Err(AddGlobError::WrongKind));
}

#[test]
fn files_a_glob_found_keep_their_first_origin() {
    let mut space = remote_space();
    space.add_remote_files(&[remote("s1", "/var/log/app/a.log")]).unwrap();
    let id = space.add_live_glob("s1", "/var/log/app", "*.log").unwrap();
    let paths = vec!["/var/log/app/a.log".to_string(), "/var/log/app/b.log".to_string()];
    let added = space.add_glob_files(&id, "web1", &paths);
    assert_eq!(added.added, vec!["remote:s1:/var/log/app/b.log".to_string()]);
    let a = space.file("remote:s1:/var/log/app/a.log").unwrap();
    assert_eq!(a.from_glob, None);
    assert_eq!(space.file("remote:s1:/var/log/app/b.log").unwrap().from_glob.as_deref(), Some(id.as_str()));
}

#[test]
fn removing_a_glob_file_excludes_its_path_and_adding_it_again_restores_it() {
    let mut space = remote_space();
    let id = space.add_live_glob("s1", "/var/log/app", "*.log").unwrap();
    space.add_glob_files(&id, "web1", &["/var/log/app/debug.log".to_string()]);
    space.remove_file("remote:s1:/var/log/app/debug.log").unwrap();
    assert_eq!(space.glob(&id).unwrap().excluded, vec!["/var/log/app/debug.log".to_string()]);
    // The same path on another Server is not the glob's.
    let outcome = space.add_remote_files(&[remote("s2", "/var/log/app/debug.log")]).unwrap();
    assert_eq!(outcome.added.len(), 1);
    assert_eq!(space.glob(&id).unwrap().excluded.len(), 1);
    // The exact path on the same Server comes back into the group.
    space.add_remote_files(&[remote("s1", "/var/log/app/debug.log")]).unwrap();
    assert!(space.glob(&id).unwrap().excluded.is_empty());
    assert_eq!(space.file("remote:s1:/var/log/app/debug.log").unwrap().from_glob.as_deref(), Some(id.as_str()));
}

#[test]
fn removing_a_glob_keeps_files_added_by_path() {
    let mut space = remote_space();
    space.add_remote_files(&[remote("s1", "/var/log/app/a.log")]).unwrap();
    let id = space.add_live_glob("s1", "/var/log/app", "*.log").unwrap();
    space.add_glob_files(&id, "web1", &["/var/log/app/a.log".to_string(), "/var/log/app/b.log".to_string()]);
    let removed = space.remove_live_glob(&id);
    assert_eq!(removed.len(), 1);
    assert!(space.has_file("remote:s1:/var/log/app/a.log"));
    assert!(!space.has_file("remote:s1:/var/log/app/b.log"));
    assert!(space.globs.is_empty());
}

#[test]
fn stopping_a_glob_is_remembered_in_the_project_file() {
    let mut space = remote_space();
    let id = space.add_live_glob("s1", "/var/log/app", "*.log").unwrap();
    space.add_glob_files(&id, "web1", &["/var/log/app/a.log".to_string()]);
    space.set_glob_stopped(&id, true);
    let json = serde_json::to_string(&space).unwrap();
    let back: LogSpace = serde_json::from_str(&json).unwrap();
    assert!(back.glob(&id).unwrap().stopped);
    assert_eq!(back.glob_files(&id).len(), 1);
}

#[test]
fn an_old_space_without_globs_still_loads() {
    let old = r#"{"id":"r","name":"Prod","kind":"Remote","files":[{"file_id":"remote:s1:/a.log","display_name":"web1:/a.log","path":null,"remote":{"server_id":"s1","path":"/a.log"}}]}"#;
    let space: LogSpace = serde_json::from_str(old).unwrap();
    assert!(space.globs.is_empty());
    assert_eq!(space.files[0].from_glob, None);
}

#[test]
fn markers_keep_their_distance_when_a_rotated_file_is_loaded_above() {
    use log_gpui::workspace::log_space::LogFile;
    let mut file = LogFile::remote("s1", "web1", "/app/app.log");
    // A Marker on line 5 of the file being read (distance 0).
    assert!(file.paint_marker(5, "line five", 0..4, red()));
    // The file rotated once while it was open: that Marker is now 1 back.
    file.begin_generation();
    assert_eq!(file.markers_on_line_in(5, 1).len(), 1);
    // A Rotated File is loaded above: lines read in the old Generation 0 are
    // now 2 back, the current lines 0 back, and the Marker stays on its line.
    file.begin_earlier_generation();
    assert_eq!(file.generations_ago(2), 0);
    assert_eq!(file.generations_ago(1), 1);
    assert_eq!(file.generations_ago(0), 2);
    assert_eq!(file.markers_on_line_in(5, 1).len(), 1);
    // A Marker on a line of the Rotated File is stored 2 back, and found
    // again when the file is reopened and loads the same Generations.
    assert!(file.paint_marker_in(2, 120, "an older line", 0..2, red()));
    assert_eq!(file.markers_on_line_in(120, 2).len(), 1);
    assert!(file.markers_on_line_in(120, 0).is_empty());
}

#[test]
fn a_starting_time_is_remembered_with_the_remote_file_and_old_sessions_have_none() {
    use log_gpui::workspace::log_space::LogFile;
    let mut file = LogFile::remote("s1", "web1", "/app/app.log");
    assert!(file.remote.as_ref().unwrap().start_time.is_none());
    file.set_start_time(Some("1h".into()));
    let json = serde_json::to_string(&file).unwrap();
    let back: LogFile = serde_json::from_str(&json).unwrap();
    assert_eq!(back.remote.unwrap().start_time.as_deref(), Some("1h"));
    // A blank one is no Starting Time, and the old form has none.
    file.set_start_time(Some("  ".into()));
    assert!(file.remote.as_ref().unwrap().start_time.is_none());
    let old = r#"{"server_id":"s1","path":"/app/app.log"}"#;
    let remote: log_gpui::workspace::log_space::RemoteRef = serde_json::from_str(old).unwrap();
    assert!(remote.start_time.is_none());
}

// ─── Editing Remote Files, Live Globs and Remote Commands ───────────────────

mod edit {
    use super::*;
    use log_gpui::workspace::log_space::{EditError, GlobEditPlan, RemoteTarget};

    fn space() -> LogSpace {
        LogSpace::new("r".into(), "Checkout".into(), LogSpaceKind::Remote)
    }

    fn file_target(path: &str) -> RemoteTarget {
        RemoteTarget::File { path: path.into() }
    }

    fn add(space: &mut LogSpace, path: &str) {
        space.add_remote_files(&[("s1".into(), "web1".into(), path.into())]).unwrap();
    }

    #[test]
    fn replacing_the_path_keeps_the_id_the_place_and_the_order() {
        let mut space = space();
        add(&mut space, "/app/first.log");
        add(&mut space, "/app/ap.log");
        add(&mut space, "/app/third.log");
        let seq = space.file("remote:s1:/app/ap.log").unwrap().added_seq;
        space.file_mut("remote:s1:/app/ap.log").unwrap().paint_marker(1, "x", 0..1, super::red());
        space.replace_remote_target("remote:s1:/app/ap.log", "s1", "web1", file_target("/app/app.log")).unwrap();
        let ordered: Vec<&str> = space.ordered_files().iter().map(|f| f.display_name.as_str()).collect();
        assert_eq!(ordered, ["web1:/app/first.log", "web1:/app/app.log", "web1:/app/third.log"]);
        let file = space.file("remote:s1:/app/ap.log").unwrap();
        assert_eq!(file.added_seq, seq);
        assert_eq!(file.remote.as_ref().unwrap().path(), Some("/app/app.log"));
        assert!(file.markers.is_empty());
        assert_eq!(file.generation, 0);
    }

    #[test]
    fn replacing_the_server_renames_the_file_after_the_new_server() {
        let mut space = space();
        add(&mut space, "/app/app.log");
        space.replace_remote_target("remote:s1:/app/app.log", "s2", "web2", file_target("/app/app.log")).unwrap();
        let file = space.file("remote:s1:/app/app.log").unwrap();
        assert_eq!(file.display_name, "web2:/app/app.log");
        assert_eq!(file.remote.as_ref().unwrap().server_id, "s2");
    }

    #[test]
    fn replacing_with_what_another_member_reads_is_refused_and_changes_nothing() {
        let mut space = space();
        add(&mut space, "/app/a.log");
        add(&mut space, "/app/b.log");
        let before = space.clone().files;
        let result = space.replace_remote_target("remote:s1:/app/a.log", "s1", "web1", file_target("/app/b.log"));
        assert_eq!(result, Err(EditError::Duplicate));
        assert_eq!(space.files, before);
        // Its own target is not a duplicate of itself.
        assert!(space.replace_remote_target("remote:s1:/app/a.log", "s1", "web1", file_target("/app/a.log")).is_ok());
    }

    #[test]
    fn an_edited_file_does_not_make_its_old_path_a_duplicate() {
        let mut space = space();
        add(&mut space, "/app/ap.log");
        space.replace_remote_target("remote:s1:/app/ap.log", "s1", "web1", file_target("/app/app.log")).unwrap();
        // The old path can be added again: it is another member, with its own id.
        let outcome = space.add_remote_files(&[("s1".into(), "web1".into(), "/app/ap.log".into())]).unwrap();
        assert_eq!(outcome.added.len(), 1);
        assert_ne!(outcome.added[0], "remote:s1:/app/ap.log");
        assert_eq!(space.remote_file_id("s1", "/app/ap.log").as_deref(), Some(outcome.added[0].as_str()));
        assert_eq!(space.remote_file_id("s1", "/app/app.log").as_deref(), Some("remote:s1:/app/ap.log"));
        // And the edited file's new path is a duplicate.
        let again = space.add_remote_files(&[("s1".into(), "web1".into(), "/app/app.log".into())]).unwrap();
        assert!(again.added.is_empty());
        assert_eq!(again.skipped.len(), 1);
    }

    #[test]
    fn a_file_cannot_become_a_command() {
        let mut space = space();
        add(&mut space, "/app/a.log");
        let command = RemoteTarget::Command { command: "ls".into(), preset: None, restart_on_exit: false };
        assert_eq!(space.replace_remote_target("remote:s1:/app/a.log", "s1", "web1", command), Err(EditError::NotFound));
    }

    #[test]
    fn replacing_a_command_keeps_markers_restart_on_exit_and_the_place() {
        let mut space = space();
        space.add_remote_commands(&[("s1".into(), "web1".into(), "journalctl -f -u nginx".into(), None)]).unwrap();
        let id = space.files[0].file_id.clone();
        if let Some(RemoteTarget::Command { restart_on_exit, .. }) = space.file_mut(&id).unwrap().remote.as_mut().map(|r| &mut r.target) {
            *restart_on_exit = true;
        }
        space.file_mut(&id).unwrap().paint_marker(3, "a line", 0..1, super::red());
        let command = RemoteTarget::Command { command: "journalctl -f -u api".into(), preset: None, restart_on_exit: false };
        space.replace_remote_target(&id, "s2", "web2", command).unwrap();
        let file = space.file(&id).unwrap();
        assert_eq!(file.display_name, "web2$ journalctl -f -u api");
        assert_eq!(file.markers.len(), 1);
        match &file.remote.as_ref().unwrap().target {
            RemoteTarget::Command { command, restart_on_exit, .. } => {
                assert_eq!(command, "journalctl -f -u api");
                assert!(*restart_on_exit, "Restart on exit keeps its value");
            }
            _ => panic!("still a command"),
        }
    }

    #[test]
    fn a_command_that_elevates_is_refused() {
        let mut space = space();
        space.add_remote_commands(&[("s1".into(), "web1".into(), "tail -f /x".into(), None)]).unwrap();
        let id = space.files[0].file_id.clone();
        let command = RemoteTarget::Command { command: "sudo tail -f /x".into(), preset: None, restart_on_exit: false };
        assert!(matches!(space.replace_remote_target(&id, "s1", "web1", command), Err(EditError::Elevation(_))));
        assert_eq!(space.files[0].display_name, "web1$ tail -f /x");
    }

    #[test]
    fn the_starting_time_changes_without_touching_markers() {
        let mut space = space();
        add(&mut space, "/app/app.log");
        let id = "remote:s1:/app/app.log";
        space.file_mut(id).unwrap().paint_marker(40_000, "x", 0..1, super::red());
        assert!(space.set_start_time(id, Some("1h".into())));
        assert!(space.set_start_time(id, Some("3h".into())));
        let file = space.file(id).unwrap();
        assert_eq!(file.remote.as_ref().unwrap().start_time.as_deref(), Some("3h"));
        assert_eq!(file.markers.len(), 1);
        assert!(space.set_start_time(id, Some("  ".into())));
        assert!(space.file(id).unwrap().remote.as_ref().unwrap().start_time.is_none());
        assert!(!space.set_start_time("nope", None));
    }

    fn plan(add: &[&str], remove: &[&str], keep: &[&str]) -> GlobEditPlan {
        let owned = |v: &[&str]| v.iter().map(|p| format!("/var/log/app/{p}")).collect();
        GlobEditPlan { add: owned(add), remove: owned(remove), keep: owned(keep) }
    }

    fn glob_space() -> (LogSpace, String) {
        let mut space = space();
        let id = space.add_live_glob("s1", "/var/log/app", "*.log").unwrap();
        let paths: Vec<String> = ["app-1.log", "app-2.log", "worker.log"].iter().map(|n| format!("/var/log/app/{n}")).collect();
        space.add_glob_files(&id, "web1", &paths);
        (space, id)
    }

    #[test]
    fn editing_a_glob_keeps_its_id_and_the_files_that_stay() {
        let (mut space, id) = glob_space();
        space.file_mut("remote:s1:/var/log/app/app-1.log").unwrap().paint_marker(1, "x", 0..1, super::red());
        let removed = space
            .edit_live_glob(&id, "s1", "web1", "/var/log/app", "app-*.log", &plan(&["app-3.log"], &["worker.log"], &["app-1.log", "app-2.log"]))
            .unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].display_name, "web1:/var/log/app/worker.log");
        assert_eq!(space.glob(&id).unwrap().pattern, "app-*.log");
        let names: Vec<&str> = space.glob_files(&id).iter().map(|f| f.display_name.as_str()).collect();
        assert_eq!(names, ["web1:/var/log/app/app-1.log", "web1:/var/log/app/app-2.log", "web1:/var/log/app/app-3.log"]);
        assert_eq!(space.file("remote:s1:/var/log/app/app-1.log").unwrap().markers.len(), 1);
    }

    #[test]
    fn editing_a_stopped_glob_removes_but_adds_nothing() {
        let (mut space, id) = glob_space();
        space.set_glob_stopped(&id, true);
        space.edit_live_glob(&id, "s1", "web1", "/var/log/app", "app-*.log", &plan(&["app-3.log"], &["worker.log"], &["app-1.log", "app-2.log"])).unwrap();
        assert!(space.glob(&id).unwrap().stopped);
        assert_eq!(space.glob_files(&id).len(), 2);
    }

    #[test]
    fn editing_a_glob_forgets_the_exclusions_it_no_longer_matches() {
        let (mut space, id) = glob_space();
        space.remove_file("remote:s1:/var/log/app/worker.log").unwrap();
        space.glob_mut(&id).unwrap().excluded.push("/var/log/app/app-9.log".into());
        assert_eq!(space.glob(&id).unwrap().excluded.len(), 2);
        space.edit_live_glob(&id, "s1", "web1", "/var/log/app", "app-*.log", &plan(&[], &[], &["app-1.log", "app-2.log"])).unwrap();
        assert_eq!(space.glob(&id).unwrap().excluded, ["/var/log/app/app-9.log"]);
        // Another folder or Server forgets them all.
        space.edit_live_glob(&id, "s1", "web1", "/var/log/other", "app-*.log", &plan(&[], &["app-1.log", "app-2.log"], &[])).unwrap();
        assert!(space.glob(&id).unwrap().excluded.is_empty());
    }

    #[test]
    fn editing_a_glob_into_another_glob_of_the_space_is_refused() {
        let (mut space, id) = glob_space();
        space.add_live_glob("s1", "/var/log/app", "app-*.log").unwrap();
        let before = space.glob(&id).cloned();
        let result = space.edit_live_glob(&id, "s1", "web1", "/var/log/app", "app-*.log", &GlobEditPlan::default());
        assert_eq!(result, Err(EditError::Duplicate));
        assert_eq!(space.glob(&id).cloned(), before);
        assert_eq!(space.glob_files(&id).len(), 3);
    }

    #[test]
    fn an_edited_space_is_saved_and_read_back_in_the_same_format() {
        let mut space = space();
        add(&mut space, "/app/ap.log");
        space.replace_remote_target("remote:s1:/app/ap.log", "s1", "web1", file_target("/app/app.log")).unwrap();
        space.set_start_time("remote:s1:/app/ap.log", Some("3h".into()));
        let gid = space.add_live_glob("s1", "/var/log/app", "*.log").unwrap();
        space.edit_live_glob(&gid, "s1", "web1", "/var/log/app", "app-*.log", &GlobEditPlan::default()).unwrap();
        let json = serde_json::to_string(&space).unwrap();
        let back: LogSpace = serde_json::from_str(&json).unwrap();
        let file = back.file("remote:s1:/app/ap.log").unwrap();
        assert_eq!(file.display_name, "web1:/app/app.log");
        assert_eq!(file.remote.as_ref().unwrap().path(), Some("/app/app.log"));
        assert_eq!(file.remote.as_ref().unwrap().start_time.as_deref(), Some("3h"));
        assert_eq!(back.glob(&gid).unwrap().pattern, "app-*.log");
        // No new field: the stored form is the old one.
        assert!(!json.contains("added_seq\":null"));
    }
}

#[test]
fn only_several_files_or_a_live_glob_show_a_group() {
    let empty = imported(&[]);
    assert!(!empty.shows_group());
    assert!(empty.single_file().is_none());

    let one = imported(&["/var/log/a.log"]);
    assert!(!one.shows_group());
    assert_eq!(one.single_file().map(|f| f.display_name.as_str()), Some("a.log"));

    let two = imported(&["/var/log/a.log", "/var/log/b.log"]);
    assert!(two.shows_group());
    assert!(two.single_file().is_none());

    let mut glob = LogSpace::new("g".into(), "Globbed".into(), LogSpaceKind::Remote);
    glob.add_live_glob("s1", "/var/log/app", "*.log").unwrap();
    assert!(glob.shows_group());
    assert!(glob.single_file().is_none());
}

#[test]
fn a_space_goes_back_to_a_plain_row_when_a_file_is_removed() {
    let mut space = imported(&["/var/log/a.log", "/var/log/b.log"]);
    assert!(space.shows_group());
    let id = space.files[0].file_id.clone();
    space.remove_file(&id).unwrap();
    assert!(!space.shows_group());
}
