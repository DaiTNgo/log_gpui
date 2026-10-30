//! The Remote Cache on disk (openspec change `remote-local-cache`, task
//! 1.1): its segments keep the lines of each Generation in order through
//! appends, lines added above, the Live Window cut and Clear, and read the
//! same after the folder is opened again.

use log_gpui::remote::cache::{CacheTarget, Checkpoint, RemoteCache, dir_for};

fn target() -> CacheTarget {
    CacheTarget::File { path: "/var/log/app.log".into() }
}

fn lines(range: std::ops::Range<usize>) -> Vec<String> {
    range.map(|i| format!("line {i}")).collect()
}

fn refs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

/// Every line held, top to bottom, with its Generation's first number.
fn held(cache: &RemoteCache) -> Vec<String> {
    cache
        .segment_paths()
        .iter()
        .flat_map(|(_, p)| std::fs::read_to_string(p).unwrap().lines().map(str::to_string).collect::<Vec<_>>())
        .collect()
}

#[test]
fn appends_roll_over_into_segments_and_read_back_in_order() {
    let root = tempfile::tempdir().unwrap();
    let dir = dir_for(root.path(), "srv-1", &target());
    let mut cache = RemoteCache::open(&dir, "srv-1", target(), 10).unwrap();
    cache.append(&refs(&lines(0..25))).unwrap();
    cache.append(&refs(&lines(25..32))).unwrap();
    assert_eq!(cache.segment_paths().len(), 4, "10 + 10 + 10 + 2 lines");
    assert_eq!(held(&cache), lines(0..32));
    drop(cache);
    // Opened again: the same lines, from meta.json and the segment files.
    let cache = RemoteCache::open(&dir, "srv-1", target(), 10).unwrap();
    assert_eq!(cache.lines(), 32);
    assert_eq!(held(&cache), lines(0..32));
}

#[test]
fn older_lines_go_above_and_number_the_generation_from_them() {
    let root = tempfile::tempdir().unwrap();
    let dir = dir_for(root.path(), "srv-1", &target());
    let mut cache = RemoteCache::open(&dir, "srv-1", target(), 10).unwrap();
    cache.begin_generation(501).unwrap();
    cache.append(&refs(&lines(500..520))).unwrap();
    cache.prepend(0, &refs(&lines(490..500))).unwrap();
    assert_eq!(held(&cache), lines(490..520));
    assert_eq!(cache.meta().generations[0].first_line_no, 491);

    // A Rotated File above, and a new Generation below.
    cache.add_earlier_generation("app.log.1", 1, &refs(&lines(0..3))).unwrap();
    cache.begin_generation(1).unwrap();
    cache.append(&["new 1", "new 2"]).unwrap();
    let gens = &cache.meta().generations;
    assert_eq!(gens.len(), 3);
    assert_eq!(gens[0].rotated_name.as_deref(), Some("app.log.1"));
    assert_eq!(held(&cache).len(), 3 + 30 + 2);
}

#[test]
fn the_live_window_drops_whole_segments_from_the_top() {
    let root = tempfile::tempdir().unwrap();
    let dir = dir_for(root.path(), "srv-1", &target());
    let mut cache = RemoteCache::open(&dir, "srv-1", target(), 10).unwrap();
    cache.append(&refs(&lines(0..55))).unwrap();
    let dropped = cache.trim_to(30).unwrap();
    assert_eq!(dropped, 20, "two whole segments; 35 lines stay, at least the window");
    assert_eq!(held(&cache), lines(20..55));
    assert_eq!(cache.meta().generations[0].first_line_no, 21);
    assert_eq!(cache.trim_to(30).unwrap(), 0, "dropping one more would leave fewer than the window");
}

