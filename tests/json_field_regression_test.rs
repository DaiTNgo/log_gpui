//! What Filter and Highlight Rules see of the fields of JSON lines. Written
//! before JSON fields moved from copied text to spans of the line, and kept
//! so the move changes none of it.

use log_gpui::filter::FilterState;
use log_gpui::filter_index::FilterIndex;
use log_gpui::highlight::{HighlightRule, paint_for_row};
use log_gpui::model::LogEntry;
use log_gpui::palette::{HighlightColor, Hue, Shade};
use log_gpui::parser::text::parse_text_bytes;

fn rows() -> Vec<LogEntry> {
    parse_text_bytes(
        br#"{"time":"2026-10-18T10:00:01Z","level":"info","service":"api","msg":"start","region":"us","latencyMs":12,"note":"say \"hi\"","requestId":"r-1"}
{"time":"2026-10-18T10:00:02Z","level":"error","service":"db","msg":"boom","region":"eu","latencyMs":250,"retry":true}
  at Pool.get(Pool.java:10)
  at Db.query(Db.java:20)
{"time":"2026-10-18T10:00:03Z","level":"info","service":"api","msg":"ok","region":"EU","gone":null}
{"time":"2026-10-18T10:00:04Z","level":"warn","service":"api","msg":"slow"}
"#,
    )
    .unwrap()
}

fn visible(f: &FilterState, entries: &mut [LogEntry]) -> Vec<usize> {
    let index = FilterIndex::build(entries);
    f.evaluate(&index, entries)
}

#[test]
fn json_keys_are_filter_columns_by_key_name() {
    let index = FilterIndex::build(&rows());
    let mut keys: Vec<&str> = index.columns_in_use().map(|c| c.key.as_str()).collect();
    keys.sort();
    assert_eq!(keys, vec!["latencyMs", "level", "module", "msg", "note", "region", "request_id", "retry", "service"]);
    assert_eq!(index.column("region").unwrap().label, "region");
}

#[test]
fn json_values_are_the_text_of_the_line() {
    let index = FilterIndex::build(&rows());
    let note = index.column("note").unwrap();
    assert_eq!(note.values[0].display, r#"say \"hi\""#);
    let latency = index.column("latencyMs").unwrap();
    let shown: Vec<&str> = latency.values.iter().map(|v| v.display.as_str()).collect();
    assert_eq!(shown, vec!["12", "250"]);
    assert_eq!(index.column("retry").unwrap().values[0].display, "true");
}

#[test]
fn a_null_value_is_no_value() {
    let index = FilterIndex::build(&rows());
    assert!(index.columns_in_use().all(|c| c.key != "gone"));
}

#[test]
fn counts_fold_case_and_skip_continuation_lines() {
    let index = FilterIndex::build(&rows());
    let region = index.column("region").unwrap();
    assert_eq!(region.values.len(), 2);
    let eu = region.values.iter().find(|v| v.folded == "eu").unwrap();
    assert_eq!(eu.count, 2);
    let us = region.values.iter().find(|v| v.folded == "us").unwrap();
    assert_eq!(us.count, 1);
}

#[test]
fn filter_on_a_json_field_keeps_its_continuation_lines() {
    let mut entries = rows();
    let mut f = FilterState::new();
    f.toggle_value("region", "eu");
    assert_eq!(visible(&f, &mut entries), vec![1, 2, 3, 4]);
}

#[test]
fn custom_value_on_a_json_field() {
    let mut entries = rows();
    let mut f = FilterState::new();
    f.add_custom("service", "AP");
    assert_eq!(visible(&f, &mut entries), vec![0, 4, 5]);
}

#[test]
fn highlight_rule_on_a_json_field_paints_continuation_lines() {
    let entries = rows();
    let color = HighlightColor::palette(Hue::Red, Shade::Medium);
    let rule = HighlightRule::new(1, "region", &["EU".into()], &[], color).unwrap();
    let painted: Vec<bool> =
        (0..entries.len()).map(|i| paint_for_row(&[rule.clone()], &entries, i).is_some()).collect();
    assert_eq!(painted, vec![false, true, true, true, true, false]);
}
