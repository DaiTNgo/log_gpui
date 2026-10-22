//! The Live Window dropping the oldest lines of a Remote File takes their rows
//! out of the open view in place, and leaves what building the view again
//! from every file would.
mod common;

use common::fake_live::{Files, check_against_rebuild, lines, open, setup};
use gpui::TestAppContext;

#[gpui::test]
fn trimming_one_file_equals_building_the_view_again(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 120, 150);
    setup(&mut f);
    // Select a row that stays.
    f.h.view.update(&mut f.h.cx, |app, cx| app.select_view_row(100, cx));
    for n in 0..3 {
        let start = 120 + n * 40;
        f.send("alpha", lines("alpha", start..start + 40, 0));
        let before = f.h.view.read_with(&f.h.cx, |app, _| app.refreshes);
        f.pump();
        let after = f.h.view.read_with(&f.h.cx, |app, _| app.refreshes);
        assert_eq!(after.full, before.full, "batch {n}: the view was not built again");
        let rows = f.h.view.read_with(&f.h.cx, |app, _| app.all_entries.len());
        assert!(rows <= 150 + 15 + 20, "the Live Window held: {rows} rows");
        check_against_rebuild(&mut f, &format!("batch {n}"));
    }
}

#[gpui::test]
fn trimming_with_the_find_filter_on_keeps_its_counter(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 120, 150);
    setup(&mut f);
    f.h.view.update(&mut f.h.cx, |app, cx| app.toggle_find_filter(cx));
    let label = |f: &mut Files| f.h.view.read_with(&f.h.cx, |app, _| app.focus.find().label());
    for n in 0..3 {
        let start = 120 + n * 40;
        f.send("alpha", lines("alpha", start..start + 40, 0));
        f.pump();
        let in_place = label(&mut f);
        assert!(in_place.ends_with(" lines"), "batch {n}: {in_place}");
        check_against_rebuild(&mut f, &format!("batch {n}"));
        assert_eq!(in_place, label(&mut f), "batch {n}: counter");
    }
}

#[gpui::test]
fn only_the_trimmed_files_rows_leave_a_merged_space(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0), ("beta", 1)], 100, 130);
    setup(&mut f);
    let b_before: Vec<usize> = f.h.view.read_with(&f.h.cx, |app, _| {
        app.all_entries.iter().filter(|e| e.raw_line.contains(" beta ")).map(|e| e.index).collect()
    });
    for n in 0..3 {
        let start = 100 + n * 40;
        f.send("alpha", lines("alpha", start..start + 40, 0));
        f.pump();
        check_against_rebuild(&mut f, &format!("batch {n}"));
    }
    f.h.view.read_with(&f.h.cx, |app, _| {
        let b_after: Vec<usize> =
            app.all_entries.iter().filter(|e| e.raw_line.contains(" beta ")).map(|e| e.index).collect();
        assert_eq!(b_after, b_before, "the other file keeps every row, in order");
        let a_first = app.all_entries.iter().find(|e| e.raw_line.contains(" alpha ")).unwrap().index;
        assert!(a_first > 1, "the oldest rows of the trimmed file left");
    });
}

fn sync_raw(f: &mut Files) {
    let view = f.h.view.clone();
    f.h.cx.update(|window, cx| view.update(cx, |app, cx| app.sync_raw_editors(window, cx)));
}

fn editor_texts(f: &mut Files) -> Vec<(String, String, String)> {
    f.h.view.read_with(&f.h.cx, |app, cx| {
        app.raw
            .editors
            .iter()
            .map(|(id, e)| {
                let shown = e.state.read(cx).value().to_string();
                let held = app.parsed_files[id].lines.join("\n");
                (id.clone(), shown, held)
            })
            .collect()
    })
}

#[gpui::test]
fn the_raw_view_cuts_the_oldest_lines_from_its_text(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0), ("beta", 1)], 100, 130);
    f.h.view.update(&mut f.h.cx, |app, _| app.view_mode = log_gpui::workspace::working_set::ViewMode::Raw);
    sync_raw(&mut f);
    assert_eq!(editor_texts(&mut f).len(), 2, "an editor per file");
    for n in 0..3 {
        let start = 100 + n * 40;
        f.send("alpha", lines("alpha", start..start + 40, 0));
        f.send("beta", lines("beta", start..start + 3, 1));
        f.pump();
        assert!(
            f.h.view.read_with(&f.h.cx, |app, _| app.raw.stale.is_empty()),
            "batch {n}: no editor is built again"
        );
        sync_raw(&mut f);
        for (id, shown, held) in editor_texts(&mut f) {
            assert_eq!(shown, held, "batch {n}: the editor of {id} shows the lines held");
        }
    }
}
