mod common;

use common::Harness;
use gpui::{Focusable as _, TestAppContext};
use log_gpui::remote::HostKeyRequest;

/// Press a key in the app window, with the app view focused, the way a
/// person would.
fn press(h: &mut Harness, keys: &str) {
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let handle = app.focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    });
    h.cx.run_until_parked();
    h.cx.simulate_keystrokes(keys);
    h.cx.run_until_parked();
}

fn search_query(h: &mut Harness) -> String {
    h.view.read_with(&h.cx, |app, _| app.focus.search().query.clone())
}

#[gpui::test]
fn escape_closes_the_server_form_then_the_servers_dialog(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    h.view.update_in(&mut h.cx, |app, window, cx| {
        app.open_servers_dialog(cx);
        app.new_server_form(cx);
        app.sync_server_dialog(window, cx);
    });
    press(&mut h, "escape");
    h.view.read_with(&h.cx, |app, _| {
        let dialog = app.server_dialog.as_ref().expect("Servers dialog still open");
        assert!(dialog.form.is_none(), "the form closed first");
    });
    press(&mut h, "escape");
    h.view.read_with(&h.cx, |app, _| assert!(app.server_dialog.is_none()));
}

#[gpui::test]
fn escape_rejects_a_host_key(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let (reply, answer) = tokio::sync::oneshot::channel();
    h.view.update(&mut h.cx, |app, cx| {
        app.remote.host_prompts.push_back(HostKeyRequest {
            server_name: "web".into(),
            address: "10.0.0.1:22".into(),
            fingerprint: "SHA256:abc".into(),
            reply,
        });
        cx.notify();
    });
    press(&mut h, "escape");
    h.view.read_with(&h.cx, |app, _| assert!(app.remote.host_prompts.is_empty()));
    assert_eq!(answer.blocking_recv(), Ok(false));
}

#[gpui::test]
fn escape_cancels_the_secret_dialog(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, cx| {
        let server = log_gpui::remote::Server::new("web", "10.0.0.1", "deploy", log_gpui::remote::AuthMethod::Password);
        let id = app.remote.servers.save(server).unwrap();
        app.open_secret_prompt(&id, cx);
    });
    press(&mut h, "escape");
    h.view.read_with(&h.cx, |app, _| assert!(app.remote.secret_prompt.is_none()));
}

#[gpui::test]
fn escape_in_a_dialog_leaves_the_search_alone(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(5));
    h.view.update_in(&mut h.cx, |app, window, cx| app.activate_search(window, cx));
    h.cx.simulate_input("line");
    h.cx.run_until_parked();
    assert_eq!(search_query(&mut h), "line");

    h.view.update(&mut h.cx, |app, cx| app.open_servers_dialog(cx));
    press(&mut h, "escape");
    h.view.read_with(&h.cx, |app, _| assert!(app.server_dialog.is_none()));
    assert_eq!(search_query(&mut h), "line");
}

#[gpui::test]
fn new_log_space_and_confirm_still_close_on_escape(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    press(&mut h, "escape");
    h.view.read_with(&h.cx, |app, _| assert!(app.new_space.is_none()));

    let id = h.view.update(&mut h.cx, |app, _| {
        app.remote.servers.save(log_gpui::remote::Server::new("web", "h", "u", log_gpui::remote::AuthMethod::Password)).unwrap()
    });
    h.view.update(&mut h.cx, |app, cx| {
        app.open_servers_dialog(cx);
        app.request_delete_server(&id, cx);
    });
    // The Confirm is on top: Escape cancels it, the Servers dialog stays.
    press(&mut h, "escape");
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.confirm.is_none());
        assert!(app.server_dialog.is_some());
        assert_eq!(app.remote.servers.servers.len(), 1);
    });
}

#[gpui::test]
fn find_in_log_focuses_the_search_field_and_leaves_panels(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(5));
    let panels = |h: &mut Harness| h.view.read_with(&h.cx, |app, _| format!("{:?}", app.context_sidebar));
    let before = panels(&mut h);
    h.view.update_in(&mut h.cx, |app, window, cx| {
        app.execute_command(log_gpui::keymap::Command::ToggleSearch, window, cx);
    });
    h.cx.run_until_parked();
    let focused = h.view.update_in(&mut h.cx, |app, window, cx| {
        app.search_input.read(cx).focus_handle(cx).is_focused(window)
    });
    assert!(focused, "the Search field has the cursor");
    assert_eq!(panels(&mut h), before, "no panel opened or closed");
    // Typing goes straight into the Search field.
    h.cx.simulate_input("line 3");
    h.cx.run_until_parked();
    assert_eq!(search_query(&mut h), "line 3");
}
