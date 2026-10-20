//! Clear, Load More and Extended Window on the cache of a Remote File, with a
//! byte vector standing for the file on the Server.

use log_gpui::app::live_more::next_surviving_row;
use log_gpui::live::{BackwardLines, BreakKind, HoldMode, LineSplitter, LiveBuffer};
use log_gpui::space_view::ParsedFile;

/// A file of `count` lines; every third line is longer, has multi-byte
/// characters, and every fifth ends with CRLF.
fn file_bytes(count: usize) -> (Vec<u8>, Vec<u64>) {
    let mut bytes = Vec::new();
    let mut starts = Vec::new();
    for i in 1..=count {
        starts.push(bytes.len() as u64);
        let extra = if i % 3 == 0 { " héllo wörld ✓" } else { "" };
        let end = if i % 5 == 0 { "\r\n" } else { "\n" };
        bytes.extend_from_slice(format!("2026-09-05 10:00:{:02} INFO line {i}{extra}{end}", i % 60).as_bytes());
    }
    (bytes, starts)
}

/// What a remote source sends first: the last `n` lines of the file.
fn tail(buf: &mut LiveBuffer, parsed: &mut ParsedFile, data: &mut Vec<u8>, n: usize) -> u64 {
    let mut backward = BackwardLines::new(data.len() as u64, 64);
    let (lines, lens) = backward.next_batch_with_lens(data, n).unwrap();
    buf.apply_tail_with(parsed, &lines, 0, &lens, backward.follow_offset());
    backward.follow_offset()
}

#[test]
fn byte_lens_count_crlf_and_multibyte_lines() {
    let (mut data, starts) = file_bytes(20);
    let mut backward = BackwardLines::new(data.len() as u64, 16);
    let (lines, lens) = backward.next_batch_with_lens(&mut data, 20).unwrap();
    assert_eq!(lines.len(), 20);
    for i in 0..20 {
        let end = starts.get(i + 1).copied().unwrap_or(data.len() as u64);
        assert_eq!(u64::from(lens[i]), end - starts[i], "line {}", i + 1);
    }
    let mut splitter = LineSplitter::new();
    let (lines, lens) = splitter.push_with_lens(&data[..20]);
    assert!(lines.is_empty() && lens.is_empty());
    let (lines, lens) = splitter.push_with_lens(&data[20..]);
    assert_eq!(lines.len(), 20);
    assert_eq!(lens.iter().map(|l| u64::from(*l)).sum::<u64>(), data.len() as u64);
    assert_eq!(lens[0] as u64, starts[1] - starts[0], "the first line was cut across two pushes");
}

#[test]
fn held_start_follows_tail_backfill_append_and_trim() {
    let (mut data, starts) = file_bytes(300);
    let mut parsed = ParsedFile::from_text("");
    let mut buf = LiveBuffer::remote("web1:/a.log");
    let end = tail(&mut buf, &mut parsed, &mut data, 50);
    buf.apply_total(&mut parsed, Some(300));
    assert_eq!(end, data.len() as u64);
    let held = buf.held_start(&parsed);
    assert_eq!(held.offset, Some(starts[250]));

    // Older lines above.
    let mut backward = BackwardLines::new(starts[250], 64);
    let (lines, lens) = backward.next_batch_with_lens(&mut data, 30).unwrap();
    buf.apply_backfill_with(&mut parsed, &lines, &lens);
    assert_eq!(buf.held_start(&parsed).offset, Some(starts[220]));

    // New lines at the end do not move the start; trims do.
    let (more, more_starts) = file_bytes(310);
    let tail_bytes = &more[data.len()..];
    let mut splitter = LineSplitter::new();
    let (new_lines, new_lens) = splitter.push_with_lens(tail_bytes);
    assert_eq!(new_lines.len(), 10);
    buf.apply_append_with(&mut parsed, &new_lines, &new_lens);
    assert_eq!(buf.held_start(&parsed).offset, Some(starts[220]));
    let trimmed = buf.trim_to_window(&mut parsed, 60).expect("90 held, window 60");
    assert_eq!(trimmed.removed, 30);
    assert_eq!(buf.held_start(&parsed).offset, Some(more_starts[250]));
    assert_eq!(buf.held_start(&parsed).line_no, 251);
}

