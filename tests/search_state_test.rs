use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::search::SearchState;

fn entry(i: usize, raw: &str) -> LogEntry {
    LogEntry::new(i, i as f64, format!("{i}"), "API".into(), LogLevel::Info, raw.into(), raw.into())
}

fn entries() -> Vec<LogEntry> {
    vec![
        entry(0, "10:00 INFO api connection refused"),
        entry(1, "10:01 ERROR db timeout"),
        entry(2, "10:02 DEBUG db timeout retry"),
        entry(3, "10:03 INFO ok"),
    ]
}

fn all(n: usize) -> Vec<usize> {
    (0..n).collect()
}

#[test]
fn matches_the_whole_raw_line() {
    let e = entries();
    let mut s = SearchState::new();
    s.query = "error".into();
    s.update_matches(&e, &all(4));
    assert_eq!(*s.matched_visible_indices, vec![1]);
}

#[test]
fn boolean_expression() {
    let e = entries();
    let mut s = SearchState::new();
    s.query = "(refused || timeout) && db".into();
    s.update_matches(&e, &all(4));
    assert_eq!(*s.matched_visible_indices, vec![1, 2]);
}

#[test]
fn lines_failing_filter_are_still_matches() {
    // 5 DEBUG lines and 12 other lines contain `timeout`; Filter would hide DEBUG in the pane only.
    let mut e = Vec::new();
    for i in 0..17 {
        let level = if i < 5 { "DEBUG" } else { "INFO" };
        e.push(entry(i, &format!("{level} db timeout {i}")));
    }
    let rows: Vec<usize> = (0..17).collect();
    let mut s = SearchState::new();
    s.query = "timeout".into();
    s.update_matches(&e, &rows);
    assert_eq!(s.total_matches(), 17);
    // Filter does not touch the view's Matches; a Match is the entry's own position.
    assert_eq!(s.matched_visible_indices.first(), Some(&0));
    assert!(s.matched_visible_indices.contains(&2));
}

#[test]
fn invalid_expression_keeps_last_matches_and_reports_error() {
    let e = entries();
    let mut s = SearchState::new();
    s.query = "timeout".into();
    s.update_matches(&e, &all(4));
    assert_eq!(*s.matched_visible_indices, vec![1, 2]);
    s.next_match();

    s.query = "timeout ||".into();
    s.update_matches(&e, &all(4));
    assert!(s.parse_error.is_some());
    assert_eq!(*s.matched_visible_indices, vec![1, 2]);
    assert_eq!(s.current_match_idx, 1);

    s.query = "timeout || refused".into();
    s.update_matches(&e, &all(4));
    assert!(s.parse_error.is_none());
    assert_eq!(*s.matched_visible_indices, vec![0, 1, 2]);
    assert_eq!(s.current_match_idx, 0);
}

#[test]
fn never_valid_expression_has_no_matches() {
    let e = entries();
    let mut s = SearchState::new();
    s.query = "(timeout".into();
    s.update_matches(&e, &all(4));
    assert!(s.parse_error.is_some());
    assert!(s.matched_visible_indices.is_empty());
}

#[test]
fn empty_query_clears_matches_and_error() {
    let e = entries();
    let mut s = SearchState::new();
    s.query = "timeout".into();
    s.update_matches(&e, &all(4));
    s.query = "  ".into();
    s.update_matches(&e, &all(4));
    assert!(s.matched_visible_indices.is_empty());
    assert!(s.parse_error.is_none());
    assert_eq!(s.current_match_display(), (0, 0));
}

#[test]
fn navigation_wraps() {
    let e = entries();
    let mut s = SearchState::new();
    s.query = "timeout".into();
    s.update_matches(&e, &all(4));
    assert_eq!(s.current_match_display(), (1, 2));
    assert_eq!(s.next_match(), Some(2));
    assert_eq!(s.next_match(), Some(1));
    assert_eq!(s.prev_match(), Some(2));
    assert_eq!(s.current_match_display(), (2, 2));
}

#[test]
fn highlight_ranges_follow_the_last_valid_expression() {
    let mut s = SearchState::new();
    s.query = "timeout".into();
    s.reparse();
    assert_eq!(s.find_ranges("x timeout y TIMEOUT z").len(), 2);
    s.query = "timeout &&".into();
    s.reparse();
    assert_eq!(s.find_ranges("x timeout").len(), 1);
}
