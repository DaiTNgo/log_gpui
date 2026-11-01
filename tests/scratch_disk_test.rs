//! Scratch files are read from disk like local files (memory-budget, groups
//! 3 and 4): no copy of their text is indexed, an edit is saved before it is
//! read again, and the editor goes when the tab goes Dormant.

mod common;

use common::{Harness, open_app};
use gpui::TestAppContext;
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

const TEXT: &str = "2026-10-20 10:00:01 INFO a first\n2026-10-20 10:00:02 WARN b second\n2026-10-20 10:00:03 INFO c third\n";

/// A Scratch Log Space named `name` with one empty file: the space's id,
/// the file's id and path. The tests of one process share a config dir and
/// a Log Space's id comes from its name, so each test names its own.
fn scratch(h: &mut Harness, name: &str) -> (String, String, String) {
    h.view.update(&mut h.cx, |app, cx| app.create_log_space(name, LogSpaceKind::Scratch, &[], cx));
    h.cx.run_until_parked();
    let space = h.view.read_with(&h.cx, |app, _| app.workspace_state.active_project().unwrap().log_spaces.last().unwrap().id.clone());
    h.view.update(&mut h.cx, |app, cx| app.new_scratch_file_in(&space, cx));
    h.cx.run_until_parked();
    let (id, path) = h.view.read_with(&h.cx, |app, _| {
        let f = &app.workspace_state.active_project().unwrap().log_space(&space).unwrap().files[0];
        (f.file_id.clone(), f.path.clone().unwrap())
    });
    (space, id, path)
}

fn tab_of(h: &Harness, space_id: &str) -> usize {
    h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().working_set.tabs().iter().position(|t| t.target.space_id() == Some(space_id)).unwrap()
    })
}

/// Open another Log Space, so the scratch tab is left.
fn leave(h: &mut Harness) {
    let path = h.dir.path().join("other.log");
    std::fs::write(&path, "2026-10-20 10:00:01 INFO other\n").unwrap();
    let source = ImportSource::Disk(path.to_string_lossy().into_owned());
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("Other", LogSpaceKind::Static, &[source], cx));
    h.cx.run_until_parked();
}

fn type_text(h: &mut Harness, id: &str, text: &str) {
    let (id, text) = (id.to_string(), text.to_string());
    h.view.update(&mut h.cx, |app, _| {
        app.raw.texts.insert(id.clone(), text);
        app.raw.edited.insert(id.clone());
        app.raw.unsaved.insert(id);
    });
}

#[gpui::test]
fn a_scratch_file_on_disk_is_read_in_the_background(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let (space, id, path) = scratch(&mut h, "Background");
    std::fs::write(&path, TEXT).unwrap();
    leave(&mut h);
    let tab = tab_of(&h, &space);
    h.view.update(&mut h.cx, |app, cx| app.switch_to_tab(tab, cx));
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.is_reading(), "Indexing runs in the background");
        assert!(!app.parsed_files.contains_key(&id), "nothing read on the UI thread");
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.parsed_files[&id].indexed.is_some());
        assert_eq!(app.view.len(), 3);
    });
}

#[gpui::test]
fn an_empty_or_missing_scratch_file_shows_empty_at_once(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let (space, id, path) = scratch(&mut h, "Empty");
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.parsed_files.contains_key(&id), "an empty file needs no read");
        assert!(!app.is_reading());
        assert_eq!(app.view.len(), 0);
    });
    leave(&mut h);
    std::fs::remove_file(&path).unwrap();
    let tab = tab_of(&h, &space);
    h.view.update(&mut h.cx, |app, cx| app.switch_to_tab(tab, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.parsed_files.contains_key(&id));
        assert_eq!(app.view.len(), 0);
        assert!(app.notice.is_none(), "a missing scratch file reads as empty: {:?}", app.notice);
    });
}

