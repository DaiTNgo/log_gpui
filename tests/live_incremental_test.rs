//! A Live batch appended after the last row gives what building everything
//! again would: Filter, delta times, longest row, highlights, Matches, the
//! Focus Pane and Focus Find.
mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::AppView;
use log_gpui::app::live::LiveSourceState;
use log_gpui::live::{SourceEvent, SourceHandle, SourceStatus};

fn line(i: usize) -> String {
    let levels = ["INFO", "ERROR", "WARN", "INFO", "DEBUG"];
    let (h, m, s) = (10 + i / 3600, (i / 60) % 60, i % 60);
    let long = if i % 17 == 0 { " with a rather long message to move the longest row".repeat(1 + i % 5) } else { String::new() };
    format!("2026-10-19 {h:02}:{m:02}:{s:02}.{:03} {} request {i} user={}{long}", i % 1000, levels[i % 5], i % 7)
}

/// The lines of a batch: every ninth is followed by a Continuation Line.
fn batch(range: std::ops::Range<usize>) -> Vec<String> {
    let mut out = Vec::new();
    for i in range {
        out.push(line(i));
        if i % 9 == 0 {
            out.push(format!("    at com.example.Handler.run(Handler.java:{i})"));
        }
    }
    out
}

/// The lines of the file as opened: it ends with a newline, so its last line
/// is finished and no batch replaces it.
fn file_lines(range: std::ops::Range<usize>) -> Vec<String> {
    let mut lines = batch(range);
    lines.push(String::new());
    lines
}

struct Snapshot {
    pass: Vec<usize>,
    visible: Vec<usize>,
    longest: usize,
    paints: Vec<Option<usize>>,
    counts: Vec<(u64, usize)>,
    matches: Vec<usize>,
    shared: Vec<usize>,
    focus: Vec<log_gpui::focus::FocusRow>,
    focus_longest: usize,
    hits: Vec<usize>,
}

fn snapshot(app: &AppView) -> Snapshot {
    let mut counts: Vec<(u64, usize)> = app.rule_counts.iter().map(|(k, v)| (*k, *v)).collect();
    counts.sort();
    Snapshot {
        pass: app.filter_pass.to_vec(),
        visible: (0..app.view.len()).collect(),
        longest: app.longest_row,
        paints: app.highlight_paints.iter().map(|p| p.map(|p| p.rule)).collect(),
        counts,
        matches: app.focus.search().matched_rows.to_vec(),
        shared: app.focus.search().matched_rows.to_vec(),
        focus: app.focus_rows.to_vec(),
        focus_longest: app.focus_longest,
        hits: app.focus.find().hits.clone(),
    }
}

fn assert_same(a: &Snapshot, b: &Snapshot, what: &str) {
    assert_eq!(a.visible, b.visible, "{what}: rows");
    assert_eq!(a.pass, b.pass, "{what}: Filter pass");
    assert_eq!(a.longest, b.longest, "{what}: longest row");
    assert_eq!(a.paints, b.paints, "{what}: highlight paints");
    assert_eq!(a.counts, b.counts, "{what}: rule counts");
    assert_eq!(a.matches, b.matches, "{what}: Matches");
    assert_eq!(a.shared, b.shared, "{what}: shared Matches");
    assert_eq!(a.focus, b.focus, "{what}: Focus Pane rows");
    assert_eq!(a.focus_longest, b.focus_longest, "{what}: longest Focus Pane row");
    assert_eq!(a.hits, b.hits, "{what}: Focus Find hits");
}

