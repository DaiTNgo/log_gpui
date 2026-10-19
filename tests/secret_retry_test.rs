mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::live::SourceStatus;
use log_gpui::remote::{AuthMethod, Server};
use log_gpui::workspace::log_space::LogSpaceKind;

/// A Remote Log Space with one Remote File on a Server nothing listens on,
/// so its source never connects on its own. Returns the Server and file ids.
fn remote_space(h: &mut Harness, auth: AuthMethod, remember: bool) -> (String, String) {
    h.view.update(&mut h.cx, |app, cx| {
        let mut server = Server::new("box", "127.0.0.1", "me", auth);
        server.port = 1;
        server.remember_secret = remember;
        let server_id = app.remote.servers.save(server).unwrap();
        *app.remote.shared_servers.lock().unwrap() = app.remote.servers.clone();
        let space_id = app.create_log_space("Remote", LogSpaceKind::Remote, &[], cx).unwrap();
        app.add_remote_files_to_space(&space_id, vec![(server_id.clone(), "box".into(), "/var/log/app.log".into())], cx);
        let space = app.workspace_state.projects.iter().find_map(|p| p.log_space(&space_id)).unwrap();
        let file_id = space.files[0].file_id.clone();
        (server_id, file_id)
    })
}

/// What the source of `file_id` reports, as if it came from the hub.
fn report(h: &mut Harness, file_id: &str, status: SourceStatus) {
    h.view.update(&mut h.cx, |app, cx| {
        app.live.sources.get_mut(file_id).unwrap().status = status;
        app.ask_again_after_auth_failure(cx);
    });
}

#[gpui::test]
fn wrong_password_reopens_the_dialog_with_an_error(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let (server_id, file_id) = remote_space(&mut h, AuthMethod::Password, true);
    report(&mut h, &file_id, SourceStatus::AuthFailed);

    h.view.read_with(&h.cx, |app, _| {
        let prompt = app.remote.secret_prompt.as_ref().expect("dialog reopened");
        assert_eq!(prompt.server_id, server_id);
        assert_eq!(prompt.error.as_deref(), Some("Wrong password \u{2014} try again"));
        assert!(prompt.remember, "Remember choice is kept");
        // The file now waits for the user to Connect.
        assert_eq!(app.live.sources[&file_id].status, SourceStatus::NeedsSecret);
    });
}

#[gpui::test]
fn wrong_passphrase_says_passphrase(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let (_, file_id) = remote_space(&mut h, AuthMethod::Key { path: "/tmp/key".into() }, false);
    report(&mut h, &file_id, SourceStatus::AuthFailed);
    h.view.read_with(&h.cx, |app, _| {
        let prompt = app.remote.secret_prompt.as_ref().expect("dialog reopened");
        assert_eq!(prompt.error.as_deref(), Some("Wrong passphrase \u{2014} try again"));
        assert!(!prompt.remember);
    });
}

#[gpui::test]
fn other_failures_do_not_ask_for_a_secret(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let (_, file_id) = remote_space(&mut h, AuthMethod::Password, false);
    report(&mut h, &file_id, SourceStatus::Error("Host key changed".into()));
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.remote.secret_prompt.is_none());
        assert_eq!(app.live.sources[&file_id].status, SourceStatus::Error("Host key changed".into()));
    });
}

#[gpui::test]
fn cancel_leaves_the_file_waiting_to_connect(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let (_, file_id) = remote_space(&mut h, AuthMethod::Password, false);
    report(&mut h, &file_id, SourceStatus::AuthFailed);
    h.view.update(&mut h.cx, |app, cx| {
        app.cancel_secret_prompt(cx);
        // Not asked again on the next batch of events.
        app.ask_again_after_auth_failure(cx);
    });
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.remote.secret_prompt.is_none());
        assert_eq!(app.live.sources[&file_id].status, SourceStatus::NeedsSecret);
    });
}
