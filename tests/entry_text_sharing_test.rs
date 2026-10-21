//! A line's text is stored once: the payload, time, tag and level text of a
//! parsed entry are stretches of its raw line, not copies of them.
use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::parser::text::{LineParser, parse_text_bytes};

fn inside(whole: &str, part: &str) -> bool {
    let (w, p) = (whole.as_ptr() as usize, part.as_ptr() as usize);
    p >= w && p + part.len() <= w + whole.len()
}

fn assert_shared(e: &LogEntry) {
    for (name, part) in [("payload", e.payload()), ("time", e.time_str()), ("tag", e.tag()), ("level", e.level_text())] {
        if !part.is_empty() {
            assert!(inside(&e.raw_line, part), "{name} {part:?} is a copy, not a range of the raw line");
        }
    }
}

#[test]
fn a_parsed_lines_texts_are_ranges_of_its_raw_line() {
    let text = "2026-09-05 12:34:56.789 [INFO] [auth] login ok\n\
                [2026-09-05T12:34:57.000Z] WARN [api] slow\n\
                {\"time\":\"2026-09-05T12:34:58Z\",\"level\":\"error\",\"module\":\"db\",\"msg\":\"boom\"}\n\
                09-05 12:34:56.789  1234  5678 E AndroidRuntime: crash\n\
                \tat com.example.Foo\n\
                \n\
                plain words";
    let entries = parse_text_bytes(text.as_bytes()).unwrap();
    for e in &entries {
        assert_shared(e);
    }
    assert_eq!(entries[0].payload(), "login ok");
    assert_eq!(entries[0].tag(), "auth");
    assert_eq!(entries[0].level_text(), "INFO");
    assert_eq!(entries[0].time_str(), "2026-09-05 12:34:56.789");
    assert_eq!(entries[2].payload(), "boom");
    assert_eq!(entries[2].tag(), "db");
    assert_eq!(entries[4].payload(), "\tat com.example.Foo", "a continuation line is all message");
    assert_eq!(entries[4].time_str(), "");
    assert_eq!(entries[5].payload(), "");
}

#[test]
fn a_line_parser_entry_shares_too() {
    let mut parser = LineParser::new();
    let e = parser.parse(1, "2026-09-05 12:34:56.789 [INFO] [auth] login ok");
    assert_shared(&e);
}

#[test]
fn a_hand_built_entry_keeps_the_text_it_was_given() {
    let mut e = LogEntry::new(1, 5.0, "ts-1".into(), "api".into(), LogLevel::Info, "hello".into(), "unrelated raw".into());
    assert_eq!((e.time_str(), e.tag(), e.payload()), ("ts-1", "api", "hello"));
    assert_eq!(e.level_text(), "INFO");
    e.set_tag("DB");
    e.set_time_str("t2");
    assert_eq!((e.time_str(), e.tag(), e.payload()), ("t2", "DB", "hello"));
    let c = e.clone();
    assert_eq!(c.tag(), "DB");
}
