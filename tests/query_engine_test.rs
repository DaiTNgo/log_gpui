use log_gpui::infrastructure::filter_engine::FilterEngine;
use log_gpui::infrastructure::search_engine::{SearchEngine, SearchMatch};
use log_gpui::parser::event::{LogEvent, LogValue};
use log_gpui::query::engine::QueryEngine;
use log_gpui::query::parser::parse;
use std::collections::HashMap;

fn event_with(fields: Vec<(&str, LogValue)>) -> LogEvent {
    let mut event = LogEvent::new(1, 100.0, "10:00:00.000".into(), "processor timeout".into(), "raw line".into());
    for (name, value) in fields {
        event.set_field(name, value);
    }
    event
}

fn log_value(s: &str) -> LogValue {
    LogValue::Str(s.to_string())
}

#[test]
fn test_eq_string_matches_case_insensitive() {
    let event = event_with(vec![("level", log_value("ERROR"))]);
    let expr = parse(r#"level == "error""#).unwrap();
    assert!(QueryEngine::evaluate_event(&event, &expr));
}

#[test]
fn test_neq_matches_when_different() {
    let event = event_with(vec![("level", log_value("INFO"))]);
    let expr = parse(r#"level != "error""#).unwrap();
    assert!(QueryEngine::evaluate_event(&event, &expr));
    let error_event = event_with(vec![("level", log_value("ERROR"))]);
    assert!(!QueryEngine::evaluate_event(&error_event, &expr));
}

#[test]
fn test_contains_case_insensitive() {
    let event = event_with(vec![("message", log_value("Connection timeout after retry"))]);
    let expr = parse(r#"message contains "TIMEOUT""#).unwrap();
    assert!(QueryEngine::evaluate_event(&event, &expr));
}

#[test]
fn test_starts_with_and_ends_with() {
    let event = event_with(vec![("module", log_value("api-gateway"))]);
    let starts = parse(r#"module startsWith "api""#).unwrap();
    let ends = parse(r#"module endsWith "gateway""#).unwrap();
    let no_starts = parse(r#"module startsWith "db""#).unwrap();
    assert!(QueryEngine::evaluate_event(&event, &starts));
    assert!(QueryEngine::evaluate_event(&event, &ends));
    assert!(!QueryEngine::evaluate_event(&event, &no_starts));
}

#[test]
fn test_matches_regex() {
    let event = event_with(vec![("message", log_value("GET /api/users 500"))]);
    let expr = parse(r#"message matches "GET /api/users \d{3}""#).unwrap();
    assert!(QueryEngine::evaluate_event(&event, &expr));
    let no_match = parse(r#"message matches "^POST""#).unwrap();
    assert!(!QueryEngine::evaluate_event(&event, &no_match));
}

#[test]
fn test_number_eq_and_neq() {
    let event = event_with(vec![("duration", LogValue::Number(1500.0))]);
    let eq = parse(r#"duration == 1500"#).unwrap();
    let neq = parse(r#"duration != 1000"#).unwrap();
    let wrong = parse(r#"duration == 1000"#).unwrap();
    assert!(QueryEngine::evaluate_event(&event, &eq));
    assert!(QueryEngine::evaluate_event(&event, &neq));
    assert!(!QueryEngine::evaluate_event(&event, &wrong));
}

#[test]
fn test_boolean_eq() {
    let event = event_with(vec![("retry", LogValue::Bool(true))]);
    let expr = parse(r#"retry == true"#).unwrap();
    assert!(QueryEngine::evaluate_event(&event, &expr));
}

#[test]
fn test_boolean_rejects_string_literal() {
    let event = event_with(vec![("retry", LogValue::Bool(true))]);
    // Boolean fields only match boolean literals; a string query is a non-match.
    let expr = parse(r#"retry == "true""#).unwrap();
    assert!(!QueryEngine::evaluate_event(&event, &expr));
}

#[test]
fn test_missing_field_returns_false() {
    let event = event_with(vec![("level", log_value("INFO"))]);
    let expr = parse(r#"nonexistent == "anything""#).unwrap();
    assert!(!QueryEngine::evaluate_event(&event, &expr));
}

#[test]
fn test_and_group() {
    let event = event_with(vec![
        ("level", log_value("ERROR")),
        ("module", log_value("api")),
    ]);
    let expr = parse(r#"level == "error" && module == "api""#).unwrap();
    assert!(QueryEngine::evaluate_event(&event, &expr));
    let not_match = parse(r#"level == "error" && module == "db""#).unwrap();
    assert!(!QueryEngine::evaluate_event(&event, &not_match));
}

#[test]
fn test_or_group() {
    let event = event_with(vec![("level", log_value("WARN"))]);
    let expr = parse(r#"level == "error" || level == "warn""#).unwrap();
    assert!(QueryEngine::evaluate_event(&event, &expr));
}

#[test]
fn test_nested_groups() {
    let event = event_with(vec![
        ("level", log_value("ERROR")),
        ("module", log_value("db")),
        ("message", log_value("query failed")),
    ]);
    let expr = parse(r#"(level == "error" && module == "db") || message contains "timeout""#).unwrap();
    assert!(QueryEngine::evaluate_event(&event, &expr));
    let stricter = parse(r#"(level == "error" && module == "db") && message contains "timeout""#).unwrap();
    assert!(!QueryEngine::evaluate_event(&event, &stricter));
}

#[test]
fn test_in_desugars_to_or_eq() {
    let event = event_with(vec![("level", log_value("WARN"))]);
    let expr = parse(r#"level in (info, warn)"#).unwrap();
    assert!(QueryEngine::evaluate_event(&event, &expr));
    let info_event = event_with(vec![("level", log_value("INFO"))]);
    assert!(QueryEngine::evaluate_event(&info_event, &expr));
    let error_event = event_with(vec![("level", log_value("ERROR"))]);
    assert!(!QueryEngine::evaluate_event(&error_event, &expr));
}

#[test]
fn test_message_field_from_event_payload() {
    let mut event = LogEvent::new(1, 100.0, "10:00:00.000".into(), "payment rejected".into(), "raw".into());
    event.set_field("level", log_value("ERROR"));
    let expr = parse(r#"message contains "rejected""#).unwrap();
    assert!(QueryEngine::evaluate_event(&event, &expr));
}

#[test]
fn test_evaluate_entry_bridges_to_logentry() {
    let entry = log_gpui::model::LogEntry::new(
        1,
        100.0,
        "10:00:00.000".into(),
        "api".into(),
        log_gpui::model::LogLevel::Error,
        "connection timeout".into(),
        "raw".into(),
    );
    let expr = parse(r#"module == "api" && level == "ERROR""#).unwrap();
    assert!(QueryEngine::evaluate_entry(&entry, &expr));
    let wrong = parse(r#"module == "db""#).unwrap();
    assert!(!QueryEngine::evaluate_entry(&entry, &wrong));
}

#[test]
fn test_evaluate_fields_shared_path() {
    let mut fields = HashMap::new();
    fields.insert("level".to_string(), log_value("ERROR"));
    fields.insert("duration".to_string(), LogValue::Number(5.0));
    let expr = parse(r#"level == "error" && duration == 5"#).unwrap();
    assert!(QueryEngine::evaluate_fields(&fields, &expr));
}

#[test]
fn test_search_match_has_event_id_and_ranges() {
    let make = |index: usize, message: &str| {
        let mut event = LogEvent::new(index, index as f64, "10:00:00.000".into(), message.into(), message.into());
        event.set_field("level", log_value("ERROR"));
        event
    };
    let events = vec![make(10, "timeout in gateway"), make(20, "all good"), make(30, "timeout in db")];
    let expr = parse(r#"message contains "timeout""#).unwrap();
    let matches: Vec<SearchMatch> = SearchEngine::find_matches(&events, &expr);
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].event_id, 10);
    assert_eq!(matches[1].event_id, 30);
    assert!(!matches[0].ranges.is_empty());
    for (start, end) in &matches[0].ranges {
        assert!(end > start);
    }
}

#[test]
fn test_search_ranges_point_at_literal() {
    let mut event = LogEvent::new(1, 1.0, "10:00:00.000".into(), "x timeout y timeout".into(), "x timeout y timeout".into());
    event.set_field("level", log_value("ERROR"));
    let expr = parse(r#"message contains "timeout""#).unwrap();
    let matches = SearchEngine::find_matches(&[event], &expr);
    assert_eq!(matches[0].ranges.len(), 2);
    for (start, end) in &matches[0].ranges {
        assert_eq!(&"x timeout y timeout"[*start..*end], "timeout");
    }
}

#[test]
fn test_filter_engine_visible_indices() {
    let make = |index: usize, level: &str| {
        let mut event = LogEvent::new(index, index as f64, "10:00:00.000".into(), "m".into(), "m".into());
        event.set_field("level", log_value(level));
        event
    };
    let events = vec![make(1, "ERROR"), make(2, "INFO"), make(3, "ERROR")];
    let expr = parse(r#"level == "error""#).unwrap();
    let visible = FilterEngine::get_visible(&events, &expr);
    assert_eq!(visible, vec![0, 2]);
}