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
    let mut space = LogSpace::new("s".into(), "Payment".into(), LogSpaceKind::Imported);
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