#[gpui::test]
fn an_edit_is_saved_then_read_again_from_disk(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let (_, id, path) = scratch(&mut h, "Saved");
    type_text(&mut h, &id, TEXT);
    h.view.update(&mut h.cx, |app, cx| app.rebuild_open_view_keeping_position(cx));
    h.view.read_with(&h.cx, |app, _| assert!(app.raw.unsaved.is_empty(), "saved before it is read"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), TEXT);
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.view.len(), 3);
        assert!(app.raw.edited.is_empty());
    });
}

#[cfg(unix)]
#[gpui::test]
fn an_edit_that_cannot_be_saved_keeps_the_rows_it_had(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt;
    let mut h = open_app(cx);
    let (_, id, path) = scratch(&mut h, "Unsaved");
    type_text(&mut h, &id, TEXT);
    h.view.update(&mut h.cx, |app, cx| app.rebuild_open_view_keeping_position(cx));
    h.cx.run_until_parked();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    type_text(&mut h, &id, &format!("{TEXT}2026-10-20 10:00:04 INFO d fourth\n"));
    h.view.update(&mut h.cx, |app, cx| app.rebuild_open_view_keeping_position(cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.notice.as_deref().is_some_and(|n| n.contains("Could not save")), "{:?}", app.notice);
        assert_eq!(app.view.len(), 3, "the rows read before");
        assert!(app.raw.edited.contains(&id), "read again once it is saved");
        assert!(app.raw.texts.contains_key(&id), "the text typed stays");
    });
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
}

#[gpui::test]
fn a_dormant_scratch_tab_lets_its_text_go_once_saved(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let (_, id, path) = scratch(&mut h, "Dormant");
    type_text(&mut h, &id, TEXT);
    leave(&mut h);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), TEXT, "saved first");
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.raw.texts.contains_key(&id), "no copy of the text is left");
        assert!(!app.raw.edited.contains(&id));
        assert!(!app.parsed_files.contains_key(&id));
    });
}

#[cfg(unix)]
#[gpui::test]
fn a_dormant_scratch_tab_that_could_not_save_keeps_its_text(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt;
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let (_, id, path) = scratch(&mut h, "Dormant unsaved");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    type_text(&mut h, &id, TEXT);
    leave(&mut h);
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.raw.texts.contains_key(&id), "kept until it is saved");
        assert!(app.raw.unsaved.contains(&id));
    });
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
}

/// Opening a scratch tab in its Raw View while the file is still being read
/// must not give it an empty editor: that would show nothing and then be
/// saved over the file.
#[gpui::test]
fn the_raw_editor_waits_for_the_file_to_be_read(cx: &mut TestAppContext) {
    use gpui::px;
    use log_gpui::workspace::working_set::ViewMode;
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let space = h.view.update(&mut h.cx, |app, cx| app.create_log_space("Raw Waits", LogSpaceKind::Scratch, &[], cx).unwrap());
    h.cx.run_until_parked();
    let (id, path) = h.view.read_with(&h.cx, |app, _| {
        let f = &app.workspace_state.active_project().unwrap().log_space(&space).unwrap().files[0];
        (f.file_id.clone(), f.path.clone().unwrap())
    });
    std::fs::write(&path, TEXT).unwrap();
    leave(&mut h);
    let tab = tab_of(&h, &space);
    h.view.update(&mut h.cx, |app, cx| {
        app.switch_to_tab(tab, cx);
        app.set_view_mode(ViewMode::Raw, cx);
    });
    // One frame while the read is still under way.
    h.cx.simulate_resize(gpui::size(px(900.0), px(700.0)));
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.parsed_files.contains_key(&id), "the read has not finished");
        assert!(!app.raw.texts.contains_key(&id), "no empty text is kept for the file");
    });
    h.cx.run_until_parked();
    h.cx.simulate_resize(gpui::size(px(901.0), px(700.0)));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, cx| {
        let editor = app.raw.editors.get(&id).expect("the editor opens once the file is read");
        assert_eq!(editor.state.read(cx).value().to_string(), TEXT.trim_end_matches('\n').to_string());
    });
}
