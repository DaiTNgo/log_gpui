//! The value list of a Filter column with many values (openspec change
//! `add-swimlane`, task 1.2): a column may have up to 1000 values, so only
//! the rows in sight are built, and the list still scrolls and ticks.
mod common;

use common::Harness;
use gpui::{Bounds, Modifiers, Pixels, TestAppContext};
use log_gpui::workspace::state::ContextSection;

/// Lines of `worker-0` to `worker-999`, each once.
fn lines() -> Vec<String> {
    (0..1000)
        .map(|i| format!(r#"{{"time":"2026-10-29T10:{:02}:{:02}Z","level":"info","thread":"worker-{i}","msg":"job {i}"}}"#, i / 60 % 60, i % 60))
        .collect()
}

fn draw(h: &mut Harness) {
    h.view.update(&mut h.cx, |_, cx| cx.notify());
    h.cx.simulate_resize(gpui::size(gpui::px(1400.0), gpui::px(900.0)));
    h.cx.run_until_parked();
}

fn row(h: &mut Harness, i: usize) -> Option<Bounds<Pixels>> {
    h.cx.debug_bounds(Box::leak(format!("filter-val-thread-{i}").into_boxed_str()))
}

/// The value rows drawn, by index.
fn drawn(h: &mut Harness) -> Vec<usize> {
    (0..1000).filter(|&i| row(h, i).is_some()).collect()
}

#[gpui::test]
fn a_list_of_1000_values_builds_only_the_rows_in_sight(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.context_sidebar.reveal(ContextSection::Filter);
        app.pick_filter_column("thread", cx);
        app.filter_ui.expanded = Some("thread".into());
    });
    draw(&mut h);
    let first = drawn(&mut h);
    assert!(!first.is_empty() && first.len() <= 12, "{first:?}");
    assert_eq!(first[0], 0);

    // The wheel over the list brings later values in.
    let at = row(&mut h, 0).unwrap().center();
    h.cx.simulate_event(gpui::ScrollWheelEvent {
        position: at,
        delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(-2000.0))),
        ..Default::default()
    });
    draw(&mut h);
    let later = drawn(&mut h);
    assert!(later.first().is_some_and(|&i| i > 50), "{later:?}");

    // Ticking a row in sight ticks its value.
    let i = later[later.len() / 2];
    let left = row(&mut h, i).unwrap();
    h.cx.simulate_click(gpui::point(left.origin.x + gpui::px(8.0), left.center().y), Modifiers::none());
    h.cx.run_until_parked();
    let ticked = h.view.read_with(&h.cx, |app, _| app.filter_state.is_value_ticked("thread", &format!("worker-{i}")));
    assert!(ticked, "worker-{i} is ticked");
}
