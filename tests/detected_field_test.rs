//! Detected Fields: `key=value` pairs found in the message.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::column::PinnableColumn;
use log_gpui::extract::scan_kv;
use log_gpui::filter_index::FilterIndex;
use log_gpui::focus::FocusPaneNotice;
use log_gpui::model::FieldKey;
use log_gpui::parser::text::parse_text_bytes;

fn pairs(text: &str) -> Vec<(&str, &str)> {
    scan_kv(text).into_iter().map(|(k, v)| (&text[k], &text[v])).collect()
}

// ─── Finding pairs ───────────────────────────────────────────────────────────

#[test]
fn finds_logfmt_pairs_with_quoted_values_as_written() {
    assert_eq!(
        pairs(r#"charge ok user=alice amount=12.50 note="first order""#),
        vec![("user", "alice"), ("amount", "12.50"), ("note", r#""first order""#)]
    );
}

#[test]
fn a_comparison_is_not_a_pair() {
    assert!(pairs("a == b").is_empty());
    assert!(pairs("x==y").is_empty());
    assert!(pairs("a= b").is_empty(), "no value");
    assert!(pairs("1+x=2").is_empty(), "a key starts after a space or separator");
}

#[test]
fn the_first_of_a_repeated_key_is_used() {
    assert_eq!(pairs("retry id=1 id=2"), vec![("id", "1")]);
}

#[test]
fn an_escaped_quote_stays_in_the_string() {
    assert_eq!(pairs(r#"msg="say \"hi\" now" next=1"#), vec![("msg", r#""say \"hi\" now""#), ("next", "1")]);
}

#[test]
fn keys_may_hold_dots_and_dashes() {
    assert_eq!(pairs("http.status=200 req-id=7f,x=1"), vec![("http.status", "200"), ("req-id", "7f,x=1")]);
}

// ─── Counting ────────────────────────────────────────────────────────────────

fn rows(text: &str) -> Vec<log_gpui::model::LogEntry> {
    parse_text_bytes(text.as_bytes()).unwrap()
}

#[test]
fn counts_lines_per_key_without_reading_the_field() {
    let es = rows("2026-10-20 10:00:01 INFO a user=alice\n  at x user=nobody\n2026-10-20 10:00:02 INFO b user=bob tenant=acme\n");
    let index = FilterIndex::build(&es);
    assert_eq!(index.kv_counts().get(&FieldKey::new("user")), Some(&2), "continuation lines do not count");
    assert_eq!(index.kv_counts().get(&FieldKey::new("tenant")), Some(&1));
    assert!(index.column("user").is_none(), "not a Filter column until turned on");
}

#[test]
fn counts_follow_lines_coming_and_leaving() {
    let es = rows("2026-10-20 10:00:01 INFO a user=alice\n2026-10-20 10:00:02 INFO b tenant=acme\n");
    let mut index = FilterIndex::build(&es[..1]);
    index.append(&es[1..]);
    assert_eq!(index.kv_counts().get(&FieldKey::new("tenant")), Some(&1), "a new key comes with its line");
    index.truncate(&es[1..], &es[..1]);
    assert!(!index.kv_counts().contains_key(&FieldKey::new("user")), "a key without lines goes");
}

// ─── In the app ──────────────────────────────────────────────────────────────

fn lines() -> Vec<String> {
    (0..20)
        .map(|i| {
            let user = ["alice", "bob"][i % 2];
            format!(r#"2026-10-20 10:00:{i:02} INFO [api] charge ok user={user} amount={i}.50 note="order {i}""#)
        })
        .collect()
}

fn show(h: &mut Harness, key: &str, on: bool) {
    h.view.update(&mut h.cx, |a, cx| a.set_column_shown(key, on, cx));
    h.cx.run_until_parked();
}

fn user_col() -> PinnableColumn {
    PinnableColumn::Field(FieldKey::new("user"))
}

#[gpui::test]
fn turning_a_detected_field_on_makes_a_column_that_filters(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    let detected = h.view.read_with(&h.cx, |a, _| a.column_groups("").detected);
    let user = detected.iter().find(|r| r.key == "user").expect("user is detected");
    assert!(!user.shown);
    assert_eq!(user.lines, 20);
    assert!(h.view.read_with(&h.cx, |a, _| a.filter_index.column("user").is_none()), "off: not filterable");

    show(&mut h, "user", true);
    assert!(h.view.read_with(&h.cx, |a, _| a.pane_columns().unwrap().columns().contains(&user_col())));
    let note = h.view.read_with(&h.cx, |a, _| a.all_entries[0].span_text("note").map(str::to_string));
    assert_eq!(note, None, "only fields turned on are read");
    h.view.update(&mut h.cx, |a, cx| a.filter_toggle_value("user", "alice", cx));
    h.cx.run_until_parked();
    assert_eq!(h.view.read_with(&h.cx, |a, _| a.filter_pass.len()), 10);

    show(&mut h, "user", false);
    assert!(!h.view.read_with(&h.cx, |a, _| a.pane_columns().unwrap().columns().contains(&user_col())));
    assert!(h.view.read_with(&h.cx, |a, _| a.filter_state.column("user").is_some_and(|c| c.is_active())), "the condition stays");
    let notice = h.view.read_with(&h.cx, |a, _| a.focus_pane_notice()).unwrap();
    assert!(matches!(notice, FocusPaneNotice::UnreadColumn { can_turn_on: true, .. }), "{notice:?}");
    assert!(notice.title().contains("column turned off"));
    h.view.update(&mut h.cx, |a, cx| a.turn_column_back_on("user", cx));
    h.cx.run_until_parked();
    assert_eq!(h.view.read_with(&h.cx, |a, _| a.filter_pass.len()), 10);
}

#[gpui::test]
fn a_quoted_value_shows_with_its_quotes(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    show(&mut h, "note", true);
    let note = h.view.read_with(&h.cx, |a, _| a.all_entries[3].span_text("note").map(str::to_string));
    assert_eq!(note.as_deref(), Some(r#""order 3""#));
}
