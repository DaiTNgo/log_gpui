//! Log Formats: templates of Format Tokens, compiled to a regex that reads a
//! whole line.

use log_gpui::log_format::{
    BUILT_IN, FormatChoice, FormatChoices, LogFormat, LogFormatStore, TOKENS, compile_template, glob_matches, resolve,
};
use log_gpui::model::LogLevel;
use log_gpui::parser::text::{parse_events_with, read_timestamp, reread_entries};
use log_gpui::model::LogEntry;

fn read(template: &str, text: &str) -> Vec<LogEntry> {
    let f = compile_template(template).unwrap();
    parse_events_with(text, Some(&f)).iter().map(LogEntry::from_event).collect()
}

// ─── Templates ───────────────────────────────────────────────────────────────

#[test]
fn a_simple_template_reads_fields_and_fixed_columns() {
    let es = read(
        "%{TIMESTAMP:timestamp} [%{LEVEL:level}] %{IP:client} %{GREEDY:message}",
        "2026-10-20 10:00:01 [INFO] 10.0.4.17 GET /health",
    );
    assert_eq!(es[0].span_text("client"), Some("10.0.4.17"));
    assert_eq!(es[0].span_text("message"), Some("GET /health"));
    assert_eq!(es[0].level, LogLevel::Info);
    assert!(es[0].has_time);
    let fields: Vec<&str> = es[0].fields().map(|(k, _)| k.as_str()).collect();
    assert_eq!(fields, vec!["client"]);
}

#[test]
fn a_named_group_is_a_field() {
    let es = read("%{WORD:op} took (?<took>\\d+ms)", "charge took 34ms");
    assert_eq!(es[0].span_text("took"), Some("34ms"));
    let f = compile_template("%{WORD:op} took (?<took>\\d+ms)").unwrap();
    assert_eq!(f.fields, vec!["op", "took"]);
}

#[test]
fn whitespace_matches_any_run_and_other_characters_match_themselves() {
    let es = read("%{WORD:a} | %{WORD:b}.", "x   |\tyy.");
    assert_eq!((es[0].span_text("a"), es[0].span_text("b")), (Some("x"), Some("yy")));
    assert!(read("%{WORD:a} | %{WORD:b}.", "x | yyz")[0].span_text("a").is_none(), ". is not any character");
}

#[test]
fn a_template_reads_the_whole_line_after_leading_whitespace() {
    let es = read("%{INT:n} %{WORD:w}", "   12 ab");
    assert_eq!(es[0].span_text("n"), Some("12"));
    assert!(read("%{INT:n}", "12 ab")[0].span_text("n").is_none(), "anchored at the end");
}

#[test]
fn a_token_without_a_field_matches_without_reading() {
    let es = read("%{IP} %{WORD:w}", "1.2.3.4 hi");
    assert_eq!(es[0].span_text("w"), Some("hi"));
    assert_eq!(compile_template("%{IP} %{WORD:w}").unwrap().fields, vec!["w"]);
}

