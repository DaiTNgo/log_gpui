//! A view with files read through their Line Index (task 4b.3) shows what
//! the same view of held files shows: the same rows in the same order, the
//! same Filter columns, values and counts.

use log_gpui::filter_index::FilterIndex;
use log_gpui::index::file::IndexedFile;
use log_gpui::index::indexing::IndexOptions;
use log_gpui::index::{BytesSource, LineSource};
use log_gpui::space_view::{ParsedFile, build_file_view, build_space_view};
use log_gpui::workspace::log_space::{LogSpace, LogSpaceKind};
use std::collections::HashMap;
use std::sync::Arc;

fn indexed(text: &str) -> ParsedFile {
    let source: Arc<dyn LineSource> = Arc::new(BytesSource::new(text.as_bytes().to_vec()));
    ParsedFile::from_indexed(IndexedFile::index_now(source, IndexOptions::default()).unwrap())
}

fn describe(f: &FilterIndex) -> String {
    let mut out = format!("rows {} parents {} timed {} hint {:?}\n", f.rows(), f.parent_lines(), f.timed_lines(), f.time_hint());
    for row in 0..f.rows() {
        out += &format!("ts {row} {:?}\n", f.effective_timestamp(row));
    }
    for c in &f.columns {
        out += &format!("col {} overflow {} covered {} values {:?}\n", c.key, c.overflow, c.covered(), c.values);
        let ids: Vec<_> = (0..f.rows()).map(|r| c.id_of_row(r)).collect();
        out += &format!("  {ids:?}\n");
    }
    out
}

fn texts() -> Vec<(&'static str, String)> {
    let api: String = (0..300)
        .map(|i| {
            let mut line = format!("2026-09-05 10:00:{:02}.{:03} [INFO] [api] request {i}\n", i % 60, i);
            if i % 40 == 3 {
                line += "  at a stack line\n";
            }
            line
        })
        .collect();
    let worker: String = (0..200).map(|i| format!("2026-09-05 10:00:{:02}.{:03} [WARN] [worker] job {i}\n", (i * 7) % 60, i)).collect();
    let json = r#"{"time":"2026-09-05T10:00:00Z","level":"info","msg":"x","region":"eu"}"#.to_string() + "\n";
    vec![("/api.log", api), ("/worker.log", worker), ("/notes.txt", "just notes\nmore\n".into()), ("/j.log", json)]
}

fn space(files: &[(&str, String)]) -> LogSpace {
    let mut space = LogSpace::new("s".into(), "Payment".into(), LogSpaceKind::Static);
    let paths: Vec<String> = files.iter().map(|(p, _)| p.to_string()).collect();
    space.add_imported_files(&paths).unwrap();
    space
}

#[test]
fn a_log_space_with_indexed_files_shows_the_same_rows() {
    let files = texts();
    let space = space(&files);
    let held: HashMap<String, ParsedFile> = files.iter().map(|(p, t)| (p.to_string(), ParsedFile::from_text(t))).collect();
    let want = build_space_view(&space, &held);
    // Every file indexed, and only some.
    for indexed_files in [vec![0, 1, 2, 3], vec![1], vec![0, 2]] {
        let mixed: HashMap<String, ParsedFile> = files
            .iter()
            .enumerate()
            .map(|(i, (p, t))| (p.to_string(), if indexed_files.contains(&i) { indexed(t) } else { ParsedFile::from_text(t) }))
            .collect();
        let got = build_space_view(&space, &mixed);
        let rows = |v: &log_gpui::space_view::BuiltView, p| -> Vec<String> {
            v.materialize(p).iter().map(|e| format!("{e:?}")).collect()
        };
        assert_eq!(rows(&got, &mixed), rows(&want, &held), "{indexed_files:?}: rows");
        assert_eq!(describe(&got.index), describe(&want.index), "{indexed_files:?}: Filter");
        assert_eq!(got.no_timestamp_files, want.no_timestamp_files);
        assert_eq!(got.max_chars, want.max_chars);
    }
}

#[test]
fn a_file_view_of_an_indexed_file_shows_the_same_rows() {
    let files = texts();
    let space = space(&files);
    for (path, text) in &files {
        let file = space.file(path).unwrap();
        let held = ParsedFile::from_text(text);
        let want = build_file_view(file, &held);
        let got_file = indexed(text);
        let got = build_file_view(file, &got_file);
        let p_held: HashMap<String, ParsedFile> = [(path.to_string(), held)].into();
        let p_got: HashMap<String, ParsedFile> = [(path.to_string(), got_file)].into();
        let rows = |v: &log_gpui::space_view::BuiltView, p| -> Vec<String> {
            v.materialize(p).iter().map(|e| format!("{e:?}")).collect()
        };
        assert_eq!(rows(&got, &p_got), rows(&want, &p_held), "{path}");
        assert_eq!(describe(&got.index), describe(&want.index), "{path}");
        assert_eq!(got.max_chars, want.max_chars, "{path}");
    }
}

