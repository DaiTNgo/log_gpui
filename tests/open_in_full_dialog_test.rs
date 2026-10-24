//! The Open in full dialog in the real `AppView`, on a Server nothing
//! listens on: what it says before it reads anything.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::remote::{AuthMethod, Server};

/// A Remote Log Space "Prod" with one file on a Server nothing listens on,
/// with its Open in full dialog open.
fn open_dialog(h: &mut Harness) {
    h.cx.executor().allow_parking();
    h.view.update(&mut h.cx, |app, cx| {
        let mut server = Server::new("web", "127.0.0.1", "u", AuthMethod::Password);
        server.port = 1;
        let id = app.remote.servers.save(server).unwrap();
        let space_id = app
            .create_remote_log_space("Prod", vec![(id, "web".into(), "/var/log/api.log".into())], Vec::new(), Vec::new(), cx)
            .unwrap();
        app.open_in_full_space(&space_id, cx);
    });
    h.cx.run_until_parked();
}

fn set_name(h: &mut Harness, name: &str) {
    h.view.update(&mut h.cx, |app, cx| {
        app.remote.full_dialog.as_mut().unwrap().new_name = name.to_string();
        cx.notify();
    });
    h.cx.run_until_parked();
}

#[gpui::test]
fn a_name_that_cannot_be_used_is_said_in_the_dialog_and_keeps_it_open(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    open_dialog(&mut h);
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.full_name_error(), None, "the suggested name is free"));
    assert!(h.cx.debug_bounds("full-name-error").is_none());

    set_name(&mut h, "  ");
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.full_name_error(), Some("Enter a name")));
    assert!(h.cx.debug_bounds("full-name-error").is_some(), "said under the field");

    set_name(&mut h, "Prod");
    h.view.update(&mut h.cx, |app, cx| {
        assert_eq!(app.full_name_error(), Some("A Log Space with this name already exists"));
        app.confirm_open_in_full(cx);
        assert!(app.remote.full_dialog.is_some(), "Open does nothing: the dialog and its choices stay");
        assert_eq!(app.workspace_state.active_project().unwrap().log_spaces.len(), 1);
    });

    // Adding to an existing Log Space needs no name.
    h.view.update(&mut h.cx, |app, cx| {
        let id = app.workspace_state.active_project().unwrap().log_spaces[0].id.clone();
        app.choose_full_destination(Some(id), cx);
        assert_eq!(app.full_name_error(), None);
    });
}

#[gpui::test]
fn many_log_spaces_scroll_and_open_stays_on_screen(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, _| {
        let project = app.workspace_state.active_project_mut().unwrap();
        for i in 1..=40 {
            project.create_log_space(&format!("Imported {i}"), log_gpui::workspace::log_space::LogSpaceKind::Imported).unwrap();
        }
    });
    open_dialog(&mut h);
    h.cx.update(|window, _| window.refresh());
    h.cx.run_until_parked();

    let footer = h.cx.debug_bounds("full-footer").expect("the buttons are drawn");
    let window = h.cx.update(|window, _| window.viewport_size());
    assert!(footer.bottom() <= window.height, "Open is on screen: {footer:?} in {window:?}");
    let body = h.cx.debug_bounds("full-body").expect("the body is drawn");
    assert!(body.bottom() <= footer.top(), "the body ends above the buttons");
}
