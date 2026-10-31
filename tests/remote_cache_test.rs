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
        .skip(cache.meta().generations.first().and_then(|g| g.segments.first()).map_or(0, |s| s.skip_lines))
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
    assert_eq!(dropped, 25, "two whole segments, and five lines of the third");
    assert_eq!(cache.segment_paths().len(), 4);
    assert_eq!(cache.meta().generations[0].first_line_no, 26);
    assert_eq!(cache.trim_to(30).unwrap(), 0);
    // Read from after the lines that left, as one source too.
    let (chain, _) = cache.chain(log_gpui::index::HandleBudget::new(8)).unwrap();
    use log_gpui::index::LineSource as _;
    let text = String::from_utf8(chain.read_range(0, chain.len().unwrap() as usize).unwrap()).unwrap();
    assert_eq!(text.lines().map(str::to_string).collect::<Vec<_>>(), lines(25..55));
    drop(cache);
    let cache = RemoteCache::open(&dir, "srv-1", target(), 10).unwrap();
    assert_eq!(cache.lines(), 30, "opened again");
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

/// Lines held from the middle of a file on the Server keep their numbers
/// there (task 2.1).
#[test]
fn a_generation_held_from_its_middle_numbers_its_lines_as_on_the_server() {
    use log_gpui::index::file::{GenerationStart, IndexedFile};
    use log_gpui::index::{HandleBudget, LineSource};
    use log_gpui::view_data::FileEntries;
    use std::sync::Arc;
    let root = tempfile::tempdir().unwrap();
    let dir = dir_for(root.path(), "srv-1", &target());
    let mut cache = RemoteCache::open(&dir, "srv-1", target(), 50).unwrap();
    let line = |i: usize| format!("2026-09-05 10:00:00 [INFO] [api] line {i}");
    cache.begin_generation(4001).unwrap();
    cache.append(&refs(&(4001..4011).map(line).collect::<Vec<_>>())).unwrap();
    cache.begin_generation(1).unwrap();
    cache.append(&refs(&(1..4).map(line).collect::<Vec<_>>())).unwrap();
    let (chain, starts) = cache.chain(HandleBudget::new(8)).unwrap();
    let file = IndexedFile::index_now(Arc::new(chain) as Arc<dyn LineSource>, Default::default()).unwrap();
    file.set_generations(
        starts
            .iter()
            .enumerate()
            .map(|(g, &(pos, first))| GenerationStart { pos, first_number: first as usize, generation: g as u32, mark: None })
            .collect(),
    );
    assert_eq!(file.entry(0).unwrap().index, 4001);
    assert_eq!(file.line_number(9), Some((4010, 0)));
    assert_eq!(file.line_number(10), Some((1, 1)), "the next Generation starts at 1");
    let entries = FileEntries::Indexed(&file);
    assert_eq!(entries.position_of(4005, 0), Some(4));
    assert_eq!(entries.position_of(2, 1), Some(11));
    assert_eq!(entries.position_of(4000, 0), None, "not held");
}

// ── Cleanup (task 8) ────────────────────────────────────────────────────────

use log_gpui::remote::cache;
use std::collections::HashSet;

/// A cache of `n` lines of about 100 bytes for `path`, last opened at `at`.
fn cache_of(root: &std::path::Path, path: &str, n: usize, at: u64) -> std::path::PathBuf {
    let target = CacheTarget::File { path: path.into() };
    let dir = dir_for(root, "srv-1", &target);
    let mut c = RemoteCache::open(&dir, "srv-1", target, 1_000).unwrap();
    let l: Vec<String> = (0..n).map(|i| format!("{i:>99}")).collect();
    c.append(&refs(&l)).unwrap();
    drop(c);
    let meta_path = dir.join(cache::META_FILE);
    let mut meta: serde_json::Value = serde_json::from_slice(&std::fs::read(&meta_path).unwrap()).unwrap();
    meta["last_opened"] = at.into();
    std::fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();
    dir
}

