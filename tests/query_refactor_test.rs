use log_gpui::filter::{ConditionField, ConditionOp, FilterEngine, LogicMode};
use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::search::SearchState;

fn entry(index: usize, level: LogLevel, tag: &str, payload: &str, raw: &str) -> LogEntry {
    LogEntry::new(
        index,
        index as f64,
        format!("{:.3}", index as f64),
        tag.into(),
        level,
        payload.into(),
        raw.into(),
    )
}

fn fixtures() -> Vec<LogEntry> {
    vec![
        entry(1, LogLevel::Info, "API", "Request started", "req started"),
        entry(2, LogLevel::Error, "API", "Connection timeout", "conn timeout"),
        entry(3, LogLevel::Warn, "DB", "Query users slow", "query slow"),
        entry(4, LogLevel::Error, "DB", "Connection refused", "conn refused"),
    ]
}

#[test]
fn test_filter_delegates_to_query_engine() {
    let mut entries = fixtures();
    let mut engine = FilterEngine::new();
    engine.add_condition(ConditionField::Level, ConditionOp::Is, "ERROR".into());
    let visible = engine.evaluate(&mut entries);
    assert_eq!(visible, vec![1, 3]);

    // The built expression uses query-language semantics (level == "ERROR").
    let expr = engine.to_query_expression().expect("mappable to AST");
    for (idx, entry) in entries.iter().enumerate() {
        assert_eq!(
            log_gpui::query::engine::QueryEngine::evaluate_entry(entry, &expr),
            visible.contains(&idx),
            "index {idx} should agree"
        );
    }
}

#[test]
fn test_filter_all_any_delegate_to_query_engine() {
    // ALL: level ERROR AND module DB
    let mut entries = fixtures();
    let mut engine = FilterEngine::new();
    engine.add_condition(ConditionField::Level, ConditionOp::Is, "ERROR".into());
    engine.add_condition(ConditionField::Module, ConditionOp::Is, "DB".into());
    engine.logic_mode = LogicMode::All;
    assert!(engine.to_query_expression().is_some());
    let visible_all = engine.evaluate(&mut entries);
    assert_eq!(visible_all, vec![3]);

    // ANY: module DB OR level WARN
    let mut entries = fixtures();
    let mut engine2 = FilterEngine::new();
    engine2.add_condition(ConditionField::Module, ConditionOp::Is, "DB".into());
    engine2.add_condition(ConditionField::Level, ConditionOp::Is, "WARN".into());
    engine2.logic_mode = LogicMode::Any;
    let visible_any = engine2.evaluate(&mut entries);
    assert_eq!(visible_any, vec![2, 3]);
}

#[test]
fn test_filter_not_contains_falls_back_to_legacy() {
    let mut entries = fixtures();
    let mut engine = FilterEngine::new();
    engine.add_condition(ConditionField::Message, ConditionOp::NotContains, "connection".into());
    // NotContains has no AST equivalent → legacy per-row path, same results.
    assert!(engine.to_query_expression().is_none());
    let visible = engine.evaluate(&mut entries);
    assert_eq!(visible, vec![0, 2]); // "Request started", "Query users slow"
}

#[test]
fn test_filter_contains_and_regex_still_work() {
    let mut entries = fixtures();
    let mut engine = FilterEngine::new();
    engine.add_condition(ConditionField::Message, ConditionOp::Contains, "timeout".into());
    assert_eq!(engine.evaluate(&mut entries), vec![1]);

    let mut entries = fixtures();
    let mut engine2 = FilterEngine::new();
    engine2.add_condition(ConditionField::Message, ConditionOp::MatchesRegex, "^Query".into());
    assert_eq!(engine2.evaluate(&mut entries), vec![2]);
}

#[test]
fn test_search_query_language_uses_engine() {
    let entries = fixtures();
    let visible = vec![0, 1, 2, 3];

    let mut search = SearchState::new();
    search.query = r#"level == "error""#.into();
    search.update_matches(&entries, &visible);
    assert!(search.parsed_query.is_some());
    assert_eq!(search.matched_visible_indices, vec![1, 3]);
}

#[test]
fn test_search_and_query_language() {
    let entries = fixtures();
    let visible = vec![0, 1, 2, 3];

    let mut search = SearchState::new();
    search.query = r#"module == "db" && level == "error""#.into();
    search.update_matches(&entries, &visible);
    assert_eq!(search.matched_visible_indices, vec![3]);
}

#[test]
fn test_search_invalid_query_sets_parse_error_and_no_matches() {
    let entries = fixtures();
    let visible = vec![0, 1, 2, 3];

    let mut search = SearchState::new();
    // Bare text is not valid query language → parse error, no matches.
    search.query = "timeout".into();
    search.update_matches(&entries, &visible);
    assert!(search.parsed_query.is_none());
    assert!(search.parse_error.is_some());
    assert!(search.parse_error.unwrap().message().contains("expected"));
    assert!(search.matched_visible_indices.is_empty());
}

#[test]
fn test_search_ranges_use_engine_in_query_mode() {
    let mut search = SearchState::new();
    search.query = r#"message contains "timeout""#.into();
    search.reparse();
    assert!(search.parsed_query.is_some());
    let ranges = search.find_ranges("x timeout y TIMEOUT z");
    assert_eq!(ranges.len(), 2);
    for (start, end) in ranges {
        assert_eq!(&"x timeout y TIMEOUT z"[start..end].to_lowercase(), "timeout");
    }
}

#[test]
fn test_filter_row_type_replaced_condition() {
    // FilterCondition is gone; the row model is FilterRow and stays UI-driven.
    let mut engine = FilterEngine::new();
    let id = engine.add_condition(ConditionField::Level, ConditionOp::Is, "WARN".into());
    assert!(id > 0);
    assert_eq!(engine.conditions.len(), 1);
    assert_eq!(engine.conditions[0].to_chip_label(), "WARN");
    engine.remove_condition(id);
    assert!(engine.conditions.is_empty());
}