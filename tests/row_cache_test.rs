//! The app's Row Cache (memory-budget, group 2): one cache for every file,
//! bounded in bytes, not 4096 rows per file.

use log_gpui::index::file::IndexedFile;
use log_gpui::index::indexing::IndexOptions;
use log_gpui::index::view_rows::{RowCache, SharedRowCache, row_weight, rows};
use log_gpui::index::{BytesSource, LineSource};
use std::sync::Arc;

fn file(lines: usize, width: usize) -> IndexedFile {
    let mut text = String::new();
    for i in 0..lines {
        text += &format!("2026-09-05 10:00:00.000 [INFO] line {i} {}\n", "x".repeat(width));
    }
    let source: Arc<dyn LineSource> = Arc::new(BytesSource::new(text.into_bytes()));
    IndexedFile::index_now(source, IndexOptions::default()).unwrap()
}

fn entry() -> Arc<log_gpui::model::LogEntry> {
    let a = file(1, 0);
    a.entry(0).unwrap()
}

#[test]
fn every_file_has_its_own_key() {
    let a = file(10, 0);
    let b = file(10, 0);
    assert_ne!(a.file_key(), b.file_key());
    let again = a.read_with(IndexOptions::default());
    assert_ne!(again.file_key(), a.file_key(), "read with other options: other rows");
}

#[test]
fn the_cache_keeps_to_its_bytes() {
    let mut cache = RowCache::new(1000);
    let e = entry();
    for i in 0..100 {
        cache.insert_weighed((1, 0, i), e.clone(), 100);
    }
    assert!(cache.weight() <= 1250, "{} kept", cache.weight());
    assert!(cache.contains((1, 0, 99)), "the last kept");
    assert!(!cache.contains((1, 0, 0)), "the first gone");
    // Keeping a line again does not count it twice.
    cache.insert_weighed((1, 0, 99), e.clone(), 100);
    let (n, w) = cache.of_file(1);
    assert_eq!(w, n * 100);
}

#[test]
fn forgetting_a_file_keeps_the_others() {
    let shared = SharedRowCache::new(16 << 20);
    let e = entry();
    for i in 0..10 {
        shared.shard(3).insert_weighed((3, 0, i), e.clone(), 10);
        shared.shard(19).insert_weighed((19, 0, i), e.clone(), 10);
    }
    shared.forget(3);
    assert_eq!(shared.shard(3).of_file(3), (0, 0));
    assert_eq!(shared.shard(19).of_file(19), (10, 100), "same shard, other file");
    assert_eq!(shared.shard(19).missing(19, 0, &[0, 5, 10, 11]), vec![10..12]);
}

#[test]
fn many_files_take_no_more_than_one_budget() {
    let shared_before = rows().weight();
    let files: Vec<IndexedFile> = (0..50).map(|_| file(5000, 80)).collect();
    for f in &files {
        for n in 0..f.len() {
            f.entry(n).unwrap();
        }
    }
    let kept: usize = files.iter().map(|f| f.cached_weight()).sum();
    let budget = log_gpui::index::view_rows::ROW_CACHE_BYTES;
    assert!(kept <= budget * 5 / 4, "{kept} bytes kept for 50 files (before: {shared_before})");
    assert!(kept < 50 * 5000 * row_weight(110), "rows went");
}

#[test]
fn a_file_dropped_leaves_no_rows() {
    let f = file(500, 0);
    for n in 0..f.len() {
        f.entry(n).unwrap();
    }
    let key = f.file_key();
    assert!(rows().shard(key).of_file(key).0 > 0);
    drop(f);
    assert_eq!(rows().shard(key).of_file(key), (0, 0));
}
