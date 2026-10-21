//! The texts a Search scan reads on a worker thread are kept between scans:
//! only rows that arrived since the last scan are copied.
use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::search_lines::SearchLines;

fn entries(range: std::ops::Range<usize>) -> Vec<LogEntry> {
    range
        .map(|i| LogEntry::new(i, 0.0, String::new(), String::new(), LogLevel::Info, String::new(), format!("line {i}")))
        .collect()
}

fn texts(lines: &SearchLines) -> Vec<String> {
    (0..lines.len()).map(|i| lines.get(i).to_string()).collect()
}

#[test]
fn it_follows_appended_rows_without_copying_the_old_ones() {
    let mut rows = entries(0..150_000);
    let mut lines = SearchLines::default();
    assert_eq!(lines.sync(&rows, 0), 150_000, "the first scan copies every row");
    let held = lines.clone();
    rows.extend(entries(150_000..150_010));
    assert_eq!(lines.sync(&rows, 0), 10, "only the new rows are copied");
    assert_eq!(held.len(), 150_000, "a scan holding the old snapshot is not disturbed");
    assert_eq!(texts(&lines).last().unwrap(), "line 150009");
    assert_eq!(lines.get(70_000), "line 70000");
    assert_eq!(held.get(149_999), "line 149999");
}

#[test]
fn it_starts_again_when_the_rows_left_or_moved() {
    let mut rows = entries(0..1_000);
    let mut lines = SearchLines::default();
    lines.sync(&rows, 0);
    rows.drain(0..10);
    assert_eq!(lines.sync(&rows, 1), 990, "another epoch: every row is copied again");
    assert_eq!(lines.get(0), "line 10");
    assert_eq!(lines.len(), 990);
}
