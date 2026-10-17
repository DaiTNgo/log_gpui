use std::collections::HashMap;

use log_gpui::column::PinnableColumn;
use log_gpui::filter::{ConditionField, ConditionOp};
use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::workspace::saved_view::{
    CorrelationResult, SavedView, nearest_timestamp_index, search_across_files,
};
use log_gpui::workspace::working_set::TabSessionState;

fn entry(index: usize, ts: f64, tag: &str, level: LogLevel, payload: &str, request_id: Option<&str>) -> LogEntry {
    let mut e = LogEntry::new(
        index,
        ts,
        format!("ts-{}", index),
        tag.to_string(),
        level,
        payload.to_string(),
        format!("[{}] {} {}", tag, level.as_str(), payload),
    );
    e.request_id = request_id.map(|s| s.to_string());
    e.trace_id = request_id.map(|s| format!("trace-{}", s));
    e
}

fn session_with_layout() -> TabSessionState {
    let mut session = TabSessionState::new();

    session
        .filter
        .add_condition(ConditionField::Level, ConditionOp::Is, "ERROR".to_string());
    session
        .filter
        .add_condition(ConditionField::Message, ConditionOp::Contains, "timeout".to_string());
    session.search.query = "gateway".to_string();
    session.column_layout.pin(PinnableColumn::Timestamp);
    session.column_layout.pin(PinnableColumn::Module);
    session.column_layout.set_width(PinnableColumn::Level, gpui::px(72.0));

    session
}

// ─── Saved View Capture / Apply ──────────────────────────────────────────────

#[test]
fn test_saved_view_captures_session() {
    let session = session_with_layout();
    let view = SavedView::capture("v1".into(), "5xx Timeout".into(), &session);

    assert_eq!(view.name, "5xx Timeout");
    assert_eq!(view.filter.conditions.len(), 2);
    assert_eq!(view.search_query, "gateway");
    assert_eq!(view.pinned_columns, vec![PinnableColumn::Timestamp, PinnableColumn::Module]);
    assert!(view.column_widths.contains_key(&PinnableColumn::Level));
}

#[test]
fn test_saved_view_apply_restores_filter_search_columns() {
    let mut session = TabSessionState::new();
    session.search.query = "garbage".to_string();

    let view = SavedView::capture("v1".into(), "Errors".into(), &session_with_layout());
    view.apply_to_session(&mut session);

    assert_eq!(session.filter.conditions.len(), 2);
    assert!(session
        .filter
        .conditions
        .iter()
        .any(|c| c.field == ConditionField::Level && c.value == "ERROR"));
    assert_eq!(session.search.query, "gateway");
    assert!(session.column_layout.is_pinned(PinnableColumn::Timestamp));
    assert!(session.column_layout.is_pinned(PinnableColumn::Module));
    assert_eq!(session.column_layout.width(PinnableColumn::Level).as_f32(), 72.0);
}

#[test]
fn test_saved_view_capture_with_empty_session_has_defaults() {
    let session = TabSessionState::new();
    let view = SavedView::capture("v2".into(), "Default".into(), &session);

    assert!(view.filter.conditions.is_empty());
    assert!(view.search_query.is_empty());
    assert!(view.pinned_columns.is_empty());
}

// ─── Cross-Log Correlation Search ────────────────────────────────────────────

fn build_entries_by_file() -> HashMap<String, Vec<LogEntry>> {
    let mut map = HashMap::new();
    map.insert(
        "api.log".into(),
        vec![
            entry(1, 1000.0, "api", LogLevel::Error, "timeout to upstream", Some("req-xyz")),
            entry(2, 1010.0, "api", LogLevel::Info, "healthy", Some("req-abc")),
        ],
    );
    map.insert(
        "worker.log".into(),
        vec![
            entry(1, 1005.0, "worker", LogLevel::Warn, "processing req-xyz", Some("req-xyz")),
            entry(2, 1008.0, "worker", LogLevel::Info, "done", None),
        ],
    );
    map.insert(
        "nginx.log".into(),
        vec![entry(1, 990.0, "nginx", LogLevel::Info, "GET /pay req-xyz", None)],
    );
    map
}

