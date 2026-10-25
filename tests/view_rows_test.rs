//! The rows of a view (openspec change `line-offset-index`, tasks 4.1–4.2):
//! the incremental merge gives `timeline::merge`'s order, and the cache of
//! parsed rows.

use log_gpui::index::view_rows::{Lane, Merger, RowCache, ViewRows, pack, unpack};
use log_gpui::index::{BytesSource, FileIndex, index_source};
use log_gpui::model::LogEntry;
use log_gpui::parser::text::parse_entries;
use log_gpui::timeline;

fn index(text: &str) -> FileIndex {
    index_source(&BytesSource::new(text.as_bytes().to_vec()), None).unwrap()
}

/// The rows `timeline::merge` gives, packed.
fn reference(texts: &[&str]) -> Vec<u64> {
    let entries: Vec<Vec<LogEntry>> = texts.iter().map(|t| parse_entries(t)).collect();
    let lists: Vec<&[LogEntry]> = entries.iter().map(Vec::as_slice).collect();
    timeline::merge(&lists).rows.iter().map(|r| pack(r.file as usize, r.line as usize)).collect()
}

fn merge_whole(texts: &[&str]) -> Vec<u64> {
    let indexes: Vec<FileIndex> = texts.iter().map(|t| index(t)).collect();
    let lanes: Vec<Lane> = indexes.iter().map(|i| Lane { meta: i.meta.as_slice(), done: true }).collect();
    let mut merger = Merger::new(lanes.len());
    let mut out = Vec::new();
    merger.advance(&lanes, &mut out);
    assert!(merger.is_done());
    out
}

/// The same merge with the files indexed a few lines at a time, unevenly.
fn merge_growing(texts: &[&str]) -> Vec<u64> {
    let indexes: Vec<FileIndex> = texts.iter().map(|t| index(t)).collect();
    let mut merger = Merger::new(indexes.len());
    let mut out = Vec::new();
    let mut known: Vec<usize> = vec![0; indexes.len()];
    let mut step = 0;
    loop {
        step += 1;
        for (f, k) in known.iter_mut().enumerate() {
            *k = (*k + (step * 7 + f * 3) % 5).min(indexes[f].len());
        }
        let lanes: Vec<Lane> = indexes
            .iter()
            .zip(&known)
            .map(|(i, &k)| Lane { meta: &i.meta.as_slice()[..k], done: k == i.len() })
            .collect();
        merger.advance(&lanes, &mut out);
        if merger.is_done() {
            return out;
        }
        assert!(step < 10_000, "the merge ends");
    }
}

fn check(name: &str, texts: &[&str]) {
    let want = reference(texts);
    assert_eq!(merge_whole(texts), want, "{name}: whole files");
    assert_eq!(merge_growing(texts), want, "{name}: files growing");
}

#[test]
fn two_files_interleave() {
    check(
        "interleave",
        &["2026-09-05 10:00:01 [INFO] [api] a\n2026-09-05 10:00:03 [INFO] [api] c\n", "2026-09-05 10:00:02 [INFO] [w] b\n"],
    );
}

#[test]
fn the_same_timestamp_keeps_file_order() {
    check(
        "same time",
        &["2026-09-05 10:00:01 [INFO] [api] a\n", "2026-09-05 10:00:01 [INFO] [w] b\n", "2026-09-05 10:00:01 [INFO] [x] c\n"],
    );
}

#[test]
fn a_stack_trace_stays_below_its_line() {
    let api = "2026-09-05 10:00:01 [ERROR] [api] boom\n  at a\n  at b\n  at c\n  at d\n  at e\n2026-09-05 10:00:05 [INFO] [api] after\n";
    let worker = "2026-09-05 10:00:01 [INFO] [w] same second\n2026-09-05 10:00:02 [INFO] [w] next\n";
    check("stack trace", &[api, worker]);
    check("stack trace second", &[worker, api]);
}

#[test]
fn a_file_without_timestamps_comes_last() {
    let notes = "just notes\nmore notes\n";
    let api = "head line\n2026-09-05 10:00:01 [INFO] [api] a\n2026-09-05 10:00:00 [INFO] [api] earlier\n";
    check("no timestamp", &[notes, api, "2026-09-05 09:00:00 [INFO] [w] w\n"]);
    check("only untimed", &[notes, "x\n"]);
    check("empty files", &["", api, ""]);
}

#[test]
fn many_files_out_of_order() {
    let mut files = Vec::new();
    for f in 0..4 {
        let mut text = String::new();
        for i in 0..300 {
            let s = (i * 7 + f * 13) % 50;
            text += &format!("2026-09-05 10:00:{s:02} [INFO] [f{f}] line {i}\n");
            if i % 17 == 0 {
                text += "  continued\n";
            }
        }
        files.push(text);
    }
    let texts: Vec<&str> = files.iter().map(String::as_str).collect();
    check("many", &texts);
}

#[test]
fn view_rows_of_a_file_and_of_a_space() {
    let file = ViewRows::File { lines: 3 };
    assert_eq!((file.len(), file.get(2), file.get(3)), (3, Some((0, 2)), None));
    assert_eq!(file.row_of(0, 1), Some(1));
    let space = ViewRows::Merged(vec![pack(1, 0), pack(0, 0), pack(0, 1)]);
    assert_eq!(space.get(0), Some((1, 0)));
    assert_eq!(space.row_of(0, 1), Some(2));
    assert_eq!(unpack(pack(65535, (1 << 48) - 1)), (65535, (1 << 48) - 1));
}

#[test]
fn the_row_cache_keeps_the_rows_last_used() {
    let entries = parse_entries("a\nb\nc\nd\ne\n");
    let mut cache = RowCache::new(4);
    for (i, e) in entries.iter().enumerate() {
        cache.insert((0, 0, i), e.clone());
    }
    assert_eq!(cache.len(), 5, "a quarter over the capacity before it trims");
    cache.get((0, 0, 0));
    cache.insert((1, 0, 0), entries[0].clone());
    assert_eq!(cache.len(), 4);
    assert!(cache.contains((0, 0, 0)), "used last, so kept");
    assert!(!cache.contains((0, 0, 1)), "the oldest went");
    assert_eq!(cache.missing(0, 0, &[0, 1, 2, 3, 4, 7]), vec![1..3, 7..8]);
    cache.forget_file(0);
    assert_eq!(cache.len(), 1);
}