#[test]
fn each_format_token_matches_its_shape() {
    let cases = [
        ("TIMESTAMP", "2026-10-20T10:00:01.5Z", "10:00"),
        ("LEVEL", "WARNING", "loud"),
        ("WORD", "abc_1", "a-b"),
        ("NOTSPACE", "a-b/c", "a b"),
        ("INT", "-42", "4.2"),
        ("NUMBER", "4.2e3", "x"),
        ("IP", "10.0.0.1", "10.0.0"),
        ("UUID", "123e4567-e89b-12d3-a456-426614174000", "123e4567"),
        ("HEX", "0xdeadBEEF", "0xZZ"),
        ("QUOTED", r#""say \"hi\"""#, r#""open"#),
        ("DATA", "anything at all", ""),
        ("GREEDY", "anything at all", ""),
    ];
    assert_eq!(cases.len(), TOKENS.len());
    for (token, good, bad) in cases {
        let f = compile_template(&format!("%{{{token}:v}}")).unwrap();
        let line = good;
        assert_eq!(f.read(line).and_then(|s| s.get("v").map(|r| line[r.clone()].to_string())).as_deref(), Some(good), "{token}");
        if !bad.is_empty() {
            assert!(f.read(bad).is_none(), "{token} should not match {bad:?}");
        }
    }
}

#[test]
fn timestamp_knows_every_layout_read_timestamp_does() {
    for t in ["2026-10-20 10:00:01", "2026-10-20T10:00:01.123+02:00", "2026-10-20 10:00:01,123", "20/Oct/2026:10:00:01 +0000", "10-20 10:00:01.123", "Oct 20 10:00:01"] {
        let f = compile_template("%{TIMESTAMP:timestamp}").unwrap();
        assert!(f.read(t).is_some(), "{t}");
        assert!(read_timestamp(t).is_some(), "{t}");
    }
}

#[test]
fn errors_say_where_the_template_is_wrong() {
    let e = compile_template("%{TIMESTAMP:timestamp} %{FOO:x}").unwrap_err();
    assert!(e.message.starts_with("FOO is not a Format Token"));
    assert_eq!(&"%{TIMESTAMP:timestamp} %{FOO:x}"[e.span.clone()], "FOO");

    let t = "%{WORD:a} %{IP";
    let e = compile_template(t).unwrap_err();
    assert!(e.message.contains("not closed"));
    assert_eq!(&t[e.span.clone()], "%{IP");

    let e = compile_template("%{WORD:a} %{WORD:a}").unwrap_err();
    assert!(e.message.contains("twice"));
    let t = "x (?<n>\\d+";
    assert_eq!(&t[compile_template(t).unwrap_err().span], "(?<n>\\d+");
    assert!(compile_template("(?<n>[a-)").is_err());
}

// ─── Fixed columns, Auto-detect fallback ─────────────────────────────────────

#[test]
fn a_line_the_format_does_not_match_is_read_by_auto_detect() {
    let text = "2026-10-20 10:00:01 [ERROR] 10.0.4.17 boom\n    at PaymentService.charge(Pay.java:1)\nnot this format";
    let es = read("%{TIMESTAMP:timestamp} [%{LEVEL:level}] %{IP:client} %{GREEDY:message}", text);
    assert!(es[1].is_continuation(), "a stack trace line still continues its Parent Line");
    assert_eq!(es[1].parent, Some(1));
    assert_eq!(es[1].payload.as_str(), es[1].raw_line.as_str());
    assert!(es[1].span_text("client").is_none());
    assert_eq!(es[0].level, LogLevel::Error);
}

#[test]
fn a_one_letter_level_is_a_level() {
    let es = read("%{INT:n} %{LEVEL:level} %{GREEDY:message}", "7 E boom");
    assert_eq!(es[0].level, LogLevel::Error);
}

#[test]
fn reading_again_keeps_numbers_and_generations() {
    let mut es = read("%{GREEDY:message}", "a=1\nb=2");
    for (i, e) in es.iter_mut().enumerate() {
        e.index = 40 + i;
        e.generation = 3;
    }
    reread_entries(&mut es, Some(&compile_template("%{WORD:k}=%{INT:v}").unwrap()));
    assert_eq!((es[0].index, es[0].generation), (40, 3));
    assert_eq!(es[1].span_text("v"), Some("2"));
    reread_entries(&mut es, None);
    assert!(es[1].span_text("v").is_none());
}

// ─── Built-in templates ──────────────────────────────────────────────────────

#[test]
fn every_built_in_template_reads_its_fixture_whole() {
    let fixtures = ["nginx_access.log", "python_default.log", "python_asctime.log", "log4j_ttcc.log"];
    for ((name, template, _), fixture) in BUILT_IN.iter().zip(fixtures) {
        let f = compile_template(template).unwrap_or_else(|e| panic!("{name}: {}", e.message));
        let text = std::fs::read_to_string(format!("tests/fixtures/formats/{fixture}")).unwrap();
        for line in text.lines() {
            assert!(f.read(line).is_some(), "{name} does not read {line:?}");
        }
    }
    let es = read(BUILT_IN[0].1, &std::fs::read_to_string("tests/fixtures/formats/nginx_access.log").unwrap());
    assert_eq!(es[1].span_text("client"), Some("203.0.113.9"));
    assert_eq!(es[1].span_text("status"), Some("201"));
    assert!(es.iter().all(|e| e.has_time), "the time of nginx sorts");
    let py = read(BUILT_IN[1].1, "ERROR:billing.gateway:timeout: retry 3");
    assert_eq!(py[0].tag.as_str(), "billing.gateway");
    assert_eq!(py[0].payload.as_str(), "timeout: retry 3");
}

// ─── Which format a file uses ────────────────────────────────────────────────

fn fmt(id: &str, globs: &[&str]) -> LogFormat {
    LogFormat { id: id.into(), name: id.into(), template: "%{GREEDY:message}".into(), globs: globs.iter().map(|g| g.to_string()).collect() }
}

#[test]
fn globs_compare_the_file_name_only() {
    assert!(glob_matches("access*.log", "access-2026-10-20.log"));
    assert!(glob_matches("app?.log", "app1.log"));
    assert!(!glob_matches("access*.log", "error.log"));
    let lib = [fmt("nginx", &["access*.log"])];
    let none = FormatChoices::default();
    assert_eq!(resolve(&lib, &none, "s", "f", "/var/log/access-1.log").map(|f| f.id.as_str()), Some("nginx"));
    assert!(resolve(&lib, &none, "s", "f", "/access/app.log").is_none(), "not the path");
}

#[test]
fn file_then_space_then_glob_then_auto_detect() {
    let lib = [fmt("app", &[]), fmt("nginx", &["access*.log"])];
    let mut c = FormatChoices::default();
    c.spaces.insert("s".into(), FormatChoice::Format("app".into()));
    assert_eq!(resolve(&lib, &c, "s", "f1", "access.log").unwrap().id, "app", "a choice wins over a glob");
    c.files.insert("f1".into(), FormatChoice::Format("nginx".into()));
    assert_eq!(resolve(&lib, &c, "s", "f1", "x.log").unwrap().id, "nginx");
    assert_eq!(resolve(&lib, &c, "s", "f2", "x.log").unwrap().id, "app");
    c.spaces.insert("s".into(), FormatChoice::AutoDetect);
    assert!(resolve(&lib, &c, "s", "f2", "access.log").is_none(), "Auto-detect chosen: no glob");
    c.forget("nginx");
    assert!(c.files.is_empty());
}

#[test]
fn the_library_adds_duplicates_and_saves() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log_formats.json");
    let mut store = LogFormatStore::default();
    let id = store.add("billing", "%{GREEDY:message}", vec!["billing*.log".into()]);
    let copy = store.duplicate(&id).unwrap();
    assert_eq!(store.get(&copy).unwrap().name, "billing copy");
    store.save_to(&path).unwrap();
    let back = LogFormatStore::load(&path);
    assert_eq!(back, store);
    store.remove(&id);
    assert!(store.get(&id).is_none());
}
