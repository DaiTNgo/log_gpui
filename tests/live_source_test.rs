use log_gpui::live::backward::ReadAt;
use log_gpui::live::{
    BackwardLines, BreakKind, LineSource, LineSplitter, LocalFileSource, SourceCommand,
    SourceEvent, SourceHandle,
};
use std::io::Write;
use std::time::{Duration, Instant};

#[test]
fn splitter_holds_back_unfinished_line() {
    let mut s = LineSplitter::new();
    assert_eq!(s.push(b"a\nERROR d"), vec!["a"]);
    assert_eq!(s.pending_len(), 7);
    assert_eq!(s.push(b"b\n"), vec!["ERROR db"]);
    assert_eq!(s.pending_len(), 0);
}

#[test]
fn splitter_survives_a_cut_inside_a_utf8_character() {
    let text = "héllo wörld\nxin chào\n".as_bytes();
    for cut in 0..text.len() {
        let mut s = LineSplitter::new();
        let mut lines = s.push(&text[..cut]);
        lines.extend(s.push(&text[cut..]));
        assert_eq!(lines, vec!["héllo wörld", "xin chào"], "cut at {cut}");
    }
}

#[test]
fn splitter_strips_carriage_return() {
    let mut s = LineSplitter::new();
    assert_eq!(s.push(b"a\r\nb\r\n"), vec!["a", "b"]);
}

fn all_back(mut data: Vec<u8>, block: usize, batch: usize) -> (Vec<Vec<String>>, u64) {
    let mut b = BackwardLines::new(data.len() as u64, block);
    let mut batches = Vec::new();
    let mut follow = None;
    loop {
        let lines = b.next_batch(&mut data, batch).unwrap();
        follow.get_or_insert(b.follow_offset());
        if lines.is_empty() {
            break;
        }
        batches.push(lines);
    }
    assert!(b.is_done());
    (batches, follow.unwrap())
}

fn numbered(n: usize) -> Vec<u8> {
    (1..=n).map(|i| format!("line {i}\n")).collect::<String>().into_bytes()
}

#[test]
fn backward_returns_last_lines_first_then_older_batches() {
    let (batches, follow) = all_back(numbered(10), 7, 4);
    assert_eq!(batches[0], vec!["line 7", "line 8", "line 9", "line 10"]);
    assert_eq!(batches[1], vec!["line 3", "line 4", "line 5", "line 6"]);
    assert_eq!(batches[2], vec!["line 1", "line 2"]);
    assert_eq!(follow, numbered(10).len() as u64);
}

#[test]
fn backward_file_smaller_than_requested() {
    let (batches, _) = all_back(numbered(3), 64, 1000);
    assert_eq!(batches, vec![vec!["line 1", "line 2", "line 3"]]);
}

#[test]
fn backward_leaves_unfinished_last_line_to_the_forward_reader() {
    let mut data = numbered(3);
    let complete = data.len() as u64;
    data.extend_from_slice(b"partial");
    let (batches, follow) = all_back(data, 5, 10);
    assert_eq!(batches, vec![vec!["line 1", "line 2", "line 3"]]);
    assert_eq!(follow, complete);
}

#[test]
fn backward_line_longer_than_a_block() {
    let long = "x".repeat(100);
    let data = format!("a\n{long}\nb\n").into_bytes();
    let (batches, _) = all_back(data, 8, 2);
    let flat: Vec<String> = batches.into_iter().rev().flatten().collect();
    assert_eq!(flat, vec!["a".to_string(), long, "b".to_string()]);
}

#[test]
fn backward_every_block_size_gives_the_same_lines() {
    let data = numbered(57);
    for block in 1..40 {
        for batch in [1, 3, 10, 100] {
            let (batches, _) = all_back(data.clone(), block, batch);
            let flat: Vec<String> = batches.into_iter().rev().flatten().collect();
            assert_eq!(flat.len(), 57, "block {block} batch {batch}");
            assert_eq!(flat[0], "line 1");
            assert_eq!(flat[56], "line 57");
        }
    }
}

#[test]
fn backward_empty_and_newline_free_files() {
    let (batches, follow) = all_back(Vec::new(), 8, 4);
    assert!(batches.is_empty());
    assert_eq!(follow, 0);
    let (batches, follow) = all_back(b"no newline".to_vec(), 4, 4);
    assert!(batches.is_empty());
    assert_eq!(follow, 0);
    let mut v = b"abc".to_vec();
    let mut buf = [0u8; 2];
    assert_eq!(v.read_at(1, &mut buf).unwrap(), 2);
    assert_eq!(&buf, b"bc");
}

// --- LocalFileSource ---

