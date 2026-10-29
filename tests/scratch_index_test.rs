//! A Scratch file is read through a Line Index on its text in memory
//! (openspec change `line-offset-index`, task 8.2); an edit indexes it
//! again, keeping the Markers whose line is the same.

mod common;

use common::open_app;
use gpui::TestAppContext;
use log_gpui::palette::{HighlightColor, Hue, Shade};
use log_gpui::workspace::log_space::LogSpaceKind;

const TEXT: &str = "2026-10-20 10:00:01 INFO a first\n2026-10-20 10:00:02 WARN b second\n2026-10-20 10:00:03 INFO c third";

#[gpui::test]
fn a_scratch_file_is_indexed_and_an_edit_indexes_it_again(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("Notes", LogSpaceKind::Scratch, &[], cx));
    h.cx.run_until_parked();
    let space = h.view.read_with(&h.cx, |app, _| app.workspace_state.active_project().unwrap().log_spaces.last().unwrap().id.clone());
    h.view.update(&mut h.cx, |app, cx| app.new_scratch_file_in(&space, cx));
    h.cx.run_until_parked();
    let file_id = h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().log_space(&space).unwrap().files[0].file_id.clone()
    });
    // Typed, with a Marker on the second line.
    let edit = |h: &mut common::Harness, text: &str| {
        let (id, text) = (file_id.clone(), text.to_string());
        h.view.update(&mut h.cx, |app, cx| {
            app.raw.texts.insert(id.clone(), text);
            app.raw.edited.insert(id.clone());
            app.raw.unsaved.insert(id);
            app.rebuild_open_view_keeping_position(cx);
        });
        h.cx.run_until_parked();
    };
    edit(&mut h, TEXT);
    h.view.update(&mut h.cx, |app, _| {
        assert!(app.parsed_files[&file_id].indexed.is_some(), "read through a Line Index");
        assert_eq!(app.view.len(), 3);
        let project = app.workspace_state.active_project_mut().unwrap();
        let file = project.log_space_mut(&space).unwrap().file_mut(&file_id).unwrap();
        let color = HighlightColor::palette(Hue::Red, Shade::Medium);
        assert!(file.paint_marker(2, "2026-10-20 10:00:02 WARN b second", 0..4, color));
        app.reapply_markers();
        assert_eq!(app.rows().marked_rows(), [1]);
    });

    // A line pasted at the top: the marked line moves down.
    edit(&mut h, &format!("2026-10-20 10:00:00 INFO z pasted\n{TEXT}"));
    h.view.update(&mut h.cx, |app, cx| {
        app.flush_scratch_saves();
        app.rebuild_open_view_keeping_position(cx);
    });
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.view.len(), 4);
        assert!(app.parsed_files[&file_id].indexed.is_some());
        assert_eq!(app.rows().get(2).unwrap().raw_line.as_ref(), "2026-10-20 10:00:02 WARN b second");
    });
}
