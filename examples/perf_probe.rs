//! Time of the work done on a large log: `perf_probe [lines]`.
//!
//! Generates a log of `lines` lines (1,000,000 by default), parses it, and
//! times each step the app runs when the user opens the file, types in
//! Search, ticks a Filter value or adds a Highlight Rule, plus the work done
//! on every frame.
//!
//! Then it opens the real `AppView` on a 1M-line Log Space and times the
//! work the app does for a Live append batch, a Search keystroke, a Filter
//! Value toggle, a Time Range change and a Live Window trim (the first
//! lines are `app ...`). Run it in release:
//! `cargo run --release --example perf_probe [lines]`.
use log_gpui::filter::FilterState;
use log_gpui::filter_index::FilterIndex;
use log_gpui::highlight::{HighlightRule, paint_rows};

use log_gpui::search::SearchState;
use log_gpui::space_view::ParsedFile;
use std::time::Instant;

#[path = "../tests/common/mod.rs"]
mod common;

fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    println!("{label:<40} {:>10.2?}", t.elapsed());
    out
}

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(1_000_000);
    let levels = ["INFO", "DEBUG", "WARN", "ERROR"];
    let tags = ["auth", "api", "db", "cache", "worker"];
    let mut text = String::with_capacity(n * 100);
    for i in 0..n {
        let s = i % 86_400;
        text.push_str(&format!(
            "2026-09-05 {:02}:{:02}:{:02}.{:03} [{}] [{}] request {} handled user={} took {}ms\n",
            s / 3600, (s / 60) % 60, s % 60, i % 1000,
            levels[i % 4], tags[i % 5], i, i % 977, i % 300
        ));
    }
    println!("{n} lines, {:.1} MB", text.len() as f64 / 1e6);

    let parsed = time("parse", || ParsedFile::from_bytes(text.as_bytes()));
    let mut entries = parsed.entries.clone();
    let index = time("FilterIndex::build", || FilterIndex::build(&entries));

    let mut filter = FilterState::new();
    let pass = time("Filter evaluate (none)", || filter.evaluate(&index, &mut entries));
    filter.toggle_value("level", "error");
    let pass_err = time("Filter evaluate (level=error)", || filter.evaluate(&index, &mut entries));
    println!("  pass {} / {}", pass_err.len(), pass.len());

    let rule = HighlightRule::new(1, "level", &["warn".into()], &[], log_gpui::palette::next_rule_color(0)).ok();
    if let Some(rule) = rule {
        time("paint_rows (1 rule)", || paint_rows(&[rule], &entries));
    }

    let mut search = SearchState::new();
    for q in ["t", "ti", "timeout", "user=42"] {
        search.query = q.into();
        time(&format!("Search update_matches '{q}'"), || search.update_matches(&entries, &pass));
        println!("  matches {}", search.total_matches());
    }

    // Per frame: the canvas looks for the longest message among every row.
    let visible: Vec<usize> = (0..entries.len()).collect();
    let _ = &visible;
    // Worst case of finding a line: not the selected row, so a full scan.
    let target = entries.last().unwrap();
    time("line lookup, full scan (last)", || {
        entries.iter().position(|e| {
            e.index == target.index && e.generation == target.generation
                && e.file_id.as_deref().unwrap_or("") == target.file_id.as_deref().unwrap_or("")
        })
    });
    time("visible_indices rebuild", || (0..entries.len()).collect::<Vec<usize>>());

    drop((parsed, entries, index, text));
    app_probe(n);
}

fn live_line(i: usize) -> String {
    let levels = ["INFO", "DEBUG", "WARN", "ERROR"];
    // One line a second, so time never goes backwards in a 1M-line file.
    let s = i % 86_400;
    format!(
        "2026-09-{:02} {:02}:{:02}:{:02}.{:03} [{}] request {} handled user={} took {}ms",
        5 + i / 86_400, s / 3600, (s / 60) % 60, s % 60, i % 1000, levels[i % 4], i, i % 977, i % 300
    )
}

