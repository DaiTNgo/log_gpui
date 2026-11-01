//! The Message card of Filter: Message has no values to tick, only Custom
//! Values matched against the text of each line's message.

mod common;

use common::{Harness, open_app_with_lines};
use gpui::TestAppContext;
use log_gpui::workspace::state::ContextSection;

fn lines() -> Vec<String> {
    (0..300)
        .map(|i| {
            let what = if i % 7 == 0 { "request timeout" } else { "request done" };
            format!("2026-09-05 10:00:{:02} [INFO] [API] {what} {i}", i % 60)
        })
        .collect()
}

fn draw(h: &mut Harness) {
    h.cx.update(|window, _| window.refresh());
    h.cx.run_until_parked();
}

#[gpui::test]
fn message_is_picked_from_add_filter_and_takes_a_custom_value(cx: &mut TestAppContext) {
    let mut h = open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.context_sidebar.reveal(ContextSection::Filter);
        app.toggle_filter_picker(cx);
    });
    draw(&mut h);
    let pick = h.cx.debug_bounds("filter-pick-message").expect("Message is in the column list");
    h.cx.simulate_click(pick.center(), gpui::Modifiers::none());
    h.view.update(&mut h.cx, |app, cx| {
        assert_eq!(app.filter_ui.expanded.as_deref(), Some("message"));
        assert!(app.filter_add_expanded_custom("TIMEOUT", cx));
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let every_row = app.filter_state.evaluate(&app.index(), &app.rows());
        assert_eq!(every_row.len(), 43);
        assert_eq!(app.filter_pass, every_row);
        assert_eq!(app.unread_column("message"), None, "the message is not a column no longer read");
    });
}