fn wait_for(handle: &SourceHandle, mut want: impl FnMut(&SourceEvent) -> bool) -> SourceEvent {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match handle.events.try_recv() {
            Ok(event) if want(&event) => return event,
            Ok(_) => {}
            Err(_) => {
                assert!(Instant::now() < deadline, "timed out waiting for an event");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

fn source(path: &std::path::Path, offset: u64) -> SourceHandle {
    let handle = LocalFileSource::new(path, offset)
        .with_poll_interval(Duration::from_millis(10))
        .spawn();
    // The source opens the file as it is when it starts.
    wait_for(&handle, |e| matches!(e, SourceEvent::Status(_)));
    handle
}

fn appended(lines: &[&str]) -> SourceEvent {
    SourceEvent::Append { lines: lines.iter().map(|s| s.to_string()).collect(), streams: None }
}

#[test]
fn local_source_sends_lines_written_after_the_start_offset() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.log");
    std::fs::write(&path, "old\n").unwrap();
    let h = source(&path, 4);
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(b"one\ntwo\nERROR db").unwrap();
    assert_eq!(wait_for(&h, |e| matches!(e, SourceEvent::Append { .. })), appended(&["one", "two"]));
    f.write_all(b"\n").unwrap();
    assert_eq!(wait_for(&h, |e| matches!(e, SourceEvent::Append { .. })), appended(&["ERROR db"]));
    h.close();
}

#[test]
fn local_source_pause_loses_no_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.log");
    std::fs::write(&path, "").unwrap();
    let h = source(&path, 0);
    h.send(SourceCommand::Pause);
    std::thread::sleep(Duration::from_millis(60));
    std::fs::write(&path, "a\nb\nc\n").unwrap();
    std::thread::sleep(Duration::from_millis(80));
    while let Ok(e) = h.events.try_recv() {
        assert!(!matches!(e, SourceEvent::Append { .. }), "got {e:?} while paused");
    }
    h.send(SourceCommand::Resume);
    assert_eq!(wait_for(&h, |e| matches!(e, SourceEvent::Append { .. })), appended(&["a", "b", "c"]));
    h.close();
}

#[test]
fn local_source_truncate_makes_a_break_and_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.log");
    std::fs::write(&path, "a\nb\nc\n").unwrap();
    let h = source(&path, 6);
    std::thread::sleep(Duration::from_millis(60));
    std::fs::OpenOptions::new().write(true).truncate(true).open(&path).unwrap();
    wait_for(&h, |e| *e == SourceEvent::Break { kind: BreakKind::Truncated });
    std::fs::write(&path, "x\n").unwrap();
    assert_eq!(wait_for(&h, |e| matches!(e, SourceEvent::Append { .. })), appended(&["x"]));
    h.close();
}

#[test]
fn local_source_rotation_reads_the_old_tail_then_the_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.log");
    std::fs::write(&path, "a\n").unwrap();
    let h = source(&path, 2);
    std::thread::sleep(Duration::from_millis(40));
    // Written to the old file just before it is renamed away.
    let mut old = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    old.write_all(b"last-old\n").unwrap();
    std::fs::rename(&path, dir.path().join("app.log.1")).unwrap();
    std::fs::write(&path, "first-new\n").unwrap();
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while seen.len() < 3 && Instant::now() < deadline {
        match h.events.try_recv() {
            Ok(SourceEvent::Append { lines, .. }) => seen.push(format!("lines:{}", lines.join(","))),
            Ok(SourceEvent::Break { kind: BreakKind::Rotated }) => seen.push("break".into()),
            _ => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    assert_eq!(seen, vec!["lines:last-old", "break", "lines:first-new"]);
    h.close();
}

#[test]
fn local_source_deleted_waits_for_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.log");
    std::fs::write(&path, "a\n").unwrap();
    let h = source(&path, 2);
    std::fs::remove_file(&path).unwrap();
    wait_for(&h, |e| *e == SourceEvent::Break { kind: BreakKind::Deleted });
    wait_for(&h, |e| matches!(e, SourceEvent::Status(s) if s.label() == "Waiting for file"));
    std::fs::write(&path, "new\n").unwrap();
    assert_eq!(wait_for(&h, |e| matches!(e, SourceEvent::Append { .. })), appended(&["new"]));
    h.close();
}

// --- Generations ---

use log_gpui::focus::LineRef;
use log_gpui::marker::TextMarker;
use log_gpui::palette::HighlightColor;
use log_gpui::workspace::log_space::LogFile;

fn red() -> HighlightColor {
    HighlightColor::custom("#ff0000").expect("a valid color")
}

#[test]
fn line_ref_tells_generations_apart_and_reads_old_sessions() {
    assert_ne!(LineRef::new("f", 1), LineRef::new("f", 1).with_generation(1));
    let old: LineRef = serde_json::from_str(r#"{"file_id":"f","line":3}"#).unwrap();
    assert_eq!(old, LineRef::new("f", 3));
}

#[test]
fn markers_follow_generations_and_only_distance_zero_is_restored() {
    let mut file = LogFile::new("f".into(), "f".into());
    assert!(file.paint_marker(1, "first file line", 0..5, red()));
    file.begin_generation();
    // Line 1 again, now in generation 1.
    assert!(file.paint_marker_in(file.generations_ago(1), 1, "new file line", 0..3, red()));
    assert_eq!(file.markers_on_line_in(1, 1).len(), 1, "the old one is one generation back");
    assert_eq!(file.markers_on_line_in(1, 0).len(), 1);
    assert_ne!(file.markers_on_line_in(1, 0)[0].range, file.markers_on_line_in(1, 1)[0].range);

    let json = serde_json::to_string(&file).unwrap();
    let back: LogFile = serde_json::from_str(&json).unwrap();
    assert_eq!(back.markers.len(), 1, "older generations are not restored");
    assert_eq!(back.markers[0].generations_ago, 0);
    let _: Option<TextMarker> = None;
}

#[test]
fn markers_of_lines_that_left_the_window_are_dropped() {
    let mut file = LogFile::new("f".into(), "f".into());
    for line in 1..=5 {
        file.paint_marker(line, "x", 0..1, red());
    }
    file.drop_markers_before(0, 4);
    let lines: Vec<usize> = file.markers.iter().map(|m| m.line).collect();
    assert_eq!(lines, vec![4, 5]);
}
