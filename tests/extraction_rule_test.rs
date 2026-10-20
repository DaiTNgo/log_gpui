//! Extraction Rules: reading a Field Column with a regex, and the guesses
//! "Make column…" makes from a selection.

use log_gpui::extract::{
    Candidate, ExtractionRule, LineConfig, NameCheck, PatternError, RuleScope, check_name, compile_pattern,
    cut_around, guess, preview, suggest_name,
};
use log_gpui::model::LogEntry;
use log_gpui::parser::text::{parse_text_bytes, parse_text_bytes_with_config};

fn rule(id: u64, name: &str, pattern: &str) -> ExtractionRule {
    ExtractionRule {
        id,
        name: name.into(),
        pattern: pattern.into(),
        scope: RuleScope::Line,
        description: String::new(),
        enabled: true,
    }
}

fn read(text: &str, rules: &[ExtractionRule]) -> Vec<LogEntry> {
    parse_text_bytes_with_config(text.as_bytes(), &LineConfig::new(rules)).unwrap().entries
}

// ─── LineConfig ──────────────────────────────────────────────────────────────

#[test]
fn an_empty_config_reads_lines_as_before() {
    for fixture in std::fs::read_dir("logs").into_iter().flatten().flatten() {
        let bytes = std::fs::read(fixture.path()).unwrap();
        let plain = parse_text_bytes(&bytes).unwrap();
        let with = parse_text_bytes_with_config(&bytes, &LineConfig::default()).unwrap().entries;
        assert_eq!(plain.len(), with.len());
        for (a, b) in plain.iter().zip(&with) {
            assert_eq!(a.spans, b.spans);
            assert!(b.extra.is_empty());
        }
    }
    let text = "2026-10-18 10:00:01 [INFO] [DB] user=alice ok\n{\"msg\":\"m\",\"user\":\"bob\"}\n  at x\n";
    let plain = parse_text_bytes(text.as_bytes()).unwrap();
    let with = read(text, &[]);
    for (a, b) in plain.iter().zip(&with) {
        assert_eq!((a.spans.clone(), &a.raw_line, a.parent), (b.spans.clone(), &b.raw_line, b.parent));
    }
}

#[test]
fn a_rule_reads_a_span_of_the_line() {
    let es = read("2026-10-18 10:00:01 [INFO] [DB] user=alice ok\n2026-10-18 10:00:02 [INFO] [DB] no user\n", &[rule(1, "user", r"user=(\w+)")]);
    assert_eq!(es[0].span_text("user"), Some("alice"));
    assert_eq!(es[1].span_text("user"), None);
    assert_eq!(es[1].span_text("message"), Some("no user"), "an unmatched line loses nothing");
}

#[test]
fn a_rule_runs_on_the_whole_line() {
    let es = read("2026-10-18 10:00:01 [INFO] [svc-7] hello\n", &[rule(1, "shard", r"\[svc-(\d+)\]")]);
    assert_eq!(es[0].span_text("shard"), Some("7"));
}

