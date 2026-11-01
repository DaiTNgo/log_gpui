//! A Detected Field turned on, or another Log Format, reads a file again
//! (openspec change `line-offset-index`, task 6.3): its rows stay on screen
//! meanwhile, the rows shown are parsed the new way at once, and Filter
//! gets the values of the new column once Indexing is done.

mod common;

use gpui::TestAppContext;

fn lines() -> Vec<String> {
    (0..3000)
        .map(|i| {
            let user = ["alice", "bob", "carol"][i % 3];
            format!("2026-10-20 10:{:02}:{:02} INFO [api] charge ok user={user} n={i}", i / 60 % 60, i % 60)
        })
        .collect()
}

#[gpui::test]
fn turning_a_detected_field_on_keeps_the_rows_and_reads_it_at_once(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.set_column_shown("user", true, cx);
        assert!(app.is_reading(), "the file is read again");
        assert_eq!(app.view.len(), 3000, "its rows stay meanwhile");
        assert!(app.empty_canvas().is_none());
        let first = app.rows_snapshot()[0].span_text("user").map(str::to_string);
        assert_eq!(first.as_deref(), Some("alice"), "the rows shown read the field at once");
    });
    h.cx.run_until_parked();
    h.view.update(&mut h.cx, |app, cx| {
        assert!(!app.is_reading());
        assert_eq!(app.view.len(), 3000);
        let count: usize = app.index().column("user").expect("Filter has the new column").values.iter().map(|v| v.count).sum();
        assert_eq!(count, 3000);
        app.filter_toggle_value("user", "bob", cx);
        assert_eq!(app.filter_pass.len(), 1000);
    });
}

#[gpui::test]
fn turning_it_on_and_off_again_quickly_reads_the_file_once_more(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.set_column_shown("user", true, cx);
        app.set_column_shown("user", false, cx);
        assert_eq!(app.reading().len(), 1, "the first read again gave way to the second");
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.is_reading());
        assert!(app.index().column("user").is_none());
        assert_eq!(app.view.len(), 3000);
    });
}