#[test]
fn an_extended_file_is_not_cut_until_the_full_read_limit() {
    let (mut data, _) = file_bytes(300);
    let mut parsed = ParsedFile::from_text("");
    let mut buf = LiveBuffer::remote("web1:/a.log");
    tail(&mut buf, &mut parsed, &mut data, 100);
    assert_eq!(buf.hold, HoldMode::Window);
    assert_eq!(buf.limit(50, 1_000), 50);
    // Load More reads 150 lines above.
    let start = buf.held_start(&parsed).offset.unwrap();
    let mut backward = BackwardLines::new(start, 64);
    let (lines, lens) = backward.next_batch_with_lens(&mut data, 150).unwrap();
    assert!(buf.apply_older(&mut parsed, &lines, &lens, false));
    assert_eq!(buf.hold, HoldMode::Extended);
    assert_eq!(buf.limit(50, 1_000), 1_000);
    assert!(buf.trim_to_window(&mut parsed, buf.limit(50, 1_000)).is_none());
    assert_eq!(parsed.entries.len(), 250);
    // Over the Full Read Limit the oldest lines leave like from a window.
    let trimmed = buf.trim_to_window(&mut parsed, 200).unwrap();
    assert_eq!(trimmed.removed, 50);
}

#[test]
fn older_lines_are_numbered_back_from_the_oldest_held_line_and_join_the_lines() {
    let (mut data, starts) = file_bytes(300);
    let mut parsed = ParsedFile::from_text("");
    let mut buf = LiveBuffer::remote("web1:/a.log");
    tail(&mut buf, &mut parsed, &mut data, 50);
    buf.apply_total(&mut parsed, Some(300));
    assert_eq!(parsed.entries[0].index, 251);
    buf.trim_to_window(&mut parsed, 10); // no cut: within the margin of 50 held? 50 > 11, cut.
    let first = buf.held_start(&parsed);
    assert_eq!(first.line_no, 291);
    assert!(parsed.entries[0].break_before.as_ref().is_some_and(|m| m.kind == BreakKind::LeftWindow));

    let mut backward = BackwardLines::new(first.offset.unwrap(), 64);
    let (lines, lens) = backward.next_batch_with_lens(&mut data, 40).unwrap();
    assert!(buf.apply_older(&mut parsed, &lines, &lens, false));
    assert_eq!(parsed.entries[0].index, 251);
    assert_eq!(parsed.entries[39].index, 290);
    assert_eq!(parsed.entries[40].index, 291);
    assert_eq!(buf.held_start(&parsed).offset, Some(starts[250]));
    // The old "left the window" break is gone: the lines are continuous.
    assert!(parsed.entries[40].break_before.is_none());
    assert!(!parsed.raw_breaks.contains_key(&40));
    assert_eq!(
        parsed.entries[0].break_before.as_ref().unwrap().kind,
        BreakKind::StartOfLoaded
    );

    // Up to the start of the file: no Break above the first line.
    let mut backward = BackwardLines::new(starts[250], 64);
    let (lines, lens) = backward.next_batch_with_lens(&mut data, 1_000).unwrap();
    assert_eq!(lines.len(), 250);
    assert!(backward.is_done());
    buf.apply_older(&mut parsed, &lines, &lens, true);
    assert_eq!(parsed.entries[0].index, 1);
    assert!(parsed.entries[0].break_before.is_none());
    assert!(buf.at_generation_start(&parsed));
}

