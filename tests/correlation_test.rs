use std::collections::HashMap;

use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::workspace::correlation::{CorrelationResult, nearest_timestamp_index, search_across_files};

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