#[test]
fn a_scan_reads_indexed_files_from_their_source() {
    use log_gpui::index::scan::{Matcher, scan_all};
    let files = texts();
    let space = space(&files);
    let held: HashMap<String, ParsedFile> = files.iter().map(|(p, t)| (p.to_string(), ParsedFile::from_text(t))).collect();
    let mixed: HashMap<String, ParsedFile> =
        files.iter().enumerate().map(|(i, (p, t))| (p.to_string(), if i % 2 == 0 { indexed(t) } else { ParsedFile::from_text(t) })).collect();
    let matcher: Arc<Matcher> = Arc::new(|first: usize, lines: &[&str]| {
        lines.iter().enumerate().filter(|(_, l)| l.contains("job 1") || l.contains("stack")).map(|(i, _)| (first + i) as u32).collect()
    });
    let find = |p: &HashMap<String, ParsedFile>| {
        let view = build_space_view(&space, p);
        let texts = view.view.texts(p);
        scan_all(&texts, 0..view.view.len(), &*matcher).lines
    };
    let want = find(&held);
    assert!(!want.is_empty());
    assert_eq!(find(&mixed), want);
}

#[test]
fn merged_filter_columns_are_those_of_a_build_on_the_merged_rows() {
    use log_gpui::model::LogEntry;
    let mut files = texts();
    // The same value spelt two ways in two files, and a column with too many
    // values in one of them.
    files.push(("/caps.log", (0..50).map(|i| format!("2026-09-05 10:00:{:02} [info] [API] caps {i}\n", i % 60)).collect()));
    files.push((
        "/many.log",
        (0..700).map(|i| format!("2026-09-05 10:01:{:02} [ERROR] [t{i}] many {i}\n", i % 60)).collect(),
    ));
    let space = space(&files);
    let held: HashMap<String, ParsedFile> = files.iter().map(|(p, t)| (p.to_string(), ParsedFile::from_text(t))).collect();
    let view = build_space_view(&space, &held);
    // The old way: every merged row, copied, built from scratch.
    let ordered = space.ordered_files();
    let owned: Vec<Vec<LogEntry>> = ordered.iter().map(|f| held[&f.file_id].entries()).collect();
    let lists: Vec<&[LogEntry]> = owned.iter().map(Vec::as_slice).collect();
    let timeline = log_gpui::timeline::merge(&lists);
    let fields = log_gpui::timeline::union_field_names(&lists);
    let rows = view.materialize(&held);
    let want = FilterIndex::build_merged(&rows, &fields, &timeline.effective_ts);
    assert_eq!(describe(&view.index), describe(&want));
    assert_eq!(view.index.column("module").map(|c| c.overflow), Some(true));
    assert!(view.index.column("level").unwrap().values.iter().any(|v| v.folded == "info" && v.count > 300));
    let mut want_kv: Vec<_> = want.kv_counts().iter().map(|(k, n)| (k.to_string(), *n)).collect();
    let mut got_kv: Vec<_> = view.index.kv_counts().iter().map(|(k, n)| (k.to_string(), *n)).collect();
    want_kv.sort();
    got_kv.sort();
    assert_eq!(got_kv, want_kv);
    assert_eq!(view.index.anchor_day(), want.anchor_day());
}

#[test]
fn what_a_row_tells_without_its_entry_is_what_its_entry_says() {
    let files = texts();
    let space = space(&files);
    let held: HashMap<String, ParsedFile> = files.iter().map(|(p, t)| (p.to_string(), ParsedFile::from_text(t))).collect();
    let mixed: HashMap<String, ParsedFile> = files.iter().map(|(p, t)| (p.to_string(), indexed(t))).collect();
    let (want, got) = (build_space_view(&space, &held), build_space_view(&space, &mixed));
    let (want_rows, got_rows) = (want.view.bind(&held), got.view.bind(&mixed));
    for r in 0..want.view.len() {
        assert_eq!(got_rows.line_number(r), want_rows.line_number(r), "row {r}");
        assert_eq!(got_rows.has_time(r), want_rows.has_time(r), "row {r}");
        assert_eq!(got_rows.own_time(r), want_rows.own_time(r), "row {r}");
        assert_eq!(got_rows.level(r), want_rows.level(r), "row {r}");
    }
}
