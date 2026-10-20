//! Fake Remote File sources over an Imported Log Space, to drive Live batches
//! through the real `AppView` and compare the view with one built again.
#![allow(dead_code)]

use super::Harness;
use super::open_app;
use gpui::TestAppContext;
use log_gpui::app::AppView;
use log_gpui::app::live::LiveSourceState;
use log_gpui::live::{SourceEvent, SourceHandle, SourceStatus};
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

/// Line `i` of file `name`: a second apart, `step` seconds between files'
/// lines so two files interleave. Every eighth is followed by a Continuation
/// Line.
pub fn lines(name: &str, range: std::ops::Range<usize>, offset: usize) -> Vec<String> {
    let levels = ["INFO", "ERROR", "WARN", "INFO", "DEBUG"];
    let mut out = Vec::new();
    if name.starts_with("plain") {
        // A file with no times at all, like the output of a command.
        return range.map(|i| format!("plain output {name} number {i} user={}", i % 7)).collect();
    }
    for i in range {
        let t = i * 2 + offset;
        let long = if i % 11 == 0 { " and a long message to move the longest row".repeat(1 + i % 4) } else { String::new() };
        out.push(format!(
            "2026-10-19 10:{:02}:{:02}.{:03} {} {name} request {i} user={}{long}",
            (t / 60) % 60,
            t % 60,
            i % 1000,
            levels[i % 5],
            i % 7
        ));
        if i % 8 == 0 {
            out.push(format!("    at com.example.{name}.run(Handler.java:{i})"));
        }
    }
    out
}

pub struct Files {
    pub h: Harness,
    pub senders: Vec<(String, async_channel::Sender<SourceEvent>)>,
}

/// An Imported Log Space of one file per name, read through fake Remote File
/// sources the test sends lines to.
pub fn open(cx: &mut TestAppContext, names: &[(&str, usize)], count: usize, window: usize) -> Files {
    let mut h = open_app(cx);
    let mut sources = Vec::new();
    for (name, offset) in names {
        let path = h.dir.path().join(format!("{name}.log"));
        let mut text = lines(name, 0..count, *offset);
        text.push(String::new());
        std::fs::write(&path, text.join("\n")).unwrap();
        sources.push(ImportSource::Disk(path.to_string_lossy().into_owned()));
    }
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Test", LogSpaceKind::Imported, &sources, cx);
    });
    h.cx.run_until_parked();
    let ids: Vec<(String, String)> = h.view.read_with(&h.cx, |app, _| {
        app.parsed_files
            .iter()
            .map(|(id, p)| (id.clone(), p.lines.first().cloned().unwrap_or_default()))
            .collect()
    });
    let mut senders = Vec::new();
    for (name, _) in names {
        let (id, _) = ids.iter().find(|(_, first)| first.contains(&format!(" {name} "))).expect("file loaded");
        let (tx, events) = async_channel::unbounded();
        let (commands, _rx) = async_channel::unbounded();
        let id = id.clone();
        let file = id.clone();
        h.view.update(&mut h.cx, |app, _| {
            if let Some(old) = app.live.sources.remove(&file) {
                old.handle.close();
            }
            app.live.window = window;
            app.live.sources.insert(
                file,
                LiveSourceState {
                    handle: SourceHandle { events, commands },
                    seq: 1_000,
                    status: SourceStatus::Live,
                    name: format!("{name}.log"),
                    remote: true,
                    command: false,
                    numbers_unavailable: false,
                    health: Default::default(),
                },
            );
        });
        senders.push((name.to_string(), tx));
    }
    Files { h, senders }
}

impl Files {
    pub fn send(&self, name: &str, lines: Vec<String>) {
        let bytes = lines.iter().map(|l| l.len() as u32 + 1).collect();
        let (_, tx) = self.senders.iter().find(|(n, _)| n == name).unwrap();
        tx.try_send(SourceEvent::Append { lines, bytes, streams: None }).unwrap();
    }

    pub fn pump(&mut self) {
        self.h.view.update(&mut self.h.cx, |app, cx| app.pump_live(cx));
    }
}

#[derive(Debug, PartialEq)]
pub struct View {
    pub rows: Vec<(String, usize, u32, Option<i64>, bool)>,
    pub pass: Vec<usize>,
    pub paints: Vec<Option<usize>>,
    pub counts: Vec<(u64, usize)>,
    pub matches: Vec<usize>,
    pub focus: Vec<log_gpui::focus::FocusRow>,
    pub hits: Vec<usize>,
    pub longest: usize,
    pub focus_longest: usize,
    pub selected: Option<(String, usize)>,
    pub index: Vec<(String, Vec<(String, usize)>)>,
    pub times: Vec<Option<i64>>,
    pub hint: Option<(String, String)>,
    pub parents: (usize, usize),
}

