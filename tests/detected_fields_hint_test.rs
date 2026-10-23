//! The Parsed View says when Detected Fields are there but none is shown.
mod common;

use gpui::TestAppContext;
use log_gpui::app::columns::DetectedHint;

fn lines() -> Vec<String> {
    (0..30)
        .map(|i| {
            let tail = if i % 3 == 0 { format!(" region=eu{}", i % 2) } else { String::new() };
            format!("2026-10-19 10:00:{:02} INFO request done user={} status=200{tail}", i, i % 4)
        })
        .collect()
}

fn hint(h: &common::Harness) -> Option<DetectedHint> {
    h.view.read_with(&h.cx, |app, _| app.detected_fields_hint())
}

#[gpui::test]
fn the_hint_names_the_fields_with_the_most_lines(cx: &mut TestAppContext) {
    let h = common::open_app_with_lines(cx, &lines());
    let hint = hint(&h).expect("a hint");
    assert_eq!(hint.names[..2], ["status".to_string(), "user".to_string()]);
    assert_eq!(hint.label(), "3 fields detected: status, user, region");
}

#[gpui::test]
fn turning_a_field_on_hides_the_hint(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| app.set_column_shown("user", true, cx));
    h.cx.run_until_parked();
    assert_eq!(hint(&h), None);
}

#[gpui::test]
fn a_dismissed_hint_stays_dismissed_for_the_log_space(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| app.dismiss_detected_fields_hint(cx));
    assert_eq!(hint(&h), None);
    let kept = h.view.read_with(&h.cx, |app, _| app.workspace_state.active_project().unwrap().log_spaces[0].detected_hint_dismissed);
    assert!(kept, "kept with the Log Space, so it survives a restart");
}

#[gpui::test]
fn no_hint_without_detected_fields(cx: &mut TestAppContext) {
    let h = common::open_app_with_lines(cx, &common::sample_lines(10));
    assert_eq!(hint(&h), None);
}

#[test]
fn the_label_counts_the_fields_left_out() {
    let hint = DetectedHint { names: vec!["a".into(), "b".into(), "c".into()], total: 7 };
    assert_eq!(hint.label(), "7 fields detected: a, b, c and 4 more");
}