#[test]
fn the_sweep_removes_caches_nobody_refers_to_and_those_not_opened_for_long() {
    let root = tempfile::tempdir().unwrap();
    let kept = cache_of(root.path(), "/a.log", 10, 1_000);
    let unused = cache_of(root.path(), "/b.log", 10, 1_000);
    let old = cache_of(root.path(), "/c.log", 10, 10);
    let in_use: HashSet<_> = [kept.clone(), old.clone()].into();
    assert_eq!(cache::sweep(root.path(), &in_use, None), 1);
    assert!(kept.is_dir() && old.is_dir() && !unused.exists());
    assert_eq!(cache::sweep(root.path(), &in_use, Some(500)), 1, "not opened since 500");
    assert!(kept.is_dir() && !old.exists());
    // No file refers to any: the Server's folder goes too.
    cache::sweep(root.path(), &HashSet::new(), None);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn a_folder_that_is_not_a_cache_is_never_removed() {
    let root = tempfile::tempdir().unwrap();
    let dir = cache_of(root.path(), "/a.log", 10, 1_000);
    std::fs::write(dir.join("notes.txt"), "mine").unwrap();
    assert!(cache::remove(&dir).is_err());
    assert_eq!(cache::sweep(root.path(), &HashSet::new(), None), 0);
    assert!(dir.join("notes.txt").exists());
}

#[test]
fn a_cache_another_app_window_uses_is_left() {
    let root = tempfile::tempdir().unwrap();
    let dir = cache_of(root.path(), "/a.log", 10, 1_000);
    // pid 1 runs on every Unix.
    std::fs::write(dir.join("lock"), "1").unwrap();
    assert_eq!(cache::sweep(root.path(), &HashSet::new(), None), 0);
    assert!(dir.is_dir());
}

#[test]
fn over_the_limit_the_cache_opened_longest_ago_goes_first_but_not_an_open_one() {
    let root = tempfile::tempdir().unwrap();
    // About 100 KB each.
    let open = cache_of(root.path(), "/open.log", 1_000, 1);
    let oldest = cache_of(root.path(), "/oldest.log", 1_000, 2);
    let middle = cache_of(root.path(), "/middle.log", 1_000, 3);
    let newest = cache_of(root.path(), "/newest.log", 1_000, 4);
    let keep: HashSet<_> = [open.clone()].into();
    let t = cache::trim_to_size(root.path(), 250_000, &keep);
    assert_eq!(t.removed, 2);
    assert!(open.is_dir() && !oldest.exists() && !middle.exists() && newest.is_dir());
    assert!(t.bytes <= 250_000 && t.bytes == cache::total_size(root.path()));
    // Only the open one would have to go: it stays, over the limit.
    let t = cache::trim_to_size(root.path(), 1, &[open.clone(), newest.clone()].into());
    assert_eq!(t.removed, 0);
    assert!(t.bytes > 1 && open.is_dir());
}

#[test]
fn a_cache_with_no_meta_is_listed_and_removed_like_any() {
    let root = tempfile::tempdir().unwrap();
    let dir = cache_of(root.path(), "/a.log", 10, 1_000);
    std::fs::remove_file(dir.join(cache::META_FILE)).unwrap();
    let found = cache::list(root.path());
    assert_eq!((found.len(), found[0].last_opened), (1, 0));
    assert!(!cache::has_lines(&dir), "read as no cache");
    assert_eq!(cache::trim_to_size(root.path(), 0, &HashSet::new()).removed, 1);
}

/// The output of each line of a Remote Command is kept beside its segment
/// and given to the rows read through the Line Index (task 2.2).
#[test]
fn a_commands_streams_are_kept_per_line_through_the_window_cut_and_reopening() {
    use log_gpui::index::file::IndexedFile;
    use log_gpui::index::meta::Runs;
    use log_gpui::index::{HandleBudget, LineSource};
    use log_gpui::model::Stream;
    use std::sync::Arc;
    let root = tempfile::tempdir().unwrap();
    let target = CacheTarget::Command { command: "remote-cmd:srv-1:1".into() };
    let dir = dir_for(root.path(), "cmd", &target);
    let mut cache = RemoteCache::open(&dir, "cmd", target.clone(), 7).unwrap();
    let text = lines(0..30);
    // Every third line on stderr.
    let codes: Vec<u8> = (0..30).map(|i| if i % 3 == 0 { 2 } else { 1 }).collect();
    cache.append_streams(&refs(&text[..10]), &[], &codes[..10]).unwrap();
    cache.append_streams(&refs(&text[10..]), &[], &codes[10..]).unwrap();
    assert_eq!(cache.streams(), codes);
    cache.drop_top(9).unwrap();
    assert_eq!(cache.streams(), codes[9..]);
    cache.drop_bottom(2).unwrap();
    assert_eq!(cache.streams(), codes[9..28]);
    drop(cache);
    let cache = RemoteCache::open(&dir, "cmd", target, 7).unwrap();
    assert_eq!(cache.streams(), codes[9..28], "read again");

    let (chain, _) = cache.chain(HandleBudget::new(8)).unwrap();
    let file = IndexedFile::index_now(Arc::new(chain) as Arc<dyn LineSource>, Default::default()).unwrap();
    let mut runs = Runs::default();
    for (i, code) in cache.streams().iter().enumerate() {
        runs.set(i, log_gpui::remote::cache::stream_of(*code));
    }
    file.set_streams(runs);
    for i in 0..file.len() {
        let want = if (i + 9) % 3 == 0 { Stream::Stderr } else { Stream::Stdout };
        assert_eq!(file.entry(i).unwrap().stream, Some(want), "line {i}");
    }
    let mut seen = Vec::new();
    file.each_entry_at(&[0, 1, 5], |n, e, _| seen.push((n, e.stream))).unwrap();
    assert_eq!(seen, [(0, Some(Stream::Stderr)), (1, Some(Stream::Stdout)), (5, Some(Stream::Stdout))]);
}
