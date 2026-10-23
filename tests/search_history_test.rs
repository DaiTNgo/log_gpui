//! Recent Search Expressions: remembered when searched, stepped through
//! with ↑/↓ in an empty Search field, kept with the Project.
mod common;

use gpui::TestAppContext;

fn type_search(h: &mut common::Harness, text: &str) {
    h.view.update_in(&mut h.cx, |app, window, cx| app.activate_search(window, cx));
    h.cx.simulate_input(text);
    h.cx.executor().advance_clock(std::time::Duration::from_millis(150));
    h.cx.run_until_parked();
}

fn clear_search(h: &mut common::Harness) {
    h.view.update_in(&mut h.cx, |app, window, cx| app.pick_search_history("", window, cx));
    h.view.update(&mut h.cx, |app, _| app.search_history_streak = false);
    h.cx.run_until_parked();
}

fn history(h: &common::Harness) -> Vec<String> {
    h.view.read_with(&h.cx, |app, _| app.search_history())
}

fn query(h: &common::Harness) -> String {
    h.view.read_with(&h.cx, |app, _| app.focus.search().query.clone())
}

#[gpui::test]
fn a_search_is_remembered_once_per_typing(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    type_search(&mut h, "line");
    type_search(&mut h, " 1");
    assert_eq!(history(&h), ["line 1"], "the pause in the middle is not kept");
    clear_search(&mut h);
    type_search(&mut h, "INFO");
    assert_eq!(history(&h), ["INFO", "line 1"]);
}

#[gpui::test]
fn up_and_down_step_through_recent_searches(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    type_search(&mut h, "line 1");
    clear_search(&mut h);
    type_search(&mut h, "INFO");
    clear_search(&mut h);

    h.cx.simulate_keystrokes("up");
    assert_eq!(query(&h), "INFO");
    h.cx.simulate_keystrokes("up");
    assert_eq!(query(&h), "line 1");
    h.cx.simulate_keystrokes("down down");
    assert_eq!(query(&h), "", "back to what was typed");
    h.cx.simulate_keystrokes("up escape");
    assert_eq!(query(&h), "", "Escape restores what was typed");
}

#[gpui::test]
fn recent_searches_are_kept_with_the_project(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    type_search(&mut h, "line 1");
    let kept = h.view.read_with(&h.cx, |app, _| {
        let manifest = log_gpui::workspace::session::SessionManifest::from_workspace(&app.workspace_state);
        manifest.into_workspace().active_project().unwrap().search_history.items.clone()
    });
    assert_eq!(kept, ["line 1"]);
    h.view.update(&mut h.cx, |app, cx| app.remove_search_history("line 1", cx));
    assert!(history(&h).is_empty());
}