/// The real app on `n` timestamped lines, driven through the calls the UI
/// and the Live pump make.
fn app_probe(n: usize) {
    use log_gpui::live::{SourceEvent, SourceHandle, SourceStatus};
    use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

    let mut cx = gpui::TestAppContext::single();
    let mut h = common::open_app(&mut cx);
    let path = h.dir.path().join("big.log");
    let text: String = (0..n).map(|i| live_line(i) + "\n").collect();
    std::fs::write(&path, &text).unwrap();
    drop(text);
    let source = ImportSource::Disk(path.to_string_lossy().into_owned());
    time("app open Log Space", || {
        h.view.update(&mut h.cx, |app, cx| app.create_log_space("Probe", LogSpaceKind::Imported, &[source], cx));
        h.cx.run_until_parked();
    });
    let rows = h.view.read_with(&h.cx, |app, _| app.all_entries.len());
    println!("  app rows {rows}");

    // Two Highlight Rules, and a Search that matches many lines.
    h.view.update(&mut h.cx, |app, cx| {
        for (i, v) in ["warn", "error"].iter().enumerate() {
            let color = log_gpui::palette::next_rule_color(i);
            let _ = app.add_highlight_rule("level", &[v.to_string()], &[], color, cx);
        }
        app.trigger_search_query("user=42", cx);
    });
    h.cx.run_until_parked();

    // A source that stands for a Remote File: the probe sends its events.
    let file_id = h.view.read_with(&h.cx, |app, _| app.parsed_files.keys().next().unwrap().clone());
    let (tx, events) = async_channel::unbounded();
    let (commands, _commands_rx) = async_channel::unbounded();
    h.view.update(&mut h.cx, |app, _| {
        if let Some(old) = app.live.sources.remove(&file_id) {
            old.handle.close();
        }
        // No trim until the probe asks for one.
        app.live.window = usize::MAX / 4;
        let seq = 1_000;
        app.live.sources.insert(
            file_id.clone(),
            log_gpui::app::live::LiveSourceState {
                handle: SourceHandle { events, commands },
                seq,
                status: SourceStatus::Live,
                name: "big.log".into(),
                remote: true,
                command: false,
                numbers_unavailable: false,
                health: Default::default(),
            },
        );
    });

    let state = |h: &mut common::Harness, what: &str| {
        let (rows, matches, pass) = h.view.read_with(&h.cx, |app, _| {
            (app.all_entries.len(), app.focus.search().total_matches(), app.filter_pass.len())
        });
        println!("  after {what}: rows {rows}, matches {matches}, passing {pass}");
    };
    state(&mut h, "setup");
    let mut next = n;
    let mut send_batch = |count: usize| {
        let lines: Vec<String> = (next..next + count).map(live_line).collect();
        let bytes = lines.iter().map(|l| l.len() as u32 + 1).collect();
        next += count;
        tx.try_send(SourceEvent::Append { lines, bytes, streams: None }).unwrap();
    };

    send_batch(1_000);
    time("app Live append batch (1,000 lines)", || {
        h.view.update(&mut h.cx, |app, cx| app.pump_live(cx));
    });
    state(&mut h, "Live append batch (1,000 lines)");

    time("app Search keystroke ('user=4')", || {
        h.view.update(&mut h.cx, |app, cx| app.trigger_search_query("user=4", cx));
        h.cx.run_until_parked();
    });
    state(&mut h, "Search keystroke ('user=4')");

    time("app Filter Value toggle (level=error)", || {
        h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("level", "error", cx));
    });
    state(&mut h, "Filter Value toggle (level=error)");

    time("app Time Range change (From)", || {
        h.view.update(&mut h.cx, |app, cx| {
            app.filter_state.set_time_range("2026-09-10 00:00", "");
            app.apply_filters_and_refresh(cx);
        });
    });
    state(&mut h, "Time Range change (From)");

    // A Live Window a tenth under the rows held: the next batch drops the
    // oldest ~100,000 lines.
    h.view.update(&mut h.cx, |app, _| app.live.window = n * 9 / 10);
    send_batch(1_000);
    time("app Live Window trim (~100k lines)", || {
        h.view.update(&mut h.cx, |app, cx| app.pump_live(cx));
    });
    state(&mut h, "Live Window trim (~100k lines)");
}
