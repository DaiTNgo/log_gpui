use log_gpui::filter::{FilterState, parse_time_bound};
use log_gpui::filter_index::FilterIndex;
use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::parser::text::parse_text_bytes;
use log_gpui::search_expr::{Matcher, parse};
use log_gpui::space_view::ParsedFile;
use log_gpui::timeline;

fn entries(text: &str) -> Vec<LogEntry> {
    parse_text_bytes(text.as_bytes()).unwrap()
}

fn cell<'a>(e: &'a LogEntry, field: &str) -> &'a str {
    e.span_text(field).unwrap_or("")
}

// ─── Text as stored ──────────────────────────────────────────────────────────

#[test]
fn abbreviated_level_is_shown_as_written() {
    let e = &entries("2026-10-18 10:00:01 err payment failed")[0];
    assert_eq!(e.level_text(), "err");
    assert_eq!(cell(e, "level"), "err");
    assert_eq!(e.level, LogLevel::Error, "meaning is still understood");
    assert_eq!(e.payload(), "payment failed");
}

#[test]
fn timestamp_is_shown_verbatim() {
    let e = &entries("2026-10-18T10:32:02.441+07:00 [INFO] [api] hi")[0];
    assert_eq!(e.time_str(), "2026-10-18T10:32:02.441+07:00");
    assert_eq!(cell(e, "timestamp"), "2026-10-18T10:32:02.441+07:00");
    assert!(e.has_time);
}

#[test]
fn bracketed_fields_keep_their_spans() {
    let line = "  [2026-10-18 10:00:01.5] [WARN] [auth] token near expiry";
    let e = &entries(line)[0];
    assert_eq!(cell(e, "timestamp"), "2026-10-18 10:00:01.5");
    assert_eq!(cell(e, "level"), "WARN");
    assert_eq!(cell(e, "module"), "auth");
    assert_eq!(cell(e, "message"), "token near expiry");
}

