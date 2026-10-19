//! Stream as a column of Filter, Hide Mode and Highlight Rules.

use log_gpui::filter::FilterState;
use log_gpui::filter_index::FilterIndex;
use log_gpui::highlight::{HighlightRule, count_matches, paint_for_row};
use log_gpui::model::{LogEntry, LogLevel, Stream};
use log_gpui::palette::{HighlightColor, Hue, Shade};

fn line(index: usize, text: &str, stream: Option<Stream>) -> LogEntry {
    let mut e = LogEntry::new(index, index as f64, format!("{index}"), "".into(), LogLevel::Info, text.into(), text.into());
    e.stream = stream;
    e
}

/// A command's lines among a file's: stdout, stderr, a stderr Continuation
/// Line of a stdout line, and two lines of a file.
fn rows() -> Vec<LogEntry> {
    let mut rows = vec![
        line(1, "GET /a 500", Some(Stream::Stdout)),
        line(2, "  at handler (a.js:3)", Some(Stream::Stderr)),
        line(3, "panic: nil map", Some(Stream::Stderr)),
        line(4, "from a file", None),
        line(5, "GET /b 200", Some(Stream::Stdout)),
    ];
    rows[1].parent = Some(1);
    rows[1].has_time = false;
    rows
}

fn visible(filter: &FilterState, rows: &mut [LogEntry]) -> Vec<usize> {
    let index = FilterIndex::build(rows);
    filter.evaluate(&index, rows)
}

#[test]
fn stream_is_a_filterable_column_with_stdout_and_stderr_counted() {
    let rows = rows();
    let index = FilterIndex::build(&rows);
    let stream = index.column("stream").expect("a column when lines have a Stream");
    assert_eq!(stream.label, "Stream");
    let counts: Vec<(&str, usize)> = stream.values.iter().map(|v| (v.display.as_str(), v.count)).collect();
    // The Continuation Line counts under its own Stream.
    assert_eq!(counts, vec![("stdout", 2), ("stderr", 2)]);
}

#[test]
fn a_log_space_of_files_only_has_no_stream_column() {
    let rows = vec![line(1, "a", None), line(2, "b", None)];
    let index = FilterIndex::build(&rows);
    assert!(index.columns_in_use().all(|c| c.key != "stream"));
}

#[test]
fn ticking_stderr_lists_only_the_stderr_lines_not_the_file_lines() {
    let mut rows = rows();
    let mut f = FilterState::new();
    f.toggle_value("stream", "stderr");
    // Rows 1 and 2: the Continuation Line has its own Stream, not its parent's.
    assert_eq!(visible(&f, &mut rows), vec![1, 2]);
}

#[test]
fn ticking_stdout_does_not_list_a_stderr_continuation_of_a_stdout_line() {
    let mut rows = rows();
    let mut f = FilterState::new();
    f.toggle_value("stream", "stdout");
    assert_eq!(visible(&f, &mut rows), vec![0, 4]);
}

#[test]
fn hiding_stdout_keeps_stderr_lines_and_every_line_of_a_file() {
    let mut rows = rows();
    let mut f = FilterState::new();
    f.toggle_value("stream", "stdout");
    f.set_hide("stream", true);
    assert_eq!(visible(&f, &mut rows), vec![1, 2, 3]);
}

#[test]
fn a_highlight_rule_on_stream_paints_the_stderr_rows_only() {
    let rows = rows();
    let red = HighlightColor::palette(Hue::Red, Shade::Light);
    let rule = HighlightRule::new(1, "stream", &["stderr".to_string()], &[], red).unwrap();
    let painted: Vec<bool> = (0..rows.len()).map(|i| paint_for_row(&[rule.clone()], &rows, i).is_some()).collect();
    assert_eq!(painted, vec![false, true, true, false, false]);
    // Counted by its own condition on the lines that are not Continuation Lines.
    assert_eq!(count_matches(&rule, &rows, &[0, 1, 2, 3, 4]), 1);
}