/// A source of the local file that the test sends events through: what it
/// sends is written to the file first.
fn fake_source(h: &mut Harness) -> (Sender, String) {
    let file_id = h.view.read_with(&h.cx, |app, _| app.parsed_files.keys().next().unwrap().clone());
    let (tx, events) = async_channel::unbounded();
    let (commands, _rx) = async_channel::unbounded();
    let id = file_id.clone();
    h.view.update(&mut h.cx, |app, _| {
        if let Some(old) = app.live.sources.remove(&id) {
            old.handle.close();
        }
        app.live.window = 1_000_000;
        app.live.sources.insert(
            id,
            LiveSourceState {
                handle: SourceHandle { events, commands },
                seq: 1_000,
                status: SourceStatus::Live,
                name: "app.log".into(),
                remote: false,
                command: false,
                numbers_unavailable: false,
                health: Default::default(),
            },
        );
    });
    (Sender { tx, path: h.dir.path().join("app.log") }, file_id)
}

struct Sender {
    tx: async_channel::Sender<SourceEvent>,
    path: std::path::PathBuf,
}

fn send(to: &Sender, lines: Vec<String>) {
    use std::io::Write as _;
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::OpenOptions::new().append(true).open(&to.path).unwrap().write_all(text.as_bytes()).unwrap();
    let bytes = lines.iter().map(|l| l.len() as u32 + 1).collect();
    to.tx.try_send(SourceEvent::Append { lines, bytes, streams: None }).unwrap();
}

#[gpui::test]
fn incremental_batches_equal_a_full_refresh(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &file_lines(0..60));
    let (tx, _) = fake_source(&mut h);
    h.view.update(&mut h.cx, |app, cx| {
        let color = log_gpui::palette::next_rule_color(0);
        app.add_highlight_rule("level", &["warn".to_string()], &["user=3".to_string()], color, cx).unwrap();
        app.trigger_search_query("user=4 || ERROR", cx);
        app.on_focus_find_changed("request 1", cx);
    });

    for (n, filtered) in [(0, false), (1, false), (2, true), (3, true)] {
        if filtered && n == 2 {
            h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("level", "error", cx));
        }
        let start = 60 + n * 50;
        send(&tx, batch(start..start + 50));
        let before = h.view.read_with(&h.cx, |app, _| app.refreshes);
        h.view.update(&mut h.cx, |app, cx| app.pump_live(cx));
        let after = h.view.read_with(&h.cx, |app, _| app.refreshes);
        assert_eq!((after.full, after.appended), (before.full, before.appended + 1), "only the new rows were read");
        let incremental = h.view.read_with(&h.cx, |app, _| snapshot(app));
        let rows = h.view.read_with(&h.cx, |app, _| app.view.len());
        assert!(rows > 60 + n * 50, "the batch arrived");
        h.view.update(&mut h.cx, |app, cx| app.apply_filters_and_refresh(cx));
        let full = h.view.read_with(&h.cx, |app, _| snapshot(app));
        assert_same(&incremental, &full, &format!("batch {n}"));
        assert!(!incremental.matches.is_empty() && !incremental.hits.is_empty());
    }
}

#[gpui::test]
fn the_same_with_the_find_filter_on(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &file_lines(0..60));
    let (tx, _) = fake_source(&mut h);
    h.view.update(&mut h.cx, |app, cx| {
        app.trigger_search_query("user=4 || ERROR", cx);
        app.on_focus_find_changed("request 1", cx);
        app.toggle_find_filter(cx);
    });
    let label = |h: &mut Harness| h.view.read_with(&h.cx, |app, _| app.focus.find().label());
    let start_label = label(&mut h);
    assert!(start_label.ends_with(" lines"), "{start_label}");
    for (n, filtered) in [(0, false), (1, true)] {
        if filtered {
            h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("level", "error", cx));
        }
        let start = 60 + n * 50;
        send(&tx, batch(start..start + 50));
        let before = h.view.read_with(&h.cx, |app, _| app.refreshes);
        h.view.update(&mut h.cx, |app, cx| app.pump_live(cx));
        let after = h.view.read_with(&h.cx, |app, _| app.refreshes);
        assert_eq!((after.full, after.appended), (before.full, before.appended + 1), "only the new rows were read");
        let incremental = h.view.read_with(&h.cx, |app, _| snapshot(app));
        let incremental_label = label(&mut h);
        h.view.update(&mut h.cx, |app, cx| app.apply_filters_and_refresh(cx));
        let full = h.view.read_with(&h.cx, |app, _| snapshot(app));
        assert_same(&incremental, &full, &format!("batch {n}"));
        assert_eq!(incremental_label, label(&mut h), "batch {n}: counter");
        // Every row the pane lists is one Focus Find finds.
        assert_eq!(incremental.hits, (0..incremental.focus.len()).collect::<Vec<_>>());
    }
    assert_ne!(label(&mut h), start_label, "the counter updates");
}

