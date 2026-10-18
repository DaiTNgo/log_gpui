use log_gpui::model::LogLevel;
use log_gpui::parser::text::parse_text_bytes;
use log_gpui::ui::inspector::format_hex_dump;

#[test]
fn test_parse_text_logs() {
    let log_content = b"2026-09-05 12:00:01.123 [WARN] [NAVI] GPS satellite count low\n2026-09-05 12:00:02.456 [ERROR] [BODY] Power window pinch detect\n";
    let entries = parse_text_bytes(log_content).expect("Parsing failed");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].index, 1);
    assert_eq!(entries[0].level, LogLevel::Warn);
    assert_eq!(entries[0].tag, "NAVI");
    assert_eq!(entries[0].payload, "GPS satellite count low");

    assert_eq!(entries[1].index, 2);
    assert_eq!(entries[1].level, LogLevel::Error);
    assert_eq!(entries[1].tag, "BODY");
}

#[test]
fn test_hex_dump_formatting() {
    let data = b"DLT\x01Hello World!";
    let dump = format_hex_dump(data);

    assert!(dump.contains("44 4c 54 01"));
    assert!(dump.contains("|DLT.Hello World!|"));
}

#[test]
fn test_lines_without_their_own_time_are_marked_so() {
    let log = b"2026-09-05 12:00:01.123 [ERROR] [api] boom\n    at a.b.C(C.java:1)\nplain note\n";
    let entries = parse_text_bytes(log).unwrap();

    assert!(entries[0].has_time);
    assert!(!entries[1].has_time, "a stack trace line has no time of its own");
    assert!(!entries[2].has_time);
}

#[test]
fn test_logcat_and_syslog_times_are_read_in_a_fixed_year() {
    let logcat = parse_text_bytes(b"09-05 12:34:56.789  1234  5678 E Tag: first\n").unwrap();
    let later = parse_text_bytes(b"09-05 12:34:57.000  1234  5678 E Tag: second\n").unwrap();
    assert!(logcat[0].has_time);
    assert!(later[0].timestamp > logcat[0].timestamp);

    let syslog = parse_text_bytes(b"Sep  5 12:34:56 host app[1]: started\n").unwrap();
    assert!(syslog[0].has_time);
}

#[test]
fn test_json_lines_with_a_time_have_it() {
    let log = br#"{"time":"2026-09-05T10:00:00Z","level":"info","msg":"x"}
{"level":"info","msg":"no time"}
"#;
    let entries = parse_text_bytes(log).unwrap();
    assert!(entries[0].has_time);
    assert!(!entries[1].has_time);
}
