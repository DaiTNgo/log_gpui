use log_gpui::model::LogLevel;
use log_gpui::parser::text::parse_text_bytes;

#[test]
fn test_parse_text_logs() {
    let log_content = b"2026-09-05 12:00:01.123 [WARN] [NAVI] GPS satellite count low\n2026-09-05 12:00:02.456 [ERROR] [BODY] Power window pinch detect\n";
    let entries = parse_text_bytes(log_content).expect("Parsing failed");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].index, 1);
    assert_eq!(entries[0].level, LogLevel::Warn);
    assert_eq!(entries[0].tag(), "NAVI");
    assert_eq!(entries[0].payload(), "GPS satellite count low");

    assert_eq!(entries[1].index, 2);
    assert_eq!(entries[1].level, LogLevel::Error);
    assert_eq!(entries[1].tag(), "BODY");
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

#[test]
fn test_logback_lines_with_a_comma_fraction_have_time_and_level() {
    let log = b"2026-10-20 10:00:08,123 ERROR [main] c.a.OrderService - failed to place order\njava.lang.IllegalStateException: boom\n\tat com.acme.Pay.charge(Pay.java:42)\n2026-10-20 10:00:09,500 INFO  [main] c.a.OrderService - retried\n";
    let entries = parse_text_bytes(log).unwrap();

    assert!(entries[0].has_time);
    assert_eq!(entries[0].level, LogLevel::Error);
    assert_eq!(entries[0].tag(), "main");
    assert_eq!(entries[0].payload(), "c.a.OrderService - failed to place order");
    assert!((entries[3].timestamp - entries[0].timestamp - 1.377).abs() < 1e-6);
    assert!(!entries[1].has_time, "the exception line belongs to the line above");
    assert!(!entries[2].has_time);
}

#[test]
fn test_python_logging_default_lines_have_time_level_and_logger() {
    let log = b"2026-10-20 10:00:08,123 - payments.api - WARNING - card declined\n2026-10-20 10:00:09,004 - payments.api - CRITICAL - gave up\n";
    let entries = parse_text_bytes(log).unwrap();

    assert!(entries[0].has_time);
    assert_eq!(entries[0].level, LogLevel::Warn);
    assert_eq!(entries[0].tag(), "payments.api");
    assert_eq!(entries[0].payload(), "card declined");
    assert_eq!(entries[1].level, LogLevel::Fatal);
    assert!(entries[1].timestamp > entries[0].timestamp);
}

#[test]
fn test_comma_fraction_lines_without_a_fraction_still_read() {
    let entries = parse_text_bytes(b"2026-10-20 10:00:08 ERROR [main] boom\n2026-10-20 10:00:08 - app - INFO - up\n").unwrap();
    assert!(entries[0].has_time);
    assert_eq!(entries[0].level, LogLevel::Error);
    assert!(entries[1].has_time);
    assert_eq!(entries[1].level, LogLevel::Info);
    assert_eq!(entries[1].tag(), "app");
}

#[test]
fn test_access_log_lines_have_their_bracketed_time() {
    let common = r#"10.0.0.1 - - [20/Oct/2026:10:00:01 +0000] "GET /health HTTP/1.1" 200 12"#;
    let combined = r#"10.0.0.2 - bob [20/Oct/2026:10:00:03 +0200] "POST /pay HTTP/1.1" 500 31 "-" "curl/8""#;
    let log = format!("{common}\n{combined}\n");
    let entries = parse_text_bytes(log.as_bytes()).unwrap();

    assert!(entries[0].has_time);
    assert!(entries[1].has_time);
    assert!((entries[1].timestamp - entries[0].timestamp - 2.0).abs() < 1e-6, "zones are not converted");
    assert_eq!(entries[0].time_str(), "20/Oct/2026:10:00:01 +0000");
    assert_eq!(entries[0].payload(), common, "the rest of the line stays in MESSAGE");
    assert_eq!(entries[1].payload(), combined);
}

#[test]
fn test_json_ts_and_epoch_times_are_read() {
    let log = br#"{"level":"info","ts":1697712000.123,"msg":"zap"}
{"level":30,"time":1697712001123,"msg":"pino"}
{"name":"app","time":"2023-10-19T10:40:02.000Z","msg":"bunyan"}
{"ts":"1697712003","msg":"a number in a string is not an epoch"}
{"time":0.5,"msg":"a duration is not a time"}
"#;
    let entries = parse_text_bytes(log).unwrap();
    assert!(entries[0].has_time);
    assert!((entries[0].timestamp - 1697712000.123).abs() < 1e-3);
    assert_eq!(entries[0].time_str(), "1697712000.123", "the number as written");
    assert!(entries[1].has_time);
    assert!((entries[1].timestamp - 1697712001.123).abs() < 1e-3, "milliseconds by magnitude");
    assert_eq!(entries[1].time_str(), "1697712001123");
    assert!(entries[2].has_time);
    assert!((entries[2].timestamp - 1697712002.0).abs() < 1e-3);
    assert!(!entries[3].has_time);
    assert!(!entries[4].has_time);
}