#[test]
fn clear_drops_every_line_and_the_next_line_gets_a_cleared_break() {
    let (mut data, starts) = file_bytes(200);
    let mut parsed = ParsedFile::from_text("");
    let mut buf = LiveBuffer::remote("web1:/a.log");
    tail(&mut buf, &mut parsed, &mut data, 100);
    buf.apply_total(&mut parsed, Some(200));
    let trimmed = buf.clear(&mut parsed, "10:42:11");
    assert_eq!(trimmed.removed, 100);
    assert!(parsed.entries.is_empty() && parsed.lines.is_empty() && parsed.raw_breaks.is_empty());
    assert_eq!(parsed.entries.capacity(), 0, "the memory is given back");
    assert_eq!(buf.hold, HoldMode::Window);

    // Nothing held: Load More starts at the Clear Point, with the next number.
    let held = buf.held_start(&parsed);
    assert_eq!((held.line_no, held.offset), (201, Some(data.len() as u64)));

    // New lines come under the Break.
    let (more, _) = file_bytes(203);
    let mut splitter = LineSplitter::new();
    let (lines, lens) = splitter.push_with_lens(&more[data.len()..]);
    buf.apply_append_with(&mut parsed, &lines, &lens);
    assert_eq!(parsed.entries.len(), 3);
    assert_eq!(parsed.entries[0].index, 201);
    let mark = parsed.entries[0].break_before.as_ref().unwrap();
    assert_eq!((mark.kind, mark.label.as_str()), (BreakKind::Cleared, "web1:/a.log cleared 10:42:11"));
    assert!(parsed.raw_breaks.contains_key(&0));

    // Load More after Clear reads the cleared lines again, newest first, and
    // the Break stays where Clear cut.
    let held = buf.held_start(&parsed);
    assert_eq!((held.line_no, held.offset), (201, Some(data.len() as u64)));
    let mut backward = BackwardLines::new(held.offset.unwrap(), 64);
    let (lines, lens) = backward.next_batch_with_lens(&mut data, 10).unwrap();
    assert!(buf.apply_older(&mut parsed, &lines, &lens, false));
    assert_eq!(parsed.entries[0].index, 191);
    assert_eq!(parsed.entries[9].index, 200);
    assert_eq!(parsed.entries[10].index, 201);
    assert_eq!(parsed.entries[10].break_before.as_ref().unwrap().kind, BreakKind::Cleared);
    assert_eq!(buf.held_start(&parsed).offset, Some(starts[190]));
}

#[test]
fn back_to_window_cuts_at_once_and_returns_to_window_mode() {
    let (mut data, _) = file_bytes(300);
    let mut parsed = ParsedFile::from_text("");
    let mut buf = LiveBuffer::remote("web1:/a.log");
    let _ = (&mut buf, &mut parsed);
    let mut buf = LiveBuffer::remote("web1:/a.log");
    let mut parsed = ParsedFile::from_text("");
    tail(&mut buf, &mut parsed, &mut data, 120);
    let mut backward = BackwardLines::new(buf.held_start(&parsed).offset.unwrap(), 64);
    let (lines, lens) = backward.next_batch_with_lens(&mut data, 100).unwrap();
    buf.apply_older(&mut parsed, &lines, &lens, false);
    assert_eq!(buf.hold, HoldMode::Extended);
    let trimmed = buf.back_to_window(&mut parsed, 50).expect("220 held, window 50");
    assert_eq!(trimmed.removed, 170);
    assert_eq!(parsed.entries.len(), 50);
    assert_eq!(buf.hold, HoldMode::Window);
}

#[test]
fn the_inode_of_each_generation_is_kept() {
    let mut buf = LiveBuffer::remote("web1:/a.log");
    buf.set_inode(Some(7));
    buf.apply_break(BreakKind::Rotated, "10:00:00");
    buf.set_inode(Some(9));
    assert_eq!(buf.inode_of(0), Some(7));
    assert_eq!(buf.inode_of(1), Some(9));
    assert_eq!(buf.inode_of(2), None);
}

/// Two files with interleaved times: `a` has the odd seconds, `b` the even.
fn interleaved(count: usize) -> (ParsedFile, ParsedFile, LiveBuffer, LiveBuffer) {
    let make = |name: &str, parity: usize| {
        let mut parsed = ParsedFile::from_text("");
        let mut buf = LiveBuffer::loaded(name, 0);
        let lines: Vec<String> = (0..count)
            .map(|i| format!("2026-09-05 10:00:{:02} INFO {name} line {}", i * 2 + parity, i + 1))
            .collect();
        buf.apply_append(&mut parsed, &lines);
        (parsed, buf)
    };
    let (pa, ba) = make("a.log", 1);
    let (pb, bb) = make("b.log", 0);
    (pa, pb, ba, bb)
}

