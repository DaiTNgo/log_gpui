//! Where a scratch file is edited in the Raw View: in a Log Space holding
//! only it, or in its own File View. A Log Space with several files shows
//! them as one read-only list, each with an Edit button opening its File
//! View, so only one editor ever holds a file's text.

mod common;

use common::Harness;
use gpui::{TestAppContext, px};
use log_gpui::workspace::log_space::LogSpaceKind;
use log_gpui::workspace::working_set::{TabTarget, ViewMode};

/// A Scratch Log Space with `files` empty files, in its Raw View.
fn open_scratch(cx: &mut TestAppContext, files: usize) -> (Harness, String) {
    let mut h = common::open_app(cx);
    let space_id = h.view.update(&mut h.cx, |app, cx| {
        let id = app.create_log_space("Notes", LogSpaceKind::Scratch, &[], cx).unwrap();
        for _ in 1..files {
            app.new_scratch_file_in(&id, cx);
        }
        app.set_view_mode(ViewMode::Raw, cx);
        id
    });
    draw(&mut h);
    (h, space_id)
}

fn draw(h: &mut Harness) {
    for i in 0..2 {
        h.cx.simulate_resize(gpui::size(px(900.0 + i as f32), px(700.0)));
        h.cx.run_until_parked();
    }
}

#[gpui::test]
fn a_scratch_space_with_one_file_is_edited_in_its_raw_view(cx: &mut TestAppContext) {
    let (h, _) = open_scratch(cx, 1);
    h.view.read_with(&h.cx, |app, _| {
        let sections = app.raw_sections();
        assert_eq!(sections.len(), 1);
        assert!(sections[0].editable);
        assert_eq!(app.raw.editors.len(), 1);
    });
}

#[gpui::test]
fn a_scratch_space_with_several_files_is_a_read_only_list(cx: &mut TestAppContext) {
    let (h, _) = open_scratch(cx, 3);
    h.view.read_with(&h.cx, |app, _| {
        let sections = app.raw_sections();
        assert_eq!(sections.len(), 3);
        assert!(sections.iter().all(|s| s.scratch && !s.editable), "edited one at a time, in a File View");
        assert!(app.raw.editors.is_empty(), "no editor holds their text");
        assert!(app.raw_is_list());
    });
}

#[gpui::test]
fn edit_opens_the_file_alone_in_its_raw_view(cx: &mut TestAppContext) {
    let (mut h, space_id) = open_scratch(cx, 2);
    let second = h.view.read_with(&h.cx, |app, _| app.raw_sections()[1].file_id.clone());
    h.view.update(&mut h.cx, |app, cx| app.edit_scratch_file(&second, cx));
    draw(&mut h);
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.workspace_state.active_project().and_then(|p| p.working_set.active_tab()).map(|t| t.target.clone()), Some(TabTarget::FileView { space_id: space_id.clone(), file_id: second.clone() }));
        assert_eq!(app.view_mode, ViewMode::Raw);
        let sections = app.raw_sections();
        assert_eq!(sections.len(), 1);
        assert!(sections[0].editable);
        assert_eq!(app.raw.editors.keys().collect::<Vec<_>>(), [&second], "one editor, for that file");
    });
}

#[gpui::test]
fn a_second_file_turns_the_editor_into_the_list(cx: &mut TestAppContext) {
    let (mut h, space_id) = open_scratch(cx, 1);
    h.view.update(&mut h.cx, |app, cx| app.new_scratch_file_in(&space_id, cx));
    draw(&mut h);
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.raw.editors.is_empty(), "the editor is dropped");
        assert!(app.raw_is_list());
    });
}

#[gpui::test]
fn collapse_all_and_expand_all(cx: &mut TestAppContext) {
    let (mut h, _) = open_scratch(cx, 2);
    h.view.update(&mut h.cx, |app, cx| {
        assert!(!app.all_sections_collapsed());
        app.set_all_sections_collapsed(true, cx);
        assert!(app.all_sections_collapsed());
        app.set_all_sections_collapsed(false, cx);
        assert!(app.raw_sections().iter().all(|s| !app.is_section_collapsed(&s.file_id)));
    });
}

#[gpui::test]
fn collapsing_all_from_the_bottom_shows_every_header(cx: &mut TestAppContext) {
    let (mut h, _) = open_scratch(cx, 3);
    let ids: Vec<String> = h.view.update(&mut h.cx, |app, _| {
        let ids: Vec<String> = app.raw_sections().into_iter().map(|s| s.file_id).collect();
        let text = (1..=60).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        for id in &ids {
            app.raw.texts.insert(id.clone(), text.clone());
        }
        ids
    });
    draw(&mut h);
    h.view.update(&mut h.cx, |app, cx| {
        app.raw.scroll.0.borrow().base_handle.set_offset(gpui::point(px(0.0), px(-100_000.0)));
        cx.notify();
    });
    draw(&mut h);
    h.view.update(&mut h.cx, |app, cx| app.set_all_sections_collapsed(true, cx));
    draw(&mut h);
    for id in &ids {
        assert!(h.cx.debug_bounds(Box::leak(format!("raw-section-{id}").into_boxed_str())).is_some(), "header of {id} is drawn");
    }
}
