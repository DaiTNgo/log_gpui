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
