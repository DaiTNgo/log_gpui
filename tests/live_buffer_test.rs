use log_gpui::live::{BreakKind, LiveBuffer, PROVISIONAL_BASE, is_real_line_number};
use log_gpui::space_view::ParsedFile;

fn lines(range: std::ops::RangeInclusive<usize>) -> Vec<String> {
    range.map(|i| format!("2026-09-05 10:00:{:02} INFO line {i}", i % 60)).collect()
}

#[test]
fn local_file_keeps_real_numbers_as_lines_are_appended() {
    let mut parsed = ParsedFile::from_text("2026-09-05 10:00:00 INFO a\n2026-09-05 10:00:01 INFO b\n");
    let mut buf = LiveBuffer::loaded("app.log", parsed.entries.len());
    let range = buf.apply_append(&mut parsed, &lines(3..=4));
    assert_eq!(range, 2..4);
    assert_eq!(parsed.entries[2].index, 3);
    assert_eq!(parsed.entries[3].index, 4);
    assert_eq!(parsed.lines.len(), 4);
}

#[test]
fn remote_tail_gets_real_numbers_when_the_total_arrives() {
    let mut parsed = ParsedFile::empty();
    let mut buf = LiveBuffer::remote("web1:/app/app.log");
    buf.apply_tail(&mut parsed, &lines(1..=3), 0);
    assert!(!is_real_line_number(parsed.entries[0].index));
    assert_eq!(parsed.entries[0].index, PROVISIONAL_BASE);
    // Three lines written after the tail was read.
    buf.apply_append(&mut parsed, &lines(4..=5));
    // The server had 1_999_100 lines up to the end of the tail.
    let delta = buf.apply_total(&mut parsed, Some(1_999_100));
    assert!(delta.is_some());
    let numbers: Vec<usize> = parsed.entries.iter().map(|e| e.index).collect();
    assert_eq!(numbers, vec![1_999_098, 1_999_099, 1_999_100, 1_999_101, 1_999_102]);
    assert!(buf.numbers_known);
    // The next line continues the numbering.
    buf.apply_append(&mut parsed, &lines(6..=6));
    assert_eq!(parsed.entries.last().unwrap().index, 1_999_103);
}

#[test]
fn backfill_goes_above_and_numbers_go_down() {
    let mut parsed = ParsedFile::empty();
    let mut buf = LiveBuffer::remote("web1:/a.log");
    buf.apply_tail(&mut parsed, &lines(8..=10), 0);
    buf.apply_total(&mut parsed, Some(10));
    buf.apply_backfill(&mut parsed, &lines(4..=7));
    buf.apply_backfill(&mut parsed, &lines(1..=3));
    let numbers: Vec<usize> = parsed.entries.iter().map(|e| e.index).collect();
    assert_eq!(numbers, (1..=10).collect::<Vec<_>>());
    assert_eq!(parsed.lines.len(), 10);
    assert!(parsed.lines[0].ends_with("line 1"));
    assert!(parsed.lines[9].ends_with("line 10"));
}

#[test]
fn backfill_before_the_total_is_known_still_lines_up() {
    let mut parsed = ParsedFile::empty();
    let mut buf = LiveBuffer::remote("web1:/a.log");
    buf.apply_tail(&mut parsed, &lines(8..=10), 0);
    buf.apply_backfill(&mut parsed, &lines(1..=7));
    buf.apply_total(&mut parsed, Some(10));
    let numbers: Vec<usize> = parsed.entries.iter().map(|e| e.index).collect();
    assert_eq!(numbers, (1..=10).collect::<Vec<_>>());
}

#[test]
fn backfill_relinks_continuation_lines_at_the_top() {
    let mut parsed = ParsedFile::empty();
    let mut buf = LiveBuffer::remote("web1:/a.log");
    buf.apply_tail(
        &mut parsed,
        &["    at Foo.bar(Foo.java:1)".to_string(), "2026-09-05 10:00:02 INFO next".to_string()],
        0,
    );
    assert_eq!(parsed.entries[0].parent, None);
    buf.apply_backfill(&mut parsed, &["2026-09-05 10:00:01 ERROR boom".to_string()]);
    assert_eq!(parsed.entries[1].parent, Some(parsed.entries[0].index));
}

