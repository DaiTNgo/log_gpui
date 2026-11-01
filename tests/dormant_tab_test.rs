//! Dormant Tabs (openspec change `line-offset-index`, group 11): only the
//! active tab's files are read; the other tabs read theirs again when
//! chosen.

mod common;

use common::{Harness, open_app};
use gpui::TestAppContext;
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};
use std::collections::BTreeSet;

fn log_file(h: &Harness, name: &str, lines: usize) -> ImportSource {
    let path = h.dir.path().join(name);
    let text: String = (0..lines).map(|i| format!("2026-09-05 10:00:{:02} [INFO] [{name}] line {i}\n", i % 60)).collect();
    std::fs::write(&path, text).unwrap();
    ImportSource::Disk(path.to_string_lossy().into_owned())
}

fn new_space(h: &mut Harness, name: &str, files: &[ImportSource]) -> String {
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space(name, LogSpaceKind::Live, files, cx);
    });
    h.cx.run_until_parked();
    active_space(h)
}

fn active_space(h: &Harness) -> String {
    h.view.read_with(&h.cx, |app, _| {
        let project = app.workspace_state.active_project().unwrap();
        project.working_set.active_tab().unwrap().target.space_id().unwrap().to_string()
    })
}

fn file_ids(h: &Harness, space_id: &str) -> Vec<String> {
    h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().log_space(space_id).unwrap().files.iter().map(|f| f.file_id.clone()).collect()
    })
}

/// The files read now, and those with a live source running.
fn read_now(h: &Harness) -> (BTreeSet<String>, BTreeSet<String>) {
    h.view.read_with(&h.cx, |app, _| {
        (app.parsed_files.keys().cloned().collect(), app.live.sources.keys().cloned().collect())
    })
}

fn set(ids: &[String]) -> BTreeSet<String> {
    ids.iter().cloned().collect()
}

fn tab_of(h: &Harness, space_id: &str) -> usize {
    h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().working_set.tabs().iter().position(|t| t.target.space_id() == Some(space_id)).unwrap()
    })
}

#[gpui::test]
fn only_the_active_tab_reads_its_files(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let (a, b) = (log_file(&h, "a.log", 50), log_file(&h, "b.log", 50));
    let first = new_space(&mut h, "First", &[a]);
    let second = new_space(&mut h, "Second", &[b]);
    let (first_files, second_files) = (file_ids(&h, &first), file_ids(&h, &second));

    assert_eq!(read_now(&h), (set(&second_files), set(&second_files)), "the first tab is Dormant");

    let at = tab_of(&h, &first);
    h.view.update(&mut h.cx, |app, cx| app.switch_to_tab(at, cx));
    h.cx.run_until_parked();
    assert_eq!(read_now(&h), (set(&first_files), set(&first_files)), "leaving a tab lets go of its files");
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.view.len(), 50, "the chosen tab reads its file again"));
}

#[gpui::test]
fn a_file_view_reads_its_own_file_only(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let files: Vec<ImportSource> = (0..3).map(|i| log_file(&h, &format!("f{i}.log"), 20)).collect();
    let space = new_space(&mut h, "Payment", &files);
    let ids = file_ids(&h, &space);
    h.view.update(&mut h.cx, |app, cx| app.open_file_view_tab(&space, &ids[1], cx));
    h.cx.run_until_parked();
    let only = vec![ids[1].clone()];
    assert_eq!(read_now(&h), (set(&only), set(&only)));
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.view.len(), 20));
}

#[gpui::test]
fn a_dormant_tab_stays_paused(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let (a, b) = (log_file(&h, "a.log", 5), log_file(&h, "b.log", 5));
    let first = new_space(&mut h, "First", &[a]);
    h.view.update(&mut h.cx, |app, cx| app.toggle_pause(cx));
    let _second = new_space(&mut h, "Second", &[b]);
    h.view.read_with(&h.cx, |app, _| assert!(app.live.paused_spaces.contains(&first)));
    let at = tab_of(&h, &first);
    h.view.update(&mut h.cx, |app, cx| app.switch_to_tab(at, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert!(app.live.paused_spaces.contains(&first), "still paused when chosen again"));
}

#[gpui::test]
fn returning_to_a_tab_keeps_where_it_was(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let (a, b) = (log_file(&h, "a.log", 300), log_file(&h, "b.log", 10));
    let first = new_space(&mut h, "First", &[a]);
    h.view.update(&mut h.cx, |app, _| app.selected_row_pos = Some(200));
    let _second = new_space(&mut h, "Second", &[b]);
    let at = tab_of(&h, &first);
    h.view.update(&mut h.cx, |app, cx| app.switch_to_tab(at, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.selected_row_pos, Some(200)));
}

#[gpui::test]
fn scratch_edits_are_saved_before_their_lines_go(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Notes", LogSpaceKind::Scratch, &[], cx);
    });
    h.cx.run_until_parked();
    let notes = active_space(&h);
    h.view.update(&mut h.cx, |app, cx| app.new_scratch_file_in(&notes, cx));
    h.cx.run_until_parked();
    let (file_id, path) = h.view.read_with(&h.cx, |app, _| {
        let f = &app.workspace_state.active_project().unwrap().log_space(&notes).unwrap().files[0];
        (f.file_id.clone(), f.path.clone().unwrap())
    });
    // As typing does, without waiting for the save after it.
    h.view.update(&mut h.cx, |app, _| {
        app.raw.texts.insert(file_id.clone(), "typed just now".into());
        app.raw.edited.insert(file_id.clone());
        app.raw.unsaved.insert(file_id.clone());
    });
    let b = log_file(&h, "b.log", 3);
    new_space(&mut h, "Other", &[b]);
    assert!(!read_now(&h).0.contains(&file_id), "the scratch file's lines went");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "typed just now");
}

#[gpui::test]
fn the_tab_strip_tells_dormant_tabs(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let (a, b) = (log_file(&h, "a.log", 5), log_file(&h, "b.log", 5));
    let first = new_space(&mut h, "First", &[a]);
    new_space(&mut h, "Second", &[b]);
    let (at, active) = (tab_of(&h, &first), tab_of(&h, &active_space(&h)));
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.is_dormant_tab(at));
        assert!(!app.is_dormant_tab(active));
    });
}

/// Choosing a Dormant Tab again fits its columns to the widest values
/// Indexing found, without reading every line again (task 7.4).
#[gpui::test]
fn a_dormant_tab_chosen_again_fits_its_columns_without_reading_its_lines(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let (a, b) = (log_file(&h, "a.log", 3000), log_file(&h, "b.log", 5));
    let first = new_space(&mut h, "First", &[a]);
    new_space(&mut h, "Second", &[b]);
    let at = tab_of(&h, &first);
    h.view.update(&mut h.cx, |app, cx| app.switch_to_tab(at, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let file = app.parsed_files.values().next().and_then(|p| p.indexed.clone()).expect("read through its Line Index");
        assert!(file.cached_entry(1500).is_none(), "the lines far from the top were not read");
        let every_line = log_gpui::column::FieldMaxChars::scan_entries(&app.rows_snapshot());
        let have = &app.column_layout.max_chars;
        assert_eq!((have.level, have.module), (every_line.level, every_line.module));
    });
}
