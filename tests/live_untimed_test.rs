//! A file with no times (the output of a Remote Command) in a Log Space with
//! files that have them: a Live batch of either adds its rows without
//! building the view again, in the order a full build gives.
mod common;

use common::fake_live::{check_against_rebuild, lines, open, setup};
use gpui::TestAppContext;

#[gpui::test]
fn lines_of_a_file_with_no_time_are_added_in_place(cx: &mut TestAppContext) {
    // `plain` is the last file: its rows end the view.
    let mut f = open(cx, &[("alpha", 0), ("beta", 1), ("plain", 0)], 60, 1_000_000);
    setup(&mut f);
    assert!(f.h.view.read_with(&f.h.cx, |app, _| app.no_timestamp_files.len()) == 1);
    for n in 0..4 {
        let start = 60 + n * 20;
        // Plain lines alone, then a batch of every file at once.
        f.send("plain", lines("plain", start..start + 20, 0));
        if n % 2 == 1 {
            f.send("alpha", lines("alpha", start..start + 20, 0));
            f.send("beta", lines("beta", start..start + 10, 1));
        }
        let before = f.h.view.read_with(&f.h.cx, |app, _| app.refreshes);
        f.pump();
        let after = f.h.view.read_with(&f.h.cx, |app, _| app.refreshes);
        assert_eq!(after.rebuilds, before.rebuilds, "batch {n}: no rebuild of the view");
        assert!(after.appended + after.full > before.appended + before.full);
        check_against_rebuild(&mut f, &format!("batch {n}"));
    }
}

#[gpui::test]
fn the_file_without_a_time_may_come_first_in_the_file_order(cx: &mut TestAppContext) {
    // `plain` is first in the file order, but its rows still come last.
    let mut f = open(cx, &[("plain", 0), ("alpha", 0), ("beta", 1)], 40, 1_000_000);
    setup(&mut f);
    for n in 0..3 {
        let start = 40 + n * 15;
        f.send("beta", lines("beta", start..start + 15, 1));
        f.send("plain", lines("plain", start..start + 15, 0));
        let before = f.h.view.read_with(&f.h.cx, |app, _| app.refreshes);
        f.pump();
        let after = f.h.view.read_with(&f.h.cx, |app, _| app.refreshes);
        assert_eq!(after.rebuilds, before.rebuilds, "batch {n}: no rebuild of the view");
        check_against_rebuild(&mut f, &format!("batch {n}"));
    }
}

#[gpui::test]
fn a_space_of_only_a_file_with_no_time_adds_its_lines_in_place(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("plain", 0)], 30, 1_000_000);
    setup(&mut f);
    for n in 0..3 {
        let start = 30 + n * 10;
        f.send("plain", lines("plain", start..start + 10, 0));
        let before = f.h.view.read_with(&f.h.cx, |app, _| app.refreshes);
        f.pump();
        let after = f.h.view.read_with(&f.h.cx, |app, _| app.refreshes);
        assert_eq!(after.rebuilds, before.rebuilds, "batch {n}: no rebuild of the view");
        check_against_rebuild(&mut f, &format!("batch {n}"));
    }
}
