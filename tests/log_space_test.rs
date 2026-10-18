use log_gpui::workspace::log_space::{
    FileOrder, LogFile, LogSpace, LogSpaceKind, RemoveFileError, SpaceNameError, WrongKind,
    line_hash, validate_space_name,
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
    assert_eq!(space.files.last().unwrap().path.as_deref(), Some("/tmp/spaces/s/untitled-2.log"));
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

#[test]
fn editing_a_line_drops_its_marker_but_keeps_the_others() {
    use log_gpui::marker::MarkerType;
    use log_gpui::workspace::log_space::StoredMarker;
    let mut file = LogFile::new("f".into(), "f.log".into());
    let marker = |line: usize, text: &str| StoredMarker {
        line,
        marker: Some(MarkerType::Bookmark),
        note: None,
        content_hash: Some(line_hash(text)),
    };
    file.markers = vec![marker(1, "alpha"), marker(2, "beta"), marker(3, "gamma")];

    // Line 2 was edited and a line was appended.
    let dropped = file.prune_markers(&["alpha", "beta edited", "gamma", "delta"]);

    assert_eq!(dropped, 1);
    let lines: Vec<usize> = file.markers.iter().map(|m| m.line).collect();
    assert_eq!(lines, vec![1, 3]);
}

#[test]
fn inserting_a_line_above_drops_the_markers_below() {
    use log_gpui::marker::MarkerType;
    use log_gpui::workspace::log_space::StoredMarker;
    let mut file = LogFile::new("f".into(), "f.log".into());
    file.markers = vec![StoredMarker {
        line: 2,
        marker: Some(MarkerType::Bookmark),
        note: None,
        content_hash: Some(line_hash("beta")),
    }];
    let dropped = file.prune_markers(&["alpha", "new", "beta"]);
    assert_eq!(dropped, 1);
    assert!(file.markers.is_empty());
}

#[test]
fn a_marker_past_the_end_of_the_file_is_dropped() {
    use log_gpui::marker::MarkerType;
    use log_gpui::workspace::log_space::StoredMarker;
    let mut file = LogFile::new("f".into(), "f.log".into());
    file.markers = vec![StoredMarker {
        line: 5,
        marker: Some(MarkerType::Bookmark),
        note: None,
        content_hash: Some(line_hash("x")),
    }];
    assert_eq!(file.prune_markers(&["a"]), 1);
}
