//! A local Live file honours the Live Window like a Remote File, reads new
//! data in chunks, and Load More still reaches the lines that left.
mod common;

use common::fake_live::{check_against_rebuild, lines, open, setup};
use gpui::TestAppContext;
use log_gpui::live::local::CHUNK_BYTES;
use log_gpui::live::{LineSource, LineSplitter, LocalFileSource, SourceCommand, SourceEvent};
use std::io::Write;
use std::time::{Duration, Instant};

fn next(handle: &log_gpui::live::SourceHandle, mut want: impl FnMut(&SourceEvent) -> bool) -> SourceEvent {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match handle.events.try_recv() {
            Ok(e) if want(&e) => return e,
            Ok(_) => {}
            Err(_) => {
                assert!(Instant::now() < deadline, "timed out");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

fn big_text(bytes: usize) -> String {
    let mut s = String::new();
    let mut i = 0;
    while s.len() < bytes {
        s.push_str(&format!("2026-10-19 10:00:00.000 INFO line {i} {}\n", "x".repeat(60)));
        i += 1;
    }
    s
}

fn appended_sizes(handle: &log_gpui::live::SourceHandle, total: usize) -> Vec<usize> {
    let mut sizes = Vec::new();
    let mut seen = 0;
    while seen < total {
        if let SourceEvent::Append { lines, .. } = next(handle, |e| matches!(e, SourceEvent::Append { .. })) {
            seen += lines.len();
            sizes.push(lines.iter().map(|l| l.len() + 1).sum());
        }
    }
    sizes
}

#[test]
fn a_burst_and_a_rewrite_of_a_large_file_are_read_in_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.log");
    std::fs::write(&path, "").unwrap();
    let handle = LocalFileSource::new(&path, 0).with_poll_interval(Duration::from_millis(10)).spawn();
    next(&handle, |e| matches!(e, SourceEvent::Status(_)));

    let text = big_text(CHUNK_BYTES * 3 + 1000);
    let count = text.lines().count();
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(text.as_bytes()).unwrap();
    let sizes = appended_sizes(&handle, count);
    assert!(sizes.len() >= 3, "a burst came in {} batches", sizes.len());
    assert!(sizes.iter().all(|s| *s <= CHUNK_BYTES + 200), "no batch past a chunk: {sizes:?}");

    // Truncated, then rewritten large.
    let rewritten = big_text(CHUNK_BYTES * 2 + 1000);
    std::fs::write(&path, &rewritten).unwrap();
    next(&handle, |e| matches!(e, SourceEvent::Break { .. }));
    let sizes = appended_sizes(&handle, rewritten.lines().count());
    assert!(sizes.len() >= 2 && sizes.iter().all(|s| *s <= CHUNK_BYTES + 200), "{sizes:?}");
}

#[test]
fn load_more_reads_the_lines_above_a_line_number() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.log");
    let text: String = (1..=100).map(|i| format!("row {i}\n")).collect();
    std::fs::write(&path, &text).unwrap();
    let handle = LocalFileSource::new(&path, text.len() as u64).with_poll_interval(Duration::from_millis(10)).spawn();
    next(&handle, |e| matches!(e, SourceEvent::Status(_)));
    let hash = log_gpui::workspace::log_space::line_hash("row 61");
    let more = |line: usize| SourceCommand::LoadMore {
        generation: 0,
        end_offset: 0,
        lines: 10,
        first_hash: Some(hash),
        held_bytes: 0,
        first_line_no: Some(line),
        inode: None,
    };
    handle.send(more(61));
    match next(&handle, |e| matches!(e, SourceEvent::Older { .. } | SourceEvent::OlderUnavailable)) {
        SourceEvent::Older { lines, reached_start, .. } => {
            assert_eq!(lines.first().unwrap(), "row 51");
            assert_eq!(lines.last().unwrap(), "row 60");
            assert!(!reached_start);
        }
        other => panic!("{other:?}"),
    }
    // A line that is not the one held means the file changed.
    handle.send(more(5));
    assert!(matches!(
        next(&handle, |e| matches!(e, SourceEvent::Older { .. } | SourceEvent::OlderUnavailable)),
        SourceEvent::OlderUnavailable
    ));
}

#[test]
fn splitting_without_pending_equals_splitting_with_it() {
    let text = "a\r\nbb\n\nécole\nlast".as_bytes();
    let mut whole = LineSplitter::new();
    let (lines, lens) = whole.push_with_lens(text);
    assert_eq!(lines, vec!["a", "bb", "", "école"]);
    assert_eq!(lens, vec![3, 3, 1, 7]);
    for cut in 0..text.len() {
        let mut s = LineSplitter::new();
        let (mut l, mut n) = s.push_with_lens(&text[..cut]);
        let (l2, n2) = s.push_with_lens(&text[cut..]);
        l.extend(l2);
        n.extend(n2);
        assert_eq!((l, n), (lines.clone(), lens.clone()), "cut {cut}");
    }
}

#[gpui::test]
fn a_local_file_grown_past_the_window_holds_the_window(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 120, 150);
    f.h.view.update(&mut f.h.cx, |app, _| {
        for s in app.live.sources.values_mut() {
            s.remote = false;
        }
    });
    setup(&mut f);
    for n in 0..3 {
        let start = 120 + n * 40;
        f.send("alpha", lines("alpha", start..start + 40, 0));
        f.pump();
        let rows = f.h.view.read_with(&f.h.cx, |app, _| app.all_entries.len());
        assert!(rows <= 150 + 15 + 20, "batch {n}: the window held: {rows} rows");
        check_against_rebuild(&mut f, &format!("batch {n}"));
    }
    let first = f.h.view.read_with(&f.h.cx, |app, _| app.all_entries[0].index);
    assert!(first > 1, "the oldest lines left");
}