#[test]
fn json_strings_keep_their_escapes() {
    let e = &entries(r#"{"time":"2026-10-18T10:00:00Z","level":"error","msg":"say \"hi\"","n":42,"ok":true}"#)[0];
    assert_eq!(cell(e, "message"), r#"say \"hi\""#);
    assert_eq!(cell(e, "level"), "error");
    assert_eq!(cell(e, "n"), "42");
    assert_eq!(cell(e, "ok"), "true");
    let n = e.fields().find(|(k, _)| k.as_str() == "n").unwrap();
    assert_eq!(n.1, "42");
}

#[test]
fn nothing_is_made_up_for_unreadable_lines() {
    let e = &entries("connection reset by peer")[0];
    assert_eq!(e.tag(), "", "no default module");
    assert_eq!(e.level_text(), "");
    assert_eq!(e.level, LogLevel::Unknown, "no level guessed from the words");
    assert_eq!(cell(e, "message"), "connection reset by peer");
}

#[test]
fn a_word_that_is_not_a_level_stays_in_the_message() {
    let e = &entries("2026-10-18 10:00:01 Failed to connect")[0];
    assert_eq!(e.level_text(), "");
    assert_eq!(cell(e, "message"), "Failed to connect");
}

#[test]
fn module_without_level() {
    let e = &entries("2026-10-18 10:00:01 [api] started")[0];
    assert_eq!(e.level_text(), "");
    assert_eq!(e.tag(), "api");
}

#[test]
fn logcat_level_letter_is_kept() {
    let e = &entries("09-05 12:34:56.789  1234  5678 E AndroidRuntime: boom")[0];
    assert_eq!(e.level_text(), "E");
    assert_eq!(e.level, LogLevel::Error);
    assert_eq!(e.tag(), "AndroidRuntime");
}

#[test]
fn raw_and_parsed_agree_on_an_edited_line() {
    let parsed = ParsedFile::from_text("2026-10-18 10:00:01 err boom\n");
    assert_eq!(parsed.entries[0].level_text(), "err");
}

// ─── Rows ────────────────────────────────────────────────────────────────────

#[test]
fn every_stored_line_is_a_row_including_blank_ones() {
    let es = entries("2026-10-18 10:00:01 [ERROR] boom\n    at A.b(A.java:1)\n\nlast\n");
    assert_eq!(es.len(), 4);
    assert_eq!(es.iter().map(|e| e.index).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
    assert_eq!(es[2].raw_line, "");
}

#[test]
fn continuation_lines_point_at_their_parent_line() {
    let es = entries(
        "banner\nsecond\n2026-10-18 10:00:01 [ERROR] [api] boom\n    at A.b(A.java:1)\nCaused by: X\n2026-10-18 10:00:02 [INFO] [api] ok\n",
    );
    assert_eq!(es[0].parent, None);
    assert_eq!(es[1].parent, None);
    assert_eq!(es[2].parent, None);
    assert_eq!(es[3].parent, Some(3));
    assert_eq!(es[4].parent, Some(3));
    assert_eq!(es[5].parent, None);
    // A continuation line only has its message, covering the whole line.
    assert_eq!(es[3].level_text(), "");
    assert_eq!(es[3].tag(), "");
    assert_eq!(cell(&es[3], "message"), "    at A.b(A.java:1)");
    assert_eq!(es[3].spans.len(), 1);
}

#[test]
fn a_blank_line_after_a_timestamped_line_continues_it() {
    let es = entries("2026-10-18 10:00:01 [INFO] [api] a\n\n");
    assert_eq!(es[1].parent, Some(1));
}

#[test]
fn a_file_without_timestamps_has_no_continuation_lines() {
    let es = entries("one\ntwo\nthree\n");
    assert!(es.iter().all(|e| e.parent.is_none()));
}

// ─── Merge keeps continuation lines under their parent ──────────────────────

#[test]
fn merge_keeps_a_stack_trace_under_its_parent_line() {
    let a = entries("2026-10-18 10:00:01 [ERROR] [a] boom\n  at x\n  at y\n2026-10-18 10:00:09 [INFO] [a] later\n");
    let b = entries("2026-10-18 10:00:01 [INFO] [b] same time\n2026-10-18 10:00:02 [INFO] [b] next\n");
    // File b first, so at the same instant its row sorts before file a's.
    let merged = timeline::merge(&[b.as_slice(), a.as_slice()]);
    let order: Vec<(u32, u32)> = merged.rows.iter().map(|r| (r.file, r.line)).collect();
    let boom = order.iter().position(|&r| r == (1, 0)).unwrap();
    assert_eq!(order[boom + 1], (1, 1));
    assert_eq!(order[boom + 2], (1, 2));
}

// ─── Filter, counts and Search ───────────────────────────────────────────────

fn error_with_trace() -> Vec<LogEntry> {
    let mut lines = vec!["2026-10-18 10:00:01 [INFO] [api] start".to_string()];
    lines.push("2026-10-18 10:00:02 [ERROR] [api] boom".to_string());
    for i in 0..20 {
        lines.push(format!("    at frame{i}"));
    }
    lines.push("2026-10-18 10:00:03 [INFO] [api] done".to_string());
    entries(&lines.join("\n"))
}

#[test]
fn filtering_error_keeps_the_stack_trace() {
    let mut es = error_with_trace();
    let index = FilterIndex::build(&es);
    let mut f = FilterState::new();
    f.toggle_value("level", "ERROR");
    let visible = f.evaluate(&index, &mut es);
    assert_eq!(visible, (1..=21).collect::<Vec<_>>());
}

#[test]
fn hiding_a_level_hides_its_continuation_lines_too() {
    let mut es = error_with_trace();
    let index = FilterIndex::build(&es);
    let mut f = FilterState::new();
    f.toggle_value("level", "ERROR");
    f.set_hide("level", true);
    let visible = f.evaluate(&index, &mut es);
    assert_eq!(visible, vec![0, 22]);
}

#[test]
fn filter_counts_ignore_continuation_lines() {
    let es = error_with_trace();
    let index = FilterIndex::build(&es);
    let level = index.column("level").unwrap();
    let error = level.values.iter().find(|v| v.display == "ERROR").unwrap();
    assert_eq!(error.count, 1);
}

#[test]
fn level_values_are_verbatim_and_only_case_merged() {
    let es = entries(
        "2026-10-18 10:00:01 E a\n2026-10-18 10:00:02 ERROR b\n2026-10-18 10:00:03 error c\n",
    );
    let index = FilterIndex::build(&es);
    let level = index.column("level").unwrap();
    assert_eq!(level.values.len(), 2);
    let error = level.values.iter().find(|v| v.folded == "error").unwrap();
    assert_eq!(error.count, 2);
    assert!(level.values.iter().any(|v| v.display == "E" && v.count == 1));
}

#[test]
fn search_matches_the_text_of_the_continuation_line_itself() {
    let es = entries("2026-10-18 10:00:01 [ERROR] [api] boom\n  at a\nCaused by: NullPointerException\n");
    let m = Matcher::compile(&parse("NullPointerException").unwrap().unwrap());
    let hits: Vec<usize> = es
        .iter()
        .enumerate()
        .filter(|(_, e)| m.is_match(e.payload()))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(hits, vec![2]);
    let underscore = Matcher::compile(&parse("_").unwrap().unwrap());
    assert!(!underscore.is_match(es[1].payload()), "the dim `_` is not text");
}

// ─── Time Range ──────────────────────────────────────────────────────────────

#[test]
fn time_range_reads_every_format_the_parser_reads() {
    let iso = parse_time_bound("2026-10-18T10:00:00+07:00", 0.0).unwrap().unwrap();
    let plain = parse_time_bound("2026-10-18 10:00:00", 0.0).unwrap().unwrap();
    assert_eq!(iso, plain, "the file's own clock: no zone conversion");
    assert!(parse_time_bound("2026/10/18 10:00:00", 0.0).is_ok());
    assert!(parse_time_bound("09-05 12:34:56.789", 0.0).is_ok());
    assert!(parse_time_bound("Sep  5 12:34:56", 0.0).is_ok());
    let mut es = entries("2026-10-18T09:00:00+07:00 [INFO] a\n2026-10-18T11:00:00+07:00 [INFO] b\n");
    let index = FilterIndex::build(&es);
    let mut f = FilterState::new();
    f.set_time_range("2026-10-18 10:00:00", "");
    assert_eq!(f.evaluate(&index, &mut es), vec![1]);
}
