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

#[test]
fn lines_written_since_are_indexed_on() {
    use log_gpui::index::file::Extended;
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("live.log");
    let first = text();
    // The last line is still being written: no line break yet.
    std::fs::write(&path, format!("{first}2026-09-05 11:00:00 [ERROR] [db] cut")).unwrap();
    let source: Arc<dyn LineSource> = Arc::new(DiskSource::stay_open(&path, HandleBudget::new(4)).unwrap());
    let file = IndexedFile::index_now(source, IndexOptions::default()).unwrap();
    let before = file.len();
    assert_eq!(file.extend().unwrap(), Extended::Same, "nothing new");

    // More of the cut line, still without its line break: it waits.
    let mut out = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    out.write_all(b" off").unwrap();
    assert_eq!(file.extend().unwrap(), Extended::Same);

    // Its line break and two more lines (one a stack line).
    out.write_all(b" here\n\tat x.y(Z.java:1)\n2026-09-05 11:00:01 [INFO] [db] next user=3\n").unwrap();
    assert_eq!(file.extend().unwrap(), Extended::Appended { range: before - 1..before + 2, replaced_last: true });
    let all = std::fs::read(&path).unwrap();
    let want = ParsedFile::from_bytes(&all).entries;
    assert_eq!(file.len(), want.len());
    for n in [0, before - 2, before - 1, before, before + 1] {
        assert_eq!(debug(&file.entry(n).unwrap()), debug(&want[n]), "line {n}");
    }
    // The Filter columns are those of the whole file.
    let whole = log_gpui::filter_index::FilterIndex::build(&want);
    let got = file.indexed.data.read().unwrap();
    for key in ["level", "module", "user"] {
        let (a, b) = (got.filter.column(key).map(|c| c.values.clone()), whole.column(key).map(|c| c.values.clone()));
        assert_eq!(a, b, "{key}");
    }
    assert_eq!(got.filter.parent_lines(), whole.parent_lines());
    drop(got);

    // Cut shorter than what was read: told, not read wrongly.
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(10).unwrap();
    assert_eq!(file.extend().unwrap(), Extended::Shrunk);
}
