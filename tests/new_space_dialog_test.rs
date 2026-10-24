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

#[gpui::test]
fn many_remote_files_scroll_and_create_stays_on_screen(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    let paths: Vec<String> = (1..=40).map(|i| format!("/var/log/app-{i}.log")).collect();
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    add_in_picker(&mut h, &server, &paths, "");
    h.cx.update(|window, _| window.refresh());
    h.cx.run_until_parked();

    let list = h.cx.debug_bounds("new-space-remote").expect("the remote list is drawn");
    assert!(list.size.height <= gpui::px(180.0), "the list scrolls: {:?}", list.size.height);
    let footer = h.cx.debug_bounds("new-space-footer").expect("the buttons are drawn");
    let window = h.cx.update(|window, _| window.viewport_size());
    assert!(footer.bottom() <= window.height, "Create is on screen: {footer:?} in {window:?}");
}

#[gpui::test]
fn a_folder_can_be_chosen_as_well_as_files(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    // The native picker takes files or folders, never both: a second button
    // opens it on folders.
    assert!(h.cx.debug_bounds("new-space-browse-folder").is_some(), "Choose folder… is drawn");

    // What the folder picker hands back is listed like a dropped folder.
    let folder = h.dir.path().join("logs");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("a.log"), "one\n").unwrap();
    std::fs::write(folder.join("b.log"), "two\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| {
        app.new_space_add_paths(vec![folder.to_string_lossy().into_owned()], cx);
    });
    let (names, label) = h.view.read_with(&h.cx, |app, _| {
        let form = &app.new_space.as_ref().unwrap().form;
        (form.files.iter().map(|f| f.display_name()).collect::<Vec<_>>(), form.create_label())
    });
    assert_eq!(names, ["a.log", "b.log"]);
    assert_eq!(label, "Create with 2 files");
}

#[gpui::test]
fn the_dialog_says_why_local_and_remote_files_do_not_mix(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-local-hint").is_none());
    assert!(h.cx.debug_bounds("new-space-remote-hint").is_none());

    // A local file turns the remote buttons off, and the dialog says why.
    let log = h.dir.path().join("a.log");
    std::fs::write(&log, "one\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![log.to_string_lossy().into_owned()], cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-remote-hint").is_some());

    // A remote file hides the drop zone, and the dialog says why.
    h.view.update(&mut h.cx, |app, cx| app.new_space_remove_file(0, cx));
    add_in_picker(&mut h, &server, &["/var/log/api.log"], "");
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-local-hint").is_some());
    assert!(h.cx.debug_bounds("new-space-remote-hint").is_none());
}

#[gpui::test]
fn the_first_file_names_the_log_space_until_a_name_is_typed(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let server = add_server(&mut h);
    let name = |h: &mut Harness| h.view.read_with(&h.cx, |app, _| app.new_space.as_ref().unwrap().form.name.clone());
    let field = |h: &mut Harness| {
        h.view.update(&mut h.cx, |app, cx| {
            app.new_space.as_ref().unwrap().name_input.as_ref().unwrap().read(cx).value().to_string()
        })
    };

    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    let folder = h.dir.path().join("checkout");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("a.log"), "one\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![folder.to_string_lossy().into_owned()], cx));
    h.cx.run_until_parked();
    assert_eq!(name(&mut h), "checkout");
    assert_eq!(field(&mut h), "checkout", "the field shows it");

    // A name already there is kept.
    let other = h.dir.path().join("b.log");
    std::fs::write(&other, "two\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![other.to_string_lossy().into_owned()], cx));
    assert_eq!(name(&mut h), "checkout");

    // Emptied by hand, the dialog says why Create is off.
    h.view.update(&mut h.cx, |app, cx| {
        app.new_space.as_mut().unwrap().form.set_name("");
        cx.notify();
    });
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-name-hint").is_some());

    // A remote file names a fresh dialog too.
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    add_in_picker(&mut h, &server, &["/var/log/nginx/access.log"], "");
    assert_eq!(name(&mut h), "access");
}

#[gpui::test]
fn create_empty_says_what_it_makes(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, cx| app.open_new_space_dialog(cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-scratch-hint").is_some());

    let log = h.dir.path().join("a.log");
    std::fs::write(&log, "one\n").unwrap();
    h.view.update(&mut h.cx, |app, cx| app.new_space_add_paths(vec![log.to_string_lossy().into_owned()], cx));
    h.cx.run_until_parked();
    assert!(h.cx.debug_bounds("new-space-scratch-hint").is_none(), "gone once a file is listed");
}