#[test]
fn test_correlation_search_groups_by_file() {
    let entries = build_entries_by_file();
    let result: CorrelationResult = search_across_files("req-xyz", &entries, false);

    assert_eq!(result.trace_id, "req-xyz");
    assert_eq!(result.total_hits(), 3);
    assert_eq!(result.files_matched(), 3);
    assert_eq!(result.match_count("api.log"), 1);
    assert_eq!(result.match_count("worker.log"), 1);
    assert_eq!(result.match_count("nginx.log"), 1);
    assert_eq!(result.match_count("db.log"), 0);
}

#[test]
fn test_correlation_search_empty_query_returns_empty() {
    let entries = build_entries_by_file();
    let result: CorrelationResult = search_across_files("", &entries, false);
    assert_eq!(result.total_hits(), 0);
    assert_eq!(result.files_matched(), 0);
}

#[test]
fn test_correlation_search_no_matches() {
    let entries = build_entries_by_file();
    let result: CorrelationResult = search_across_files("nonexistent-id", &entries, false);
    assert_eq!(result.total_hits(), 0);
    assert_eq!(result.files_matched(), 0);
}

#[test]
fn test_correlation_search_regex_mode() {
    let entries = build_entries_by_file();
    let result: CorrelationResult = search_across_files(r"req-\w{3}", &entries, true);
    assert!(result.total_hits() >= 3);
}

#[test]
fn test_correlation_hits_sorted_chronologically() {
    let entries = build_entries_by_file();
    let result: CorrelationResult = search_across_files("req-xyz", &entries, false);

    // nginx (990) -> api (1000) -> worker (1005)
    let ts: Vec<f64> = result.hits.iter().map(|h| h.timestamp).collect();
    assert_eq!(ts, vec![990.0, 1000.0, 1005.0]);

    // Each hit preserves its source file and line
    assert_eq!(result.hits[0].file_id, "nginx.log");
    assert_eq!(result.hits[0].line_number, 1);
    assert_eq!(result.hits[1].file_id, "api.log");
    assert_eq!(result.hits[2].file_id, "worker.log");
}

// ─── Timestamp Alignment (Synchronize Scrolling) ─────────────────────────────

#[test]
fn test_nearest_timestamp_index_basic() {
    let timestamps = vec![100.0, 200.0, 300.0, 400.0, 500.0];

    assert_eq!(nearest_timestamp_index(&timestamps, 100.0), Some(0));
    assert_eq!(nearest_timestamp_index(&timestamps, 500.0), Some(4));
    assert_eq!(nearest_timestamp_index(&timestamps, 250.0), Some(1));
    assert_eq!(nearest_timestamp_index(&timestamps, 349.0), Some(2));
    assert_eq!(nearest_timestamp_index(&timestamps, 351.0), Some(3));
    assert_eq!(nearest_timestamp_index(&timestamps, -100.0), Some(0));
    assert_eq!(nearest_timestamp_index(&timestamps, 9999.0), Some(4));
}

#[test]
fn test_nearest_timestamp_index_handles_empty() {
    let timestamps: Vec<f64> = vec![];
    assert_eq!(nearest_timestamp_index(&timestamps, 50.0), None);
}

#[test]
fn test_nearest_timestamp_aligns_different_length_files() {
    let api_ts = vec![1000.0, 1001.0, 1002.0, 1003.0, 1004.0];
    let worker_ts = vec![1000.5, 1001.5, 1002.5];

    // Scroll api to ts=1002.0 -> align worker viewport
    let api_index = nearest_timestamp_index(&api_ts, 1002.0).unwrap();
    let worker_index = nearest_timestamp_index(&worker_ts, api_ts[api_index]);

    assert_eq!(api_index, 2);
    assert_eq!(worker_index, Some(1)); // 1001.5 is closest to 1002.0
}

// ─── Strategy: applying a saved view then re-running filters ─────────────────

#[test]
fn test_applied_saved_view_survives_filter_evaluation() {
    let mut session = TabSessionState::new();
    let view = SavedView::capture("v1".into(), "Errors".into(), &session_with_layout());
    view.apply_to_session(&mut session);

    let mut entries = vec![
        entry(1, 1.0, "api", LogLevel::Error, "gateway timeout", Some("r1")),
        entry(2, 2.0, "api", LogLevel::Info, "ok", Some("r2")),
    ];

    let visible = session.filter.evaluate(&mut entries);
    assert_eq!(visible.len(), 1);
    assert_eq!(entries[visible[0]].index, 1);
}