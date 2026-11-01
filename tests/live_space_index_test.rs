//! Live lines in a Log Space go through its files' own index
//! (lean-view-memory): the space's Filter columns add up its files' again,
//! in place, and give what building the view again gives.
mod common;

use common::fake_live::{check_against_rebuild, lines, open, setup};
use gpui::TestAppContext;
use log_gpui::filter_index::ViewIndex;

/// Lines of `name` at a Level no line had before.
fn fatal(name: &str, range: std::ops::Range<usize>, offset: usize) -> Vec<String> {
    lines(name, range, offset).into_iter().filter(|l| !l.starts_with(' ')).map(|l| l.replacen(" INFO ", " FATAL ", 1)).collect()
}

#[gpui::test]
fn a_level_first_seen_live_joins_the_filter_values(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0), ("beta", 1)], 60, 1_000_000);
    setup(&mut f);
    let rebuilds = f.h.view.read_with(&f.h.cx, |app, _| app.refreshes.rebuilds);
    for n in 0..3 {
        let start = 60 + n * 20;
        f.send("alpha", fatal("alpha", start..start + 20, 0));
        f.send("beta", lines("beta", start..start + 20, 1));
        f.pump();
        check_against_rebuild(&mut f, &format!("batch {n}"));
    }
    f.h.view.read_with(&f.h.cx, |app, _| {
        assert_eq!(app.refreshes.rebuilds, rebuilds + 3, "only the checks built the view again");
        let index = app.index();
        let fatal = index.column("level").unwrap().values.iter().find(|v| v.folded == "fatal").map(|v| v.count);
        // `fatal` keeps every fourth line of `lines`, the INFO ones.
        assert_eq!(fatal, Some(3 * 4 + 3 * 4), "every FATAL line counted once");
    });
}

#[gpui::test]
fn a_live_log_space_keeps_no_columns_of_its_own_per_row(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 60, 1_000_000);
    let rebuilds = f.h.view.read_with(&f.h.cx, |app, _| app.refreshes.rebuilds);
    // Times stay within the hour `lines` writes in: they do not go back.
    for n in 0..5 {
        let start = 60 + n * 300;
        f.send("alpha", lines("alpha", start..start + 300, 0));
        f.pump();
    }
    f.h.view.read_with(&f.h.cx, |app, _| {
        assert_eq!(app.refreshes.rebuilds, rebuilds, "lines were added in place");
        assert!(app.view.len() > 1_500);
        assert!(matches!(app.filter_index, ViewIndex::Space(_)));
        // Values and maps only: nothing per row, so no row was parsed for
        // the space's columns on top of its file's Indexing.
        assert!(app.filter_index.own_heap_bytes() < 64 * 1024, "{} bytes", app.filter_index.own_heap_bytes());
    });
    check_against_rebuild(&mut f, "after the batches");
}