#[test]
fn a_json_field_wins_over_a_rule_of_the_same_name() {
    let es = read(r#"{"msg":"user=bob","user":"alice"}"#, &[rule(1, "user", r"user=(\w+)")]);
    assert_eq!(es[0].span_text("user"), Some("alice"));
}

#[test]
fn the_first_rule_of_a_name_wins() {
    let es = read("x u=u-1 v=u-2\n", &[rule(1, "user", r"u=(\S+)"), rule(2, "user", r"v=(\S+)")]);
    assert_eq!(es[0].span_text("user"), Some("u-1"));
    let es = read("x v=u-2\n", &[rule(1, "user", r"u=(\S+)"), rule(2, "user", r"v=(\S+)")]);
    assert_eq!(es[0].span_text("user"), Some("u-2"));
}

#[test]
fn a_rule_turned_off_reads_nothing() {
    let mut r = rule(1, "user", r"user=(\w+)");
    r.enabled = false;
    assert!(read("user=alice\n", &[r])[0].span_text("user").is_none());
}

#[test]
fn continuation_lines_take_no_value_of_their_own() {
    let es = read("2026-10-18 10:00:01 [INFO] user=alice\n  at user=bob\n", &[rule(1, "user", r"user=(\w+)")]);
    assert!(es[1].is_continuation());
    assert!(es[1].extra.is_empty());
}

#[test]
fn a_message_rule_reads_only_the_message() {
    let mut r = rule(1, "w2", r"^\s*(?:\S+\s+){1}(\S+)");
    r.scope = RuleScope::Message;
    let es = read("2026-10-18 10:00:01 [INFO] [DB] charge ok now\n", &[r]);
    assert_eq!(es[0].span_text("w2"), Some("ok"));
}

#[test]
fn a_pattern_needs_exactly_one_capture_group() {
    assert!(compile_pattern(r"user=(\w+)").is_ok());
    assert_eq!(compile_pattern(r"user=\w+").unwrap_err(), PatternError::Groups(0));
    assert_eq!(compile_pattern(r"(a)=(b)").unwrap_err(), PatternError::Groups(2));
    assert!(compile_pattern(r"(?:a)=(b)").is_ok());
    assert!(matches!(compile_pattern(r"(\d+"), Err(PatternError::Invalid(_))));
    assert_eq!(compile_pattern("  ").unwrap_err(), PatternError::Empty);
    assert!(matches!(compile_pattern(r"(\w{1000}){1000}"), Err(PatternError::TooBig)));
    assert!(PatternError::Groups(0).message().contains("wrap the value in ( )"));
}

// ─── Guesses ─────────────────────────────────────────────────────────────────

fn sel(line: &str, value: &str) -> std::ops::Range<usize> {
    let s = line.find(value).unwrap();
    s..s + value.len()
}

fn guesses(line: &str, value: &str, sample: &[&str]) -> Vec<Candidate> {
    let sample: Vec<(&str, Option<&std::ops::Range<usize>>)> = sample.iter().map(|l| (*l, None)).collect();
    guess(line, sel(line, value), None, &sample)
}

#[test]
fn guesses_the_key_before_the_value() {
    let line = "GET /a user=alice took=12ms";
    let g = guesses(line, "alice", &[line, "GET /b user=bob took=3ms", "GET /c"]);
    assert_eq!(g[0].description, "after `user=`");
    assert_eq!(g[0].key.as_deref(), Some("user"));
    assert_eq!(g[0].coverage, 2);
}

#[test]
fn guesses_a_json_key() {
    let line = r#"{"msg":"m","user": "al ice"}"#;
    let g = guesses(line, "al ice", &[line]);
    assert_eq!(g[0].key.as_deref(), Some("user"));
    assert!(g[0].description.contains(r#""user": ""#), "{}", g[0].description);
}

#[test]
fn guesses_a_key_with_a_colon() {
    let line = "conn id: 4242 open";
    let g = guesses(line, "4242", &[line]);
    assert_eq!(g[0].description, "after `id:`");
    let r = compile_pattern(&g[0].pattern).unwrap();
    assert_eq!(r.captures("conn id: 7 open").unwrap().get(1).unwrap().as_str(), "7");
}

#[test]
fn the_value_stops_where_the_selection_does() {
    let line = "req=7f3a,user=alice";
    let g = guesses(line, "7f3a", &[line]);
    let r = compile_pattern(&g[0].pattern).unwrap();
    assert_eq!(r.captures("req=beef,user=bob").unwrap().get(1).unwrap().as_str(), "beef");
}

#[test]
fn guesses_the_nth_bracket_group() {
    let line = "[10:00] [worker-3] [tenant-a] done";
    let g = guesses(line, "tenant-a", &[line, "[10:01] [worker-1] [tenant-b] done"]);
    let b = g.iter().find(|c| c.description == "[ ] group 3").expect("bracket guess");
    assert_eq!(b.coverage, 2);
}

#[test]
fn guesses_the_nth_word_of_the_message_and_of_the_line() {
    let line = "2026-10-18 10:00:01 [INFO] [DB] charge ok";
    let es = parse_text_bytes(line.as_bytes()).unwrap();
    let msg = es[0].spans.get("message").cloned();
    let g = guess(line, sel(line, "ok"), msg.as_ref(), &[(line, msg.as_ref())]);
    let names: Vec<&str> = g.iter().map(|c| c.description.as_str()).collect();
    assert!(names.contains(&"word 2 of the message"), "{names:?}");
    assert!(names.contains(&"word 6 of the line"), "{names:?}");
}

#[test]
fn every_guess_reads_exactly_the_selection() {
    let line = "a b=c [d] e";
    for value in ["c", "d", "e", "a"] {
        for c in guesses(line, value, &[line]) {
            let r = compile_pattern(&c.pattern).unwrap();
            assert_eq!(r.captures(line).unwrap().get(1).unwrap().range(), sel(line, value), "{}", c.description);
        }
    }
}

#[test]
fn ranks_by_coverage() {
    let line = "x [a] user=alice";
    let sample = [line, "y [b] user=bob", "z user=carol", "w user=dave"];
    let g = guesses(line, "alice", &sample);
    assert!(g.windows(2).all(|w| w[0].coverage >= w[1].coverage));
    assert_eq!(g[0].key.as_deref(), Some("user"));
}

// ─── Names ───────────────────────────────────────────────────────────────────

#[test]
fn suggests_the_key_or_a_name_from_the_shape() {
    assert_eq!(suggest_name(Some("user"), "alice"), "user");
    assert_eq!(suggest_name(None, "10.0.0.12"), "ip");
    assert_eq!(suggest_name(None, "123e4567-e89b-12d3-a456-426614174000"), "uuid");
    assert_eq!(suggest_name(None, "12.5ms"), "duration");
    assert_eq!(suggest_name(None, "alice"), "");
}

#[test]
fn checks_the_name() {
    assert_eq!(check_name("", &[]), NameCheck::Empty);
    assert_eq!(check_name("Level", &[]), NameCheck::Reserved("Level".into()));
    assert_eq!(check_name("user", &["user"]), NameCheck::Joins("user".into()));
    assert_eq!(check_name("user", &["user"]).message().unwrap(), "Values will join column user");
    assert_eq!(check_name("tenant", &["user"]), NameCheck::New);
}

// ─── Preview ─────────────────────────────────────────────────────────────────

#[test]
fn preview_puts_the_selected_line_first_and_shows_a_line_not_matched() {
    let text = "user=bob a\nuser=carol b\nno user here\nuser=alice c\nuser=dave d\n";
    let es = parse_text_bytes(text.as_bytes()).unwrap();
    let r = compile_pattern(r"user=(\w+)").unwrap();
    let p = preview(&r, RuleScope::Line, &es, 3, Some("user"));
    assert_eq!(p.matched.len(), 3);
    assert_eq!(p.matched[0].row, 3);
    assert_eq!(&p.matched[0].text[p.matched[0].highlight.clone()], "alice");
    assert_eq!(p.unmatched.as_ref().unwrap().row, 2);
    assert_eq!((p.count, p.total), (4, 5));
    assert_eq!(p.summary(), "4 of 5 lines (80%)");
    assert_eq!(p.top_values.len(), 4);
}

#[test]
fn preview_cuts_long_lines_around_the_value() {
    let line = format!("{} user=alice {}", "x".repeat(200), "y".repeat(200));
    let s = line.find("alice").unwrap();
    let (text, hl) = cut_around(&line, s..s + 5);
    assert!(text.starts_with('…') && text.ends_with('…'));
    assert_eq!(&text[hl], "alice");
    assert!(text.chars().count() < 90);
}

// ─── When errors show ────────────────────────────────────────────────────────

use log_gpui::pattern_field::PatternField;

#[test]
fn an_error_waits_for_the_user_to_stop_typing() {
    let mut f: PatternField<&str> = PatternField::default();
    let first = f.edit(Err("bad"));
    assert!(!f.is_valid());
    assert!(f.visible_error().is_none(), "not while typing");
    let second = f.edit(Err("bad"));
    assert!(!f.settle(first), "an older pause does nothing");
    assert!(f.settle(second));
    assert_eq!(f.visible_error(), Some(&"bad"));
    f.edit(Err("bad"));
    assert!(f.visible_error().is_none(), "typing again hides it");
    f.leave();
    assert_eq!(f.visible_error(), Some(&"bad"), "leaving the field shows it");
    f.edit(Ok(()));
    f.leave();
    assert!(f.is_valid() && f.visible_error().is_none());
}
