mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::ConfirmAction;
use log_gpui::remote::{AuthMethod, Server};
use log_gpui::workspace::project::ProjectColorAccent;

fn add_server(h: &mut Harness, name: &str, jump: Option<String>) -> String {
    h.view.update(&mut h.cx, |app, _| {
        let mut server = Server::new(name, "10.0.0.1", "deploy", AuthMethod::Password);
        server.jump_server_id = jump;
        app.remote.servers.save(server).unwrap()
    })
}

fn server_count(h: &mut Harness) -> usize {
    h.view.read_with(&h.cx, |app, _| app.remote.servers.servers.len())
}

fn confirm(h: &mut Harness) -> Option<ConfirmAction> {
    h.view.read_with(&h.cx, |app, _| app.confirm.clone())
}

#[gpui::test]
fn deleting_a_server_asks_first(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let id = add_server(&mut h, "web", None);
    h.view.update(&mut h.cx, |app, cx| {
        app.open_servers_dialog(cx);
        app.request_delete_server(&id, cx);
    });
    assert_eq!(confirm(&mut h), Some(ConfirmAction::DeleteServer { server_id: id.clone() }));
    assert_eq!(server_count(&mut h), 1);

    h.view.update(&mut h.cx, |app, cx| app.cancel_confirm(cx));
    assert_eq!(server_count(&mut h), 1);

    h.view.update(&mut h.cx, |app, cx| {
        app.request_delete_server(&id, cx);
        app.accept_confirm(cx);
    });
    assert_eq!(server_count(&mut h), 0);
}

#[gpui::test]
fn a_server_in_use_is_refused_without_asking(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let hop = add_server(&mut h, "hop", None);
    add_server(&mut h, "db", Some(hop.clone()));
    h.view.update(&mut h.cx, |app, cx| {
        app.open_servers_dialog(cx);
        app.request_delete_server(&hop, cx);
    });
    assert_eq!(confirm(&mut h), None);
    let error = h.view.read_with(&h.cx, |app, _| app.server_dialog.as_ref().unwrap().error.clone());
    assert!(error.is_some_and(|e| e.starts_with("Used by:") && e.contains("db")));
    assert_eq!(server_count(&mut h), 2);
}

#[gpui::test]
fn removing_a_project_asks_first(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let id = h.view.update(&mut h.cx, |app, _| {
        app.workspace_state.create_project("Other", ProjectColorAccent::Blue).id.clone()
    });
    let count = |h: &mut Harness| h.view.read_with(&h.cx, |app, _| app.workspace_state.project_count());
    assert_eq!(count(&mut h), 2);

    h.view.update(&mut h.cx, |app, cx| app.request_remove_project(&id, cx));
    assert_eq!(confirm(&mut h), Some(ConfirmAction::RemoveProject { project_id: id.clone() }));
    h.view.update(&mut h.cx, |app, cx| app.cancel_confirm(cx));
    assert_eq!(count(&mut h), 2);

    h.view.update(&mut h.cx, |app, cx| {
        app.request_remove_project(&id, cx);
        app.accept_confirm(cx);
    });
    assert_eq!(count(&mut h), 1);
}

#[gpui::test]
fn the_last_project_cannot_be_removed(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let id = h.view.read_with(&h.cx, |app, _| app.workspace_state.projects[0].id.clone());
    h.view.update(&mut h.cx, |app, cx| app.request_remove_project(&id, cx));
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.can_remove_project());
        assert_eq!(app.confirm, None);
        assert_eq!(app.workspace_state.project_count(), 1);
    });
}

#[gpui::test]
fn forgetting_a_host_key_is_not_an_error(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let id = add_server(&mut h, "web", None);
    h.view.update(&mut h.cx, |app, cx| {
        app.open_servers_dialog(cx);
        app.forget_server_host_key(&id, cx);
    });
    h.view.read_with(&h.cx, |app, _| {
        let dialog = app.server_dialog.as_ref().unwrap();
        assert_eq!(dialog.error, None);
        assert!(dialog.info.as_deref().is_some_and(|m| m.contains("no longer trusted")));
    });
}
