//! Taking the lines the Focus Pane lists out: their stored text, in view
//! order, Continuation Lines included.
mod common;

use gpui::TestAppContext;

fn lines() -> Vec<String> {
    vec![
        "2026-10-19 10:00:01 INFO request ok".into(),
        "2026-10-19 10:00:02 ERROR db timeout".into(),
        "    at db.Pool.get(Pool.java:10)".into(),
        "2026-10-19 10:00:03 INFO cache miss".into(),
        "2026-10-19 10:00:04 ERROR db down".into(),
    ]
}

fn search(h: &mut common::Harness, text: &str) {
    h.view.update_in(&mut h.cx, |app, window, cx| app.activate_search(window, cx));
    h.cx.simulate_input(text);
    h.cx.executor().advance_clock(std::time::Duration::from_millis(150));
    h.cx.run_until_parked();
}

#[gpui::test]
fn the_export_is_the_listed_lines_with_their_continuation_lines(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    search(&mut h, "error");
    let (text, count) = h.view.read_with(&h.cx, |app, _| app.focus_pane_export());
    assert_eq!(
        text,
        "2026-10-19 10:00:02 ERROR db timeout\n    at db.Pool.get(Pool.java:10)\n2026-10-19 10:00:04 ERROR db down\n"
    );
    assert_eq!(count, 3);
}

#[gpui::test]
fn copying_says_how_many_lines(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    search(&mut h, "error");
    h.view.update(&mut h.cx, |app, cx| app.copy_focus_pane_lines(cx));
    let notice = h.view.read_with(&h.cx, |app, _| app.notice.clone());
    assert_eq!(notice.as_deref(), Some("Copied 3 lines"));
    let clip = h.cx.read_from_clipboard().and_then(|c| c.text());
    assert!(clip.unwrap_or_default().starts_with("2026-10-19 10:00:02 ERROR db timeout"));
}

#[gpui::test]
fn an_empty_pane_exports_nothing(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| app.copy_focus_pane_lines(cx));
    let notice = h.view.read_with(&h.cx, |app, _| app.notice.clone());
    assert_eq!(notice.as_deref(), Some("The Focus Pane lists no lines"));
}