#[test]
fn rotation_starts_a_generation_and_draws_a_break_above_the_next_line() {
    let mut parsed = ParsedFile::from_text(&(lines(1..=3).join("\n") + "\n"));
    let mut buf = LiveBuffer::loaded("app.log", 3);
    buf.apply_break(BreakKind::Rotated, "10:02:11");
    assert_eq!(buf.generation, 1);
    buf.apply_append(&mut parsed, &lines(1..=2));
    assert_eq!(parsed.entries[3].index, 1);
    assert_eq!(parsed.entries[3].generation, 1);
    assert_eq!(parsed.entries[0].generation, 0);
    let mark = parsed.entries[3].break_before.as_ref().expect("break above the first new line");
    assert_eq!(mark.label, "app.log rotated 10:02:11");
    assert!(parsed.entries[4].break_before.is_none());
    assert!(parsed.raw_breaks.contains_key(&3));
}

#[test]
fn disconnection_makes_one_break_with_both_times_and_no_new_generation() {
    let mut parsed = ParsedFile::empty();
    let mut buf = LiveBuffer::remote("web1:/app/app.log");
    buf.apply_tail(&mut parsed, &lines(1..=2), 0);
    buf.apply_break(BreakKind::Disconnected, "10:02");
    assert!(parsed.entries.iter().all(|e| e.break_before.is_none()));
    buf.apply_break(BreakKind::Reconnected, "10:03");
    buf.apply_append(&mut parsed, &lines(3..=3));
    assert_eq!(buf.generation, 0);
    assert_eq!(
        parsed.entries[2].break_before.as_ref().unwrap().label,
        "web1:/app/app.log disconnected 10:02 – reconnected 10:03"
    );
}

#[test]
fn live_window_drops_the_oldest_lines_in_batches_with_a_break() {
    let mut parsed = ParsedFile::from_text("");
    let mut buf = LiveBuffer::loaded("app.log", 0);
    buf.apply_append(&mut parsed, &lines(1..=105));
    // Within 10% over the window nothing is dropped.
    assert!(buf.trim_to_window(&mut parsed, 100).is_none());
    buf.apply_append(&mut parsed, &lines(106..=115));
    let trimmed = buf.trim_to_window(&mut parsed, 100).expect("over the margin");
    assert_eq!(trimmed.removed, 15);
    assert_eq!(trimmed.first_kept, 16);
    assert_eq!(parsed.entries.len(), 100);
    assert_eq!(parsed.lines.len(), 100);
    assert_eq!(parsed.entries[0].index, 16);
    assert_eq!(
        parsed.entries[0].break_before.as_ref().unwrap().label,
        "app.log older lines left the window"
    );
    assert!(parsed.raw_breaks.contains_key(&0));
}

#[test]
fn a_line_loaded_without_its_newline_is_replaced_by_the_whole_line() {
    let text = "2026-09-05 10:00:00 INFO a\n2026-09-05 10:00:01 ERROR db";
    let mut parsed = ParsedFile::from_text(text);
    let mut buf = LiveBuffer::loaded_from_bytes("app.log", text.as_bytes(), parsed.entries.len());
    assert_eq!(buf.local_offset, 27, "reading starts at the unfinished line");
    // The source later delivers the finished line again.
    let range = buf.apply_append(
        &mut parsed,
        &["2026-09-05 10:00:01 ERROR db down".to_string(), "2026-09-05 10:00:02 INFO c".to_string()],
    );
    assert_eq!(range, 1..3);
    assert_eq!(parsed.entries.len(), 3);
    assert_eq!(parsed.entries[1].index, 2);
    assert!(parsed.lines[1].ends_with("db down"));
    assert_eq!(parsed.entries[2].index, 3);
}

#[test]
fn filter_counts_follow_the_lines_the_window_keeps() {
    use log_gpui::filter_index::FilterIndex;
    let mut parsed = ParsedFile::from_text("");
    let mut buf = LiveBuffer::loaded("app.log", 0);
    let mut text = Vec::new();
    for i in 1..=60 {
        text.push(format!("2026-09-05 10:00:{:02} INFO [cron] job {i}", i % 60));
    }
    for i in 61..=120 {
        text.push(format!("2026-09-05 10:01:{:02} INFO [db] query {i}", i % 60));
    }
    buf.apply_append(&mut parsed, &text);
    let count = |p: &ParsedFile, v: &str| {
        let idx = FilterIndex::build(&p.entries);
        idx.column("module")
            .and_then(|c| c.values.iter().find(|x| x.folded == v).map(|x| x.count))
    };
    assert_eq!(count(&parsed, "cron"), Some(60));
    buf.trim_to_window(&mut parsed, 60).expect("trimmed");
    assert_eq!(count(&parsed, "cron"), None, "no cron line is left");
    assert_eq!(count(&parsed, "db"), Some(60));
}