#[gpui::test]
fn the_same_without_search_or_filter_and_with_a_time_range(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &file_lines(0..30));
    let (tx, _) = fake_source(&mut h);
    h.view.update(&mut h.cx, |app, cx| {
        app.filter_state.set_time_range("10:00:20", "");
        app.apply_filters_and_refresh(cx);
    });
    for n in 0..3 {
        let start = 30 + n * 40;
        send(&tx, batch(start..start + 40));
        let before = h.view.read_with(&h.cx, |app, _| app.refreshes);
        h.view.update(&mut h.cx, |app, cx| app.pump_live(cx));
        let after = h.view.read_with(&h.cx, |app, _| app.refreshes);
        assert_eq!((after.full, after.appended), (before.full, before.appended + 1), "only the new rows were read");
        let incremental = h.view.read_with(&h.cx, |app, _| snapshot(app));
        h.view.update(&mut h.cx, |app, cx| app.apply_filters_and_refresh(cx));
        let full = h.view.read_with(&h.cx, |app, _| snapshot(app));
        assert_same(&incremental, &full, &format!("batch {n}"));
    }
}

#[gpui::test]
fn a_file_view_taking_live_lines_lists_no_rows(cx: &mut TestAppContext) {
    use log_gpui::index::view_rows::ViewRows;
    let mut h = common::open_app_with_lines(cx, &file_lines(0..60));
    let (space, file_id) = h.view.read_with(&h.cx, |app, _| {
        let project = app.workspace_state.active_project().unwrap();
        let space = project.working_set.active_tab().unwrap().target.space_id().unwrap().to_string();
        (space.clone(), project.log_space(&space).unwrap().files[0].file_id.clone())
    });
    h.view.update(&mut h.cx, |app, cx| app.open_file_view_tab(&space, &file_id, cx));
    h.cx.run_until_parked();
    // Sources start again for the tab chosen: the test's goes in after.
    let (tx, _) = fake_source(&mut h);
    let rows_before = h.view.read_with(&h.cx, |app, _| app.view.len());
    for n in 0..4 {
        send(&tx, batch(60 + n * 2500..60 + (n + 1) * 2500));
        h.view.update(&mut h.cx, |app, cx| app.pump_live(cx));
        h.cx.run_until_parked();
    }
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.view.len() >= rows_before + 10_000, "the lines arrived: {} rows", app.view.len());
        assert_eq!(app.view.rows, ViewRows::File { lines: app.view.len() });
    });
}

/// The longest row after Live lines are added in place is the one a full
/// build picks: the longest line in bytes, the first of equals in file
/// order, not the longest message.
#[gpui::test]
fn the_longest_row_added_in_place_is_the_one_a_rebuild_picks(cx: &mut TestAppContext) {
    use common::fake_live::{lines, open, view};
    let mut f = open(cx, &[("alpha", 0), ("beta", 1)], 60, 1_000_000);
    for n in 0..5 {
        let start = 60 + n * 300;
        f.send("alpha", lines("alpha", start..start + 300, 0));
        f.send("beta", lines("beta", start..start + 150, 1));
        f.pump();
        let in_place = f.h.view.read_with(&f.h.cx, |app, _| view(app).longest);
        f.h.view.update(&mut f.h.cx, |app, cx| app.rebuild_open_view_keeping_position(cx));
        let rebuilt = f.h.view.read_with(&f.h.cx, |app, _| view(app).longest);
        assert_eq!(in_place, rebuilt, "batch {n}");
    }
}
