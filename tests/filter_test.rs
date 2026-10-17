use log_gpui::filter::{ConditionField, ConditionOp, FilterEngine, LogicMode};
use log_gpui::marker::MarkerType;
use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::search::SearchState;

#[test]
fn test_filter_engine_level_and_module() {
    let mut entries = vec![
        LogEntry::new(1, 100.0, "100.0".into(), "API".into(), LogLevel::Info, "Request started".into(), "raw 1".into()),
        LogEntry::new(2, 100.1, "100.1".into(), "DB".into(), LogLevel::Debug, "Query users".into(), "raw 2".into()),
        LogEntry::new(3, 100.2, "100.2".into(), "API".into(), LogLevel::Error, "Connection timeout".into(), "raw 3".into()),
        LogEntry::new(4, 100.3, "100.3".into(), "AUTH".into(), LogLevel::Warn, "Token expired".into(), "raw 4".into()),
    ];

    let mut engine = FilterEngine::new();
    // 1. Filter only ERROR
    engine.add_quick_level(LogLevel::Error);
    let visible = engine.evaluate(&mut entries);

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], 2); // Entry index 3 (0-based 2)

    // 2. Filter Module API
    let mut engine2 = FilterEngine::new();
    engine2.add_quick_module("API");
    let visible2 = engine2.evaluate(&mut entries);

    assert_eq!(visible2.len(), 2);
    assert_eq!(visible2, vec![0, 2]);

    // 3. Nested ALL (Level is ERROR AND Module is API)
    let mut engine3 = FilterEngine::new();
    engine3.add_condition(ConditionField::Level, ConditionOp::Is, "ERROR".into());
    engine3.add_condition(ConditionField::Module, ConditionOp::Is, "API".into());
    engine3.logic_mode = LogicMode::All;
    let visible3 = engine3.evaluate(&mut entries);

    assert_eq!(visible3.len(), 1);
    assert_eq!(visible3[0], 2);

    // 4. Nested ANY (Level is ERROR OR Level is WARN)
    let mut engine4 = FilterEngine::new();
    engine4.add_condition(ConditionField::Level, ConditionOp::Is, "ERROR".into());
    engine4.add_condition(ConditionField::Level, ConditionOp::Is, "WARN".into());
    engine4.logic_mode = LogicMode::Any;
    let visible4 = engine4.evaluate(&mut entries);

    assert_eq!(visible4.len(), 2);
    assert_eq!(visible4, vec![2, 3]);
}

#[test]
fn test_search_find_modes() {
    let entries = vec![
        LogEntry::new(1, 10.0, "10.0".into(), "API".into(), LogLevel::Info, "Connection timeout in pool".into(), "raw 1".into()),
        LogEntry::new(2, 10.1, "10.1".into(), "DB".into(), LogLevel::Info, "Timeout reached".into(), "raw 2".into()),
        LogEntry::new(3, 10.2, "10.2".into(), "API".into(), LogLevel::Info, "all good".into(), "raw 3".into()),
    ];
    let visible_indices = vec![0, 1, 2];

    let mut search = SearchState::new();
    search.query = "message contains timeout".into();

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
fn test_marker_cycling_and_filter_id_monotonic() {
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

    // 2. Test monotonic filter IDs and ConditionOp::Equals / NotEquals aliases
    let mut engine = FilterEngine::new();
    let id1 = engine.add_condition(ConditionField::Level, ConditionOp::Equals, "INFO".into());
    let id2 = engine.add_condition(ConditionField::Module, ConditionOp::NotEquals, "API".into());
    assert!(id2 > id1);
    assert_eq!(engine.conditions[0].op, ConditionOp::Is);
    assert_eq!(engine.conditions[1].op, ConditionOp::IsNot);
}