mod earlier {
    use super::*;
    use log_gpui::live::source::{EarlierFile, EarlierStart};

    fn start(name: &str) -> EarlierStart {
        EarlierStart { name: name.into(), modified: Some("2026-10-18 03:00:01".into()) }
    }

    fn current_file() -> (ParsedFile, LiveBuffer) {
        let mut parsed = ParsedFile::empty();
        let mut buf = LiveBuffer::remote("web1:/app/app.log");
        buf.apply_tail(&mut parsed, &lines(1..=3), 0);
        buf.apply_total(&mut parsed, Some(3));
        (parsed, buf)
    }

    #[test]
    fn a_rotated_file_becomes_an_earlier_generation_with_its_own_numbers() {
        let (mut parsed, mut buf) = current_file();
        let moved = buf.apply_earlier(&mut parsed, Some(&start("app.log.1")), &lines(8..=10), &[], Some(10));
        assert!(moved);
        let seen: Vec<(u32, usize)> = parsed.entries.iter().map(|e| (e.generation, e.index)).collect();
        assert_eq!(seen, [(0, 8), (0, 9), (0, 10), (1, 1), (1, 2), (1, 3)]);
        assert_eq!(buf.generation, 1);
        let mark = parsed.entries[3].break_before.as_ref().unwrap();
        assert_eq!(mark.kind, BreakKind::Rotated);
        assert_eq!(mark.label, "web1:/app/app.log rotated 2026-10-18 03:00:01");
        assert_eq!(parsed.lines.len(), 6);
        assert!(parsed.raw_breaks.contains_key(&3));
    }

    #[test]
    fn more_lines_go_above_the_same_earlier_generation() {
        let (mut parsed, mut buf) = current_file();
        buf.apply_earlier(&mut parsed, Some(&start("app.log.1")), &lines(8..=10), &[], Some(10));
        let moved = buf.apply_earlier(&mut parsed, None, &lines(5..=7), &[], None);
        assert!(!moved);
        let seen: Vec<(u32, usize)> = parsed.entries.iter().map(|e| (e.generation, e.index)).collect();
        assert_eq!(seen[..6], [(0, 5), (0, 6), (0, 7), (0, 8), (0, 9), (0, 10)]);
        // The Break still sits above the current file's first line.
        assert!(parsed.entries[6].break_before.is_some());
    }

    #[test]
    fn a_second_rotated_file_moves_every_generation_up_again() {
        let (mut parsed, mut buf) = current_file();
        buf.apply_earlier(&mut parsed, Some(&start("app.log.1")), &lines(8..=10), &[], Some(10));
        buf.apply_earlier(&mut parsed, Some(&start("app.log.2.gz")), &lines(1..=2), &[], Some(2));
        let seen: Vec<(u32, usize)> = parsed.entries.iter().map(|e| (e.generation, e.index)).collect();
        assert_eq!(seen, [(0, 1), (0, 2), (1, 8), (1, 9), (1, 10), (2, 1), (2, 2), (2, 3)]);
        assert_eq!(buf.generation, 2);
        assert!(parsed.entries[2].break_before.is_some() && parsed.entries[5].break_before.is_some());
    }

    #[test]
    fn unknown_numbers_are_not_real() {
        let (mut parsed, mut buf) = current_file();
        buf.apply_earlier(&mut parsed, Some(&start("app.log.1")), &lines(1..=2), &[], None);
        assert!(!is_real_line_number(parsed.entries[0].index));
        assert!(is_real_line_number(parsed.entries[2].index));
    }

    #[test]
    fn the_button_break_goes_on_top_and_leaves_with_the_next_load() {
        let (mut parsed, mut buf) = current_file();
        let next = EarlierFile { name: "app.log.2.gz".into(), size: 38 * 1024 * 1024, compressed: true };
        buf.set_earlier_available(&mut parsed, Some(&next));
        let mark = parsed.entries[0].break_before.as_ref().unwrap();
        assert_eq!(mark.kind, BreakKind::EarlierAvailable);
        assert_eq!(mark.label, "Load earlier generation (app.log.2.gz, 38 MB)");
        buf.apply_earlier(&mut parsed, Some(&start("app.log.2.gz")), &lines(1..=2), &[], Some(2));
        // The Break between the Generations replaced it.
        assert_eq!(parsed.entries[2].break_before.as_ref().unwrap().kind, BreakKind::Rotated);
        assert!(parsed.entries[0].break_before.is_none());
        buf.set_earlier_available(&mut parsed, None);
        assert!(parsed.entries[0].break_before.is_none());
    }
}
