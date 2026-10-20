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
    deltas: Vec<Option<u64>>,
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
        visible: app.visible_indices.to_vec(),
        deltas: app.all_entries.iter().map(|e| e.delta_time_ms.map(|d| (d * 1000.0).round() as u64)).collect(),
        longest: app.longest_row,
        paints: app.highlight_paints.iter().map(|p| p.as_ref().map(|p| p.rule)).collect(),
        counts,
        matches: app.focus.search().matched_visible_indices.to_vec(),
        shared: app.focus.search().matched_visible_indices.to_vec(),
        focus: app.focus_rows.to_vec(),
        focus_longest: app.focus_longest,
        hits: app.focus.find().hits.clone(),
    }
}

fn assert_same(a: &Snapshot, b: &Snapshot, what: &str) {
    assert_eq!(a.visible, b.visible, "{what}: rows");
    assert_eq!(a.pass, b.pass, "{what}: Filter pass");
    assert_eq!(a.deltas, b.deltas, "{what}: delta times");
    assert_eq!(a.longest, b.longest, "{what}: longest row");
    assert_eq!(a.paints, b.paints, "{what}: highlight paints");
    assert_eq!(a.counts, b.counts, "{what}: rule counts");
    assert_eq!(a.matches, b.matches, "{what}: Matches");
    assert_eq!(a.shared, b.shared, "{what}: shared Matches");
    assert_eq!(a.focus, b.focus, "{what}: Focus Pane rows");
    assert_eq!(a.focus_longest, b.focus_longest, "{what}: longest Focus Pane row");
    assert_eq!(a.hits, b.hits, "{what}: Focus Find hits");
}

/// A Remote File's source that the test sends events through.
fn fake_source(h: &mut Harness) -> (async_channel::Sender<SourceEvent>, String) {
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
                remote: true,
                command: false,
                numbers_unavailable: false,
                health: Default::default(),
            },
        );
    });
    (tx, file_id)
}

fn send(tx: &async_channel::Sender<SourceEvent>, lines: Vec<String>) {
    let bytes = lines.iter().map(|l| l.len() as u32 + 1).collect();
    tx.try_send(SourceEvent::Append { lines, bytes, streams: None }).unwrap();
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
        let rows = h.view.read_with(&h.cx, |app, _| app.all_entries.len());
        assert!(rows > 60 + n * 50, "the batch arrived");
        h.view.update(&mut h.cx, |app, cx| app.apply_filters_and_refresh(cx));
        let full = h.view.read_with(&h.cx, |app, _| snapshot(app));
        assert_same(&incremental, &full, &format!("batch {n}"));
        assert!(!incremental.matches.is_empty() && !incremental.hits.is_empty());
    }
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
