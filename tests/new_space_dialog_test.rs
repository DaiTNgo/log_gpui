//! The New Log Space dialog in the real `AppView`: what it makes from the
//! remote picker, and how it lays out many members.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::remote_picker::{PickerTarget, Staged};
use log_gpui::remote::{AuthMethod, Server};

/// A Server nothing listens on: the picker's folder load fails at once.
/// Its connection runs on the app's own threads, so the test lets them be.
fn add_server(h: &mut Harness) -> String {
    h.cx.executor().allow_parking();
    h.view.update(&mut h.cx, |app, _| {
        let mut server = Server::new("web", "127.0.0.1", "u", AuthMethod::Password);
        server.port = 1;
        app.remote.servers.save(server).unwrap()
    })
}

/// One Add in the picker: `paths` on `server_id`, with `start` typed.
fn add_in_picker(h: &mut Harness, server_id: &str, paths: &[&str], start: &str) {
    h.view.update(&mut h.cx, |app, cx| {
        app.open_remote_picker(PickerTarget::NewSpace, cx);
        let picker = app.remote.picker.as_mut().unwrap();
        picker.staged = paths
            .iter()
            .map(|p| Staged::File { server_id: server_id.to_string(), path: p.to_string() })
            .collect();
        picker.start_time = start.to_string();
        app.picker_confirm(cx);
    });
    h.cx.run_until_parked();
}

#[gpui::test]
fn a_starting_time_goes_only_to_the_files_added_with_it(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    add_in_picker(&mut h, &server, &["/var/log/api.log"], "1h");
    add_in_picker(&mut h, &server, &["/var/log/db.log"], "");

    h.view.update(&mut h.cx, |app, cx| {
        app.new_space.as_mut().unwrap().form.set_name("Checkout");
        app.new_space_create(cx);
    });
    h.cx.run_until_parked();

    let starts: Vec<(String, Option<String>)> = h.view.read_with(&h.cx, |app, _| {
        let project = app.workspace_state.active_project().unwrap();
        let space = project.log_spaces.iter().find(|s| s.name == "Checkout").expect("the Remote Log Space was made");
        space
            .files
            .iter()
            .map(|f| (f.display_name.clone(), f.remote.as_ref().and_then(|r| r.start_time.clone())))
            .collect()
    });
    assert_eq!(
        starts,
        vec![
            ("web:/var/log/api.log".to_string(), Some("1h".to_string())),
            ("web:/var/log/db.log".to_string(), None),
        ]
    );
}

#[gpui::test]
fn removing_a_chosen_file_forgets_its_starting_time(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    add_in_picker(&mut h, &server, &["/var/log/api.log"], "1h");
    h.view.update(&mut h.cx, |app, cx| app.new_space_remove_remote(0, cx));
    add_in_picker(&mut h, &server, &["/var/log/api.log"], "");
    h.view.read_with(&h.cx, |app, _| assert!(app.remote.new_space_starts.is_empty()));
}