pub fn view(app: &AppView) -> View {
    let mut counts: Vec<(u64, usize)> = app.rule_counts.iter().map(|(k, v)| (*k, *v)).collect();
    counts.sort();
    let index = &app.filter_index;
    View {
        rows: app
            .all_entries
            .iter()
            .map(|e| {
                (
                    e.file_id.as_deref().unwrap_or("").to_string(),
                    e.index,
                    e.generation,
                    e.delta_time_ms.map(|d| (d * 1000.0).round() as i64),
                    e.break_before.is_some(),
                )
            })
            .collect(),
        pass: app.filter_pass.to_vec(),
        paints: app.highlight_paints.iter().map(|p| p.as_ref().map(|p| p.rule)).collect(),
        counts,
        matches: app.focus.search().matched_visible_indices.clone(),
        focus: app.focus_rows.to_vec(),
        hits: app.focus.find().hits.clone(),
        longest: app.longest_row,
        focus_longest: app.focus_longest,
        selected: app
            .selected_row_pos
            .and_then(|p| app.line_ref_of_entry(p))
            .map(|l| (l.file_id.to_string(), l.line)),
        index: index
            .columns_in_use()
            .map(|c| {
                let mut values: Vec<(String, usize)> =
                    c.values.iter().filter(|v| v.count > 0).map(|v| (v.folded.clone(), v.count)).collect();
                values.sort();
                (c.key.clone(), values)
            })
            .collect(),
        times: (0..app.all_entries.len()).map(|r| index.effective_timestamp(r).map(|t| (t * 1000.0).round() as i64)).collect(),
        hint: index.time_hint(),
        parents: (index.parent_lines(), index.timed_lines()),
    }
}

pub fn check_against_rebuild(f: &mut Files, what: &str) {
    let in_place = f.h.view.read_with(&f.h.cx, |app, _| view(app));
    f.h.view.update(&mut f.h.cx, |app, cx| app.rebuild_open_view_keeping_position(cx));
    let rebuilt = f.h.view.read_with(&f.h.cx, |app, _| view(app));
    assert_eq!(in_place.rows.len(), rebuilt.rows.len(), "{what}: row count");
    macro_rules! same {
        ($($field:ident),*) => {$(
            if in_place.$field != rebuilt.$field {
                let at = in_place.$field.iter().zip(rebuilt.$field.iter()).position(|(a, b)| a != b);
                panic!("{what}: {} differs (first at {at:?}); lengths {} vs {}; in place {:?} / rebuilt {:?}",
                    stringify!($field), in_place.$field.len(), rebuilt.$field.len(),
                    at.and_then(|i| in_place.$field.get(i)), at.and_then(|i| rebuilt.$field.get(i)));
            }
        )*};
    }
    if std::env::var_os("SHOW").is_some() {
        let short = |v: &View| v.rows.iter().take(40).map(|r| format!("{}{}", &r.0[r.0.len() - 8..r.0.len() - 7], r.1)).collect::<Vec<_>>().join(" ");
        eprintln!("in place: {}\nrebuilt : {}", short(&in_place), short(&rebuilt));
    }
    same!(rows, pass, paints, times, matches, focus, hits);
    assert_eq!(in_place.counts, rebuilt.counts, "{what}: rule counts");
    assert_eq!((in_place.longest, in_place.focus_longest), (rebuilt.longest, rebuilt.focus_longest), "{what}: longest");
    assert_eq!(in_place.selected, rebuilt.selected, "{what}: selection");
    assert_eq!(in_place.index, rebuilt.index, "{what}: filter index");
    assert_eq!(in_place.hint, rebuilt.hint, "{what}: time hint");
    assert_eq!(in_place.parents, rebuilt.parents, "{what}: line counts");
}

pub fn setup(f: &mut Files) {
    f.h.view.update(&mut f.h.cx, |app, cx| {
        let color = log_gpui::palette::next_rule_color(0);
        app.add_highlight_rule("level", &["warn".to_string()], &["user=3".to_string()], color, cx).unwrap();
        app.trigger_search_query("user=4 || ERROR", cx);
        app.on_focus_find_changed("request 1", cx);
        app.filter_toggle_value("level", "error", cx);
    });
}