fn merged_files(a: &ParsedFile, b: &ParsedFile) -> Vec<String> {
    let timeline = log_gpui::timeline::merge(&[a.entries.as_slice(), b.entries.as_slice()]);
    timeline.rows.iter().map(|r| if r.file == 0 { "a".to_string() } else { "b".to_string() }).collect()
}

#[test]
fn clearing_one_of_two_interleaved_files_keeps_the_other_and_moves_to_the_next_row() {
    let (mut a, b, mut buf_a, _buf_b) = interleaved(5);
    let before = merged_files(&a, &b);
    assert_eq!(before, ["b", "a", "b", "a", "b", "a", "b", "a", "b", "a"]);

    // The user was on row 3 (the second `a` line).
    let gone: std::collections::HashSet<String> = ["a".to_string()].into();
    let file_of = |ix: usize| Some(before[ix].as_str());
    assert_eq!(next_surviving_row(before.len(), file_of, 3, &gone), Some(4));
    // At the end, with nothing after: the last row that stays.
    assert_eq!(next_surviving_row(before.len(), file_of, 9, &gone), Some(8));
    // Nothing stays: no row.
    let all: std::collections::HashSet<String> = ["a".to_string(), "b".to_string()].into();
    assert_eq!(next_surviving_row(before.len(), file_of, 3, &all), None);

    buf_a.clear(&mut a, "10:42:11");
    let after = merged_files(&a, &b);
    assert_eq!(after, ["b", "b", "b", "b", "b"]);
    assert_eq!(b.entries.len(), 5);
    assert_eq!(b.entries[0].index, 1, "the other file is untouched");
}

#[test]
fn older_lines_go_above_an_earlier_generation_and_say_when_they_are_before_the_starting_time() {
    use log_gpui::live::source::EarlierStart;
    let mut parsed = ParsedFile::from_text("");
    let mut buf = LiveBuffer::remote("web1:/a.log");
    let current: Vec<String> = (1..=10).map(|i| format!("2026-09-05 11:00:{i:02} INFO now {i}")).collect();
    buf.apply_tail_with(&mut parsed, &current, 0, &[], 0);
    buf.apply_total(&mut parsed, Some(10));
    // A Rotated File's last ten lines (numbers 91–100) come above.
    let earlier: Vec<String> = (91..=100).map(|i| format!("2026-09-05 10:00:{:02} INFO old {i}", i % 60)).collect();
    let start = EarlierStart { name: "a.log.1".into(), modified: None };
    assert!(buf.apply_earlier(&mut parsed, Some(&start), &earlier, &[], Some(100)));
    assert_eq!((parsed.entries[0].generation, parsed.entries[0].index), (0, 91));
    assert_eq!(buf.held_start(&parsed).generation, 0);
    assert_eq!(buf.held_start(&parsed).offset, None, "a Rotated File's offset comes from the bytes held");

    let more: Vec<String> = (86..=90).map(|i| format!("2026-09-05 10:00:{:02} INFO old {i}", i % 60)).collect();
    assert!(buf.apply_older_with(&mut parsed, &more, &[], false, true));
    assert_eq!(parsed.entries.len(), 25);
    assert_eq!((parsed.entries[0].generation, parsed.entries[0].index), (0, 86));
    assert_eq!((parsed.entries[5].generation, parsed.entries[5].index), (0, 91));
    assert_eq!(parsed.entries[0].break_before.as_ref().unwrap().kind, BreakKind::StartOfLoaded);
    let joined = parsed.entries[5].break_before.as_ref().unwrap();
    assert_eq!(
        (joined.kind, joined.label.as_str()),
        (BreakKind::BeforeStartTime, "web1:/a.log 5 lines before the Starting Time loaded")
    );
    // The Rotated Generation's Break between the two is where it was.
    assert_eq!(parsed.entries[15].break_before.as_ref().unwrap().kind, BreakKind::Rotated);
    assert_eq!(buf.hold, HoldMode::Extended);
}
