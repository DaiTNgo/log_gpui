//! A local file read through its Line Index (task 4b.2): its entries,
//! read again from the file, are those parsing it whole gives.

use log_gpui::index::file::{IndexedFile, READ_AROUND};
use log_gpui::index::indexing::IndexOptions;
use log_gpui::index::{BytesSource, DiskSource, HandleBudget, LineSource};
use log_gpui::parser::text::LineConfig;
use log_gpui::space_view::ParsedFile;
use std::sync::Arc;

fn text() -> String {
    let mut out = String::new();
    for i in 0..2000 {
        if i % 9 == 4 {
            out += "java.lang.IllegalStateException: boom\n\tat a.b(C.java:12)\n";
        }
        out += &format!("2026-09-05 10:{:02}:{:02}.{:03} [INFO] [api] request {i} user={}\n", i / 60 % 60, i % 60, i % 1000, i % 7);
    }
    out
}

fn debug(e: &log_gpui::model::LogEntry) -> String {
    format!("{e:?}")
}

#[test]
fn entries_read_again_are_those_of_the_whole_file() {
    let text = text();
    let source: Arc<dyn LineSource> = Arc::new(BytesSource::new(text.clone().into_bytes()));
    let file = IndexedFile::index_now(source, IndexOptions::default()).unwrap();
    let want = ParsedFile::from_bytes(text.as_bytes()).entries;
    assert_eq!(file.len(), want.len());
    // Out of order, across block edges, and twice (the second from the cache).
    let order: Vec<usize> = (0..want.len()).rev().chain([READ_AROUND - 1, READ_AROUND, READ_AROUND + 1]).collect();
    for n in order {
        assert_eq!(debug(&file.entry(n).unwrap()), debug(&want[n]), "line {n}");
    }
    assert!(file.entry(want.len()).is_none());
    assert!(file.meta(5).is_some());
}

#[test]
fn detected_fields_are_read_with_the_lines() {
    let text = text();
    let config = Arc::new(LineConfig::new(&["user".to_string()]));
    let options = IndexOptions { config: config.clone(), ..Default::default() };
    let source: Arc<dyn LineSource> = Arc::new(BytesSource::new(text.clone().into_bytes()));
    let file = IndexedFile::index_now(source, options).unwrap();
    let mut want = ParsedFile::from_bytes(text.as_bytes()).entries;
    config.apply(&mut want);
    for n in [0, 7, 500, want.len() - 1] {
        assert_eq!(debug(&file.entry(n).unwrap()), debug(&want[n]), "line {n}");
    }
    let user = file.indexed.data.read().unwrap().filter.column("user").map(|c| c.values.len());
    assert_eq!(user, Some(7), "the Filter column of the Detected Field");
}

#[test]
fn the_cache_keeps_a_few_screens_only() {
    let text = text();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.log");
    std::fs::write(&path, &text).unwrap();
    let source: Arc<dyn LineSource> = Arc::new(DiskSource::stay_open(&path, HandleBudget::new(4)).unwrap());
    let file = IndexedFile::index_now(source, IndexOptions::default()).unwrap();
    for n in 0..file.len() {
        file.entry(n).unwrap();
    }
    assert!(file.cached() <= log_gpui::index::view_rows::ROW_CACHE * 5 / 4, "{} kept", file.cached());
}

#[test]
fn the_raw_view_reads_an_indexed_file_s_lines() {
    use log_gpui::raw_view::RawLines;
    let text = text();
    let source: Arc<dyn LineSource> = Arc::new(BytesSource::new(text.clone().into_bytes()));
    let file = Arc::new(IndexedFile::index_now(source, IndexOptions::default()).unwrap());
    let held = ParsedFile::from_bytes(text.as_bytes()).lines;
    let lines = RawLines::Indexed(file.clone());
    assert_eq!(lines.len(), held.len());
    assert_eq!(lines.get(7), held.get(7).cloned(), "one line, read from the file");
    let all = lines.all();
    assert_eq!(*all, *held, "every line");
    assert!(Arc::ptr_eq(&all, &lines.all()), "read once, then kept");
    assert_eq!(lines.get(9), held.get(9).cloned());
}
