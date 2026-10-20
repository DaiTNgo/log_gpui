//! The Live Window dropping the oldest lines of a Remote File takes their rows
//! out of the open view in place, and leaves what building the view again
//! from every file would.
mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::AppView;
use log_gpui::app::live::LiveSourceState;
use log_gpui::live::{SourceEvent, SourceHandle, SourceStatus};
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

/// Line `i` of file `name`: a second apart, `step` seconds between files'
/// lines so two files interleave. Every eighth is followed by a Continuation
/// Line.
fn lines(name: &str, range: std::ops::Range<usize>, offset: usize) -> Vec<String> {
    let levels = ["INFO", "ERROR", "WARN", "INFO", "DEBUG"];
    let mut out = Vec::new();
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

struct Files {
    h: Harness,
    senders: Vec<(String, async_channel::Sender<SourceEvent>)>,
}

/// An Imported Log Space of one file per name, read through fake Remote File
/// sources the test sends lines to.
fn open(cx: &mut TestAppContext, names: &[(&str, usize)], count: usize, window: usize) -> Files {
    let mut h = common::open_app(cx);
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
    fn send(&self, name: &str, lines: Vec<String>) {
        let bytes = lines.iter().map(|l| l.len() as u32 + 1).collect();
        let (_, tx) = self.senders.iter().find(|(n, _)| n == name).unwrap();
        tx.try_send(SourceEvent::Append { lines, bytes, streams: None }).unwrap();
    }

    fn pump(&mut self) {
        self.h.view.update(&mut self.h.cx, |app, cx| app.pump_live(cx));
    }
}

#[derive(Debug, PartialEq)]
struct View {
    rows: Vec<(String, usize, u32, Option<i64>, bool)>,
    pass: Vec<usize>,
    paints: Vec<Option<usize>>,
    counts: Vec<(u64, usize)>,
    matches: Vec<usize>,
    focus: Vec<log_gpui::focus::FocusRow>,
    hits: Vec<usize>,
    longest: usize,
    focus_longest: usize,
    selected: Option<(String, usize)>,
    index: Vec<(String, Vec<(String, usize)>)>,
    times: Vec<Option<i64>>,
    hint: Option<(String, String)>,
    parents: (usize, usize),
}

fn view(app: &AppView) -> View {
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

fn check_against_rebuild(f: &mut Files, what: &str) {
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

fn setup(f: &mut Files) {
    f.h.view.update(&mut f.h.cx, |app, cx| {
        let color = log_gpui::palette::next_rule_color(0);
        app.add_highlight_rule("level", &["warn".to_string()], &["user=3".to_string()], color, cx).unwrap();
        app.trigger_search_query("user=4 || ERROR", cx);
        app.on_focus_find_changed("request 1", cx);
        app.filter_toggle_value("level", "error", cx);
    });
}

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
