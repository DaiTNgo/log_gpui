use log_gpui::marker::MarkerType;
use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::search::SearchState;

#[test]
fn test_search_find_modes() {
    let entries = vec![
        LogEntry::new(1, 10.0, "10.0".into(), "API".into(), LogLevel::Info, "Connection timeout in pool".into(), "10.0 API Connection timeout in pool".into()),
        LogEntry::new(2, 10.1, "10.1".into(), "DB".into(), LogLevel::Info, "Timeout reached".into(), "10.1 DB Timeout reached".into()),
        LogEntry::new(3, 10.2, "10.2".into(), "API".into(), LogLevel::Info, "all good".into(), "10.2 API all good".into()),
    ];
    let visible_indices = vec![0, 1, 2];

    let mut search = SearchState::new();
    search.query = "timeout".into();

    search.update_matches(&entries, &visible_indices);
    // Finds row 0 and 1 (case insensitive)
    assert_eq!(search.total_matches(), 2);
    assert_eq!(search.matched_visible_indices, vec![0, 1]);

    // Test navigation next/prev
    assert_eq!(search.current_match_idx, 0);
    let next_match = search.next_match();
    assert_eq!(next_match, Some(1));
    assert_eq!(search.current_match_idx, 1);

    let next_match_wrapped = search.next_match();
    assert_eq!(next_match_wrapped, Some(0));
    assert_eq!(search.current_match_idx, 0);
}

#[test]
fn test_markers_and_bookmarks() {
    let mut entry = LogEntry::new(
        10234,
        100.0,
        "10:32:02.441".into(),
        "API".into(),
        LogLevel::Error,
        "Connection timeout".into(),
        "raw line".into(),
    );

    // Passive marker auto-detected from level ERROR
    assert_eq!(entry.effective_marker(), Some(MarkerType::Error));
    assert_eq!(entry.effective_marker().unwrap().symbol(), "●");

    // Toggle bookmark
    assert!(!entry.is_bookmarked());
    entry.toggle_bookmark();
    assert!(entry.is_bookmarked());
    assert_eq!(entry.effective_marker(), Some(MarkerType::Bookmark));
    assert_eq!(entry.effective_marker().unwrap().symbol(), "◆");

    // Toggle bookmark off restores passive marker
    entry.toggle_bookmark();
    assert!(!entry.is_bookmarked());
    assert_eq!(entry.effective_marker(), Some(MarkerType::Error));
}

#[test]
fn test_marker_cycling() {
    // 1. Test MarkerType::cycle
    let mut marker = None;
    marker = MarkerType::cycle(marker);
    assert_eq!(marker, Some(MarkerType::Bookmark));
    marker = MarkerType::cycle(marker);
    assert_eq!(marker, Some(MarkerType::Important));
    marker = MarkerType::cycle(marker);
    assert_eq!(marker, Some(MarkerType::Note));
    marker = MarkerType::cycle(marker);
    assert_eq!(marker, None);
}