#[test]
fn clear_drops_every_line_and_numbers_on() {
    let root = tempfile::tempdir().unwrap();
    let dir = dir_for(root.path(), "srv-1", &target());
    let mut cache = RemoteCache::open(&dir, "srv-1", target(), 10).unwrap();
    cache.append(&refs(&lines(0..15))).unwrap();
    cache.set_checkpoint(Checkpoint { server_end: 999, ..Checkpoint::default() }).unwrap();
    cache.clear().unwrap();
    assert_eq!(cache.lines(), 0);
    let g = &cache.meta().generations[0];
    assert_eq!(g.first_line_no, 16, "the next line keeps its number");
    assert_eq!(g.checkpoint.server_end, 999, "reading goes on from where it was");
    cache.append(&["after"]).unwrap();
    assert_eq!(held(&cache), ["after"]);
}

#[test]
fn a_segment_written_after_the_last_save_is_counted_again() {
    let root = tempfile::tempdir().unwrap();
    let dir = dir_for(root.path(), "srv-1", &target());
    let mut cache = RemoteCache::open(&dir, "srv-1", target(), 100).unwrap();
    cache.append(&refs(&lines(0..5))).unwrap();
    cache.save().unwrap();
    cache.append(&refs(&lines(5..8))).unwrap();
    // The app stops before saving again.
    std::mem::forget(cache);
    std::fs::remove_file(dir.join("lock")).unwrap();
    let cache = RemoteCache::open(&dir, "srv-1", target(), 100).unwrap();
    assert_eq!(cache.lines(), 8);
}

#[test]
fn a_broken_meta_is_a_cache_with_no_lines() {
    let root = tempfile::tempdir().unwrap();
    let dir = dir_for(root.path(), "srv-1", &target());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("meta.json"), "{ not json").unwrap();
    let cache = RemoteCache::open(&dir, "srv-1", target(), 10).unwrap();
    assert_eq!(cache.lines(), 0);
}

#[test]
fn another_running_app_holds_the_cache() {
    let root = tempfile::tempdir().unwrap();
    let dir = dir_for(root.path(), "srv-1", &target());
    std::fs::create_dir_all(&dir).unwrap();
    // pid 1 (launchd or init) always runs.
    std::fs::write(dir.join("lock"), "1").unwrap();
    let err = RemoteCache::open(&dir, "srv-1", target(), 10).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);
}

#[test]
fn files_and_commands_have_their_own_folders() {
    let root = std::path::Path::new("/c");
    let a = dir_for(root, "srv:1", &target());
    let b = dir_for(root, "srv:1", &CacheTarget::Command { command: "kubectl logs -f api".into() });
    assert_ne!(a, b);
    assert!(b.file_name().unwrap().to_string_lossy().starts_with("cmd-"));
    assert_eq!(a.parent().unwrap().file_name().unwrap(), "srv_1");
}

/// The segments read as one source by a Line Index (task 1.2).
#[test]
fn a_line_index_reads_the_segments_end_to_end() {
    use log_gpui::index::file::IndexedFile;
    use log_gpui::index::{HandleBudget, LineSource};
    use std::sync::Arc;
    let root = tempfile::tempdir().unwrap();
    let dir = dir_for(root.path(), "srv-1", &target());
    let mut cache = RemoteCache::open(&dir, "srv-1", target(), 7).unwrap();
    let text: Vec<String> = (0..40).map(|i| format!("2026-09-05 10:00:{:02} [INFO] [api] line {i}", i % 60)).collect();
    cache.begin_generation(101).unwrap();
    cache.append(&refs(&text[20..])).unwrap();
    cache.prepend(0, &refs(&text[..20])).unwrap();
    let (chain, starts) = cache.chain(HandleBudget::new(8)).unwrap();
    assert_eq!(starts, [(0, 81)]);
    let file = IndexedFile::index_now(Arc::new(chain) as Arc<dyn LineSource>, Default::default()).unwrap();
    assert_eq!(file.len(), 40);
    for (i, want) in text.iter().enumerate() {
        assert_eq!(file.line(i).unwrap().as_ref(), want.as_str());
    }
}
