//! The Log Name Column (openspec change `add-swimlane`): the files of one
//! log (`api.log`, `api.log.1`) share a value, which filters like any other.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::column::PinnableColumn;
use log_gpui::filter_index::FilterIndex;
use log_gpui::model::LogEntry;
use log_gpui::space_view::ParsedFile;
use log_gpui::timeline;
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

fn lines(tag: &str, n: usize, second: usize) -> String {
    (0..n).map(|i| format!("2026-10-19 10:00:{:02} INFO [{tag}] line {i}\n", (second + i) % 60)).collect()
}

fn entries(text: &str) -> Vec<LogEntry> {
    ParsedFile::from_text(text).entries()
}

fn merged(named: &[(&str, &[LogEntry])]) -> FilterIndex {
    let files: Vec<&[LogEntry]> = named.iter().map(|(_, e)| *e).collect();
    let timeline = timeline::merge(&files);
    let named: Vec<(&str, &str, &[LogEntry])> = named.iter().map(|(n, e)| (*n, *n, *e)).collect();
    let rows = timeline::materialize(&named, &timeline);
    FilterIndex::build_merged(&rows, &timeline::union_field_names(&files), &timeline.effective_ts)
}

fn values(index: &FilterIndex, key: &str) -> Vec<(String, usize)> {
    index.column(key).map_or(Vec::new(), |c| c.values.iter().map(|v| (v.display.clone(), v.count)).collect())
}

#[test]
fn a_new_piece_of_a_split_file_joins_its_log_name() {
    let first = entries(&lines("A", 3, 0));
    let second = entries(&lines("A", 2, 10));
    let one = merged(&[("message.0", &first)]);
    assert_eq!(values(&one, "log_name"), [("message".to_string(), 3)]);
    let both = merged(&[("message.0", &first), ("message.1", &second)]);
    assert_eq!(values(&both, "log_name"), [("message".to_string(), 5)], "one value, the lines of both files");
    assert_eq!(values(&both, "source").len(), 2, "the Source Column still tells them apart");
}

fn space(h: &mut Harness, files: &[(&str, String)]) {
    let sources: Vec<ImportSource> = files
        .iter()
        .map(|(name, text)| {
            let path = h.dir.path().join(name);
            std::fs::write(&path, text).unwrap();
            ImportSource::Disk(path.to_string_lossy().into_owned())
        })
        .collect();
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("S", LogSpaceKind::Static, &sources, cx));
    h.cx.run_until_parked();
}

/// The Source Files of the rows passing the Filter.
fn passing_sources(h: &mut Harness) -> Vec<String> {
    h.view.read_with(&h.cx, |app, _| {
        let rows = app.rows();
        let mut names: Vec<String> = app.filter_pass.iter().filter_map(|r| rows.source(r).map(str::to_string)).collect();
        names.sort();
        names.dedup();
        names
    })
}

#[gpui::test]
fn ticking_a_log_name_keeps_every_file_of_it(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    space(&mut h, &[("api.log", lines("API", 4, 0)), ("api.log.1", lines("API", 4, 20)), ("db.log", lines("DB", 4, 40))]);
    let names = h.view.read_with(&h.cx, |app, _| values(&app.filter_index.read(&app.view.rows), "log_name"));
    assert_eq!(names.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["api.log", "db.log"]);
    h.view.update(&mut h.cx, |app, cx| {
        app.pick_filter_column("log_name", cx);
        app.filter_toggle_value("log_name", "api.log", cx);
    });
    h.cx.run_until_parked();
    let sources = passing_sources(&mut h);
    assert!(sources.iter().any(|s| s.ends_with("api.log")) && sources.iter().any(|s| s.ends_with("api.log.1")), "{sources:?}");
    assert!(!sources.iter().any(|s| s.ends_with("db.log")), "{sources:?}");
}

#[gpui::test]
fn the_log_name_column_starts_hidden_and_hiding_it_keeps_the_filter(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    space(&mut h, &[("api.log", lines("API", 4, 0)), ("api.log.1", lines("API", 4, 20)), ("db.log", lines("DB", 4, 40))]);
    let laid_out = |h: &mut Harness| h.view.read_with(&h.cx, |app, _| app.column_layout.columns().contains(&PinnableColumn::LogName));
    assert!(!laid_out(&mut h), "hidden until turned on");
    let row = h.view.read_with(&h.cx, |app, _| app.column_groups("").built_in.into_iter().find(|r| r.key == "log_name"));
    assert!(row.is_some_and(|r| !r.shown && r.switchable), "the Columns section has its switch");

    h.view.update(&mut h.cx, |app, cx| app.set_column_shown("log_name", true, cx));
    h.cx.run_until_parked();
    let order = h.view.read_with(&h.cx, |app, _| app.column_layout.columns());
    let at = |c| order.iter().position(|x| *x == c);
    assert_eq!(at(PinnableColumn::LogName), at(PinnableColumn::Source).map(|i| i + 1), "right after Source: {order:?}");
    assert!(h.view.read_with(&h.cx, |app, _| app.workspace_state.active_project().unwrap().log_spaces[0].log_name_column), "kept by the Log Space");
    h.view.update(&mut h.cx, |app, cx| app.toggle_pin_column(PinnableColumn::LogName, cx));
    assert!(h.view.read_with(&h.cx, |app, _| app.column_layout.is_pinned(PinnableColumn::LogName)), "it can be pinned");

    h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("log_name", "db.log", cx));
    h.view.update(&mut h.cx, |app, cx| app.set_column_shown("log_name", false, cx));
    h.cx.run_until_parked();
    assert!(!laid_out(&mut h));
    let sources = passing_sources(&mut h);
    assert!(sources.len() == 1 && sources[0].ends_with("db.log"), "the Filter still holds: {sources:?}");
}
